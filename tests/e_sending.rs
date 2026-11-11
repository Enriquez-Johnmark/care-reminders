//! E1, E3–E5 and E7: an approved reminder goes through the engine's outbox,
//! only within the sending rules, and says nothing about the patient's care.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use std::sync::Arc;

use care_reminders::checks::check;
use care_reminders::engine::accounts::{Role, Session};
use care_reminders::engine::clock::{Clock, FixedClock};
use care_reminders::engine::mail::MailpitMailer;
use care_reminders::engine::outbox::Sender;
use care_reminders::reminders::{Drafter, LatestDraft, Reminders};
use care_reminders::review::{Reviews, content_hash};
use care_reminders::sending::{SendingRules, cap_until, finish};
use common::{Fake, clock, practice};
use serde_json::{Value, json};
use sqlx::PgPool;
use time::Duration;
use time::macros::datetime;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct Setup {
    pool: PgPool,
    clock: Arc<FixedClock>,
    reviews: Reviews,
    coordinator: Session,
    mailpit: MockServer,
}

/// The practice with its first reminders drafted from the template, at 10 am
/// on Tuesday 10 November in New York, and a fake mail provider.
async fn setup(pool: PgPool) -> Setup {
    let clock = clock();
    clock.advance(Duration::hours(6));
    let family = practice(&pool, clock.clone()).await;
    care_reminders::settings::set_phone(&pool, family, "(555) 010-0100")
        .await
        .unwrap();
    Drafter::<Fake>::new(pool.clone(), clock.clone(), None)
        .draft_due()
        .await
        .unwrap();
    let reviews = Reviews::new(pool.clone(), clock.clone()).with_sending(common::sending());
    let member = reviews
        .add_coordinator(family, "carla@family.test", "Carla Reyes", "admin@family.test")
        .await
        .unwrap();
    let mailpit = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/send"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ID": "mp_1" })))
        .mount(&mailpit)
        .await;
    Setup {
        coordinator: Session {
            tenant: family,
            member,
            email: "carla@family.test".into(),
            role: Role::Staff,
        },
        pool,
        clock,
        reviews,
        mailpit,
    }
}

impl Setup {
    async fn drafts(&self) -> Vec<(LatestDraft, String)> {
        let drafts = Reminders::new(self.pool.clone())
            .latest_drafts(self.coordinator.tenant)
            .await
            .unwrap();
        let mut named = Vec::new();
        for draft in drafts {
            let name: String = sqlx::query_scalar(
                "select p.fhir_id from reminders r join follow_ups f on f.id = r.follow_up_id
                 join patients p on p.id = f.patient_id where r.id = $1",
            )
            .bind(draft.reminder.as_uuid())
            .fetch_one(&self.pool)
            .await
            .unwrap();
            named.push((draft, name));
        }
        named.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.reminder.as_uuid().cmp(&b.0.reminder.as_uuid())));
        named
    }

    async fn approve(&self, draft: &LatestDraft) {
        self.reviews
            .approve(
                &self.coordinator,
                draft.reminder,
                draft.draft,
                &content_hash(&draft.text),
            )
            .await
            .unwrap();
    }

    /// Sends what is due, as the server does, then marks what went out.
    async fn send(&self) -> usize {
        let as_clock: Arc<dyn Clock> = self.clock.clone();
        let mailer = MailpitMailer::new(&self.mailpit.uri(), "reminders@family.example.test").unwrap();
        let rules = Arc::new(SendingRules::new(self.pool.clone(), as_clock.clone()));
        finish(&self.pool, as_clock.clone()).await.unwrap();
        let report = Sender::new(self.pool.clone(), mailer, as_clock.clone(), rules)
            .drain_once(100)
            .await
            .unwrap();
        finish(&self.pool, as_clock).await.unwrap();
        report.sent
    }

    /// The bodies the fake mail provider received.
    async fn sent_bodies(&self) -> Vec<String> {
        self.mailpit
            .received_requests()
            .await
            .unwrap()
            .iter()
            .map(|request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                body["Text"].as_str().unwrap().to_owned()
            })
            .collect()
    }

    async fn count(&self, sql: &'static str) -> i64 {
        sqlx::query_scalar(sql).fetch_one(&self.pool).await.unwrap()
    }
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e1_nothing_reaches_the_outbox_without_an_approval(pool: PgPool) {
    let s = setup(pool).await;
    let drafts = s.drafts().await;
    let before = s.count("select count(*) from reminder_messages").await;

    s.approve(&drafts[0].0).await;

    assert_eq!(before, 0);
    assert_eq!(s.count("select count(*) from reminder_messages").await, 1);
    assert_eq!(s.send().await, 1);
    assert_eq!(s.sent_bodies().await.len(), 1);
    let outcome: String = sqlx::query_scalar("select outcome from reminders where id = $1")
        .bind(drafts[0].0.reminder.as_uuid())
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(outcome, "sent", "a sent reminder is finished, so the next can follow");
    assert_eq!(
        s.count("select count(*) from audit_events where action = 'reminder.sent'")
            .await,
        1
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e1_a_reminder_changed_after_approval_is_never_sent(pool: PgPool) {
    let s = setup(pool).await;
    let drafts = s.drafts().await;
    s.approve(&drafts[0].0).await;
    s.reviews
        .edit(
            &s.coordinator,
            drafts[0].0.reminder,
            &drafts[0].0.text.replace("Hello", "Hi"),
        )
        .await
        .unwrap();

    assert_eq!(s.send().await, 0);
    assert!(s.sent_bodies().await.is_empty());
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e3_a_patient_gets_at_most_one_reminder_a_day(pool: PgPool) {
    let s = setup(pool).await;
    // Ana (p01) has two follow-ups: an abnormal result and a screening.
    let anas: Vec<LatestDraft> = s
        .drafts()
        .await
        .into_iter()
        .filter(|(_, p)| p == "p01")
        .map(|(d, _)| d)
        .collect();
    assert_eq!(anas.len(), 2);
    for draft in &anas {
        s.approve(draft).await;
    }

    let today = s.send().await;
    s.clock.advance(Duration::days(1));
    let tomorrow = s.send().await;

    assert_eq!((today, tomorrow), (1, 1));
}

#[test]
fn e3_no_more_than_three_a_week_and_one_a_day() {
    let tz = "America/New_York";
    let now = datetime!(2026-11-13 15:00 UTC);

    let none = cap_until(&[], now, tz).unwrap();
    let today = cap_until(&[datetime!(2026-11-13 14:00 UTC)], now, tz).unwrap();
    let three = [
        datetime!(2026-11-08 15:00 UTC),
        datetime!(2026-11-10 15:00 UTC),
        datetime!(2026-11-12 15:00 UTC),
    ];
    let week = cap_until(&three, now, tz).unwrap();

    assert_eq!(none, None);
    // The next morning's window: Monday, as Saturday and Sunday are out.
    assert_eq!(today, Some(datetime!(2026-11-16 13:00 UTC)));
    // A week after the oldest of the three, then the next window.
    assert_eq!(week, Some(datetime!(2026-11-16 13:00 UTC)));
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e4_a_reminder_approved_at_the_weekend_waits_for_monday_morning(pool: PgPool) {
    let s = setup(pool).await;
    let drafts = s.drafts().await;
    // Saturday 14 November, 11 am in New York.
    s.clock.advance(Duration::days(4) + Duration::hours(1));
    s.approve(&drafts[0].0).await;

    let saturday = s.send().await;
    s.clock.advance(Duration::days(2) - Duration::hours(3));
    let monday_eight = s.send().await;

    assert_eq!((saturday, monday_eight), (0, 1));
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e5_a_patient_who_asked_not_to_be_emailed_is_never_emailed(pool: PgPool) {
    let s = setup(pool).await;
    // Jun (p10) asked not to be emailed.
    let jun = s.drafts().await.into_iter().find(|(_, p)| p == "p10").unwrap().0;

    s.approve(&jun).await;

    assert_eq!(s.count("select count(*) from reminder_messages").await, 0);
    assert_eq!(
        s.count("select count(*) from audit_events where action = 'reminder.not_emailed'")
            .await,
        1
    );
    assert_eq!(s.send().await, 0);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e5_a_patient_who_asks_not_to_be_emailed_after_approval_is_still_not_emailed(pool: PgPool) {
    let s = setup(pool).await;
    let drafts = s.drafts().await;
    s.approve(&drafts[0].0).await;
    sqlx::query("update patients set no_email = true")
        .execute(&s.pool)
        .await
        .unwrap();

    assert_eq!(s.send().await, 0);
    assert!(s.sent_bodies().await.is_empty());
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e7_the_sent_emails_name_no_reason_test_or_specialty(pool: PgPool) {
    let s = setup(pool).await;
    for (draft, _) in s.drafts().await {
        s.approve(&draft).await;
    }
    let reasons: Vec<String> = sqlx::query_scalar("select reason from follow_ups")
        .fetch_all(&s.pool)
        .await
        .unwrap();

    // One a day each: the patient with the most follow-ups has three.
    for _ in 0..3 {
        s.send().await;
        s.clock.advance(Duration::days(1));
    }

    let bodies = s.sent_bodies().await;
    // Forty reminders, but Jun asked not to be emailed.
    assert_eq!(bodies.len(), 39);
    for body in &bodies {
        let plain: String = body.chars().filter(|c| !c.is_ascii_digit()).collect();
        let problems = check(&plain).problems;
        assert!(
            problems
                .iter()
                .all(|p| p.check != care_reminders::checks::Check::ClinicalWords),
            "{body}: {problems:?}"
        );
        for reason in &reasons {
            // Each reason names a test, a screening or a hospital stay.
            let named = reason.split(" on ").next().unwrap().trim_start_matches("Abnormal ");
            assert!(!body.contains(named), "{body} names {named}");
        }
        assert!(body.contains("(555) 010-0100") && body.contains("/o/"), "{body}");
    }
}
