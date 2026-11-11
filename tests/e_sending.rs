//! E1, E3–E5 and E7: an approved reminder goes through the engine's outbox,
//! only within the sending rules, and says nothing about the patient's care.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::checks::check;
use care_reminders::reminders::LatestDraft;
use care_reminders::sending::cap_until;
use common::outbox::setup;
use sqlx::PgPool;
use time::Duration;
use time::macros::datetime;

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

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e1_an_email_of_a_reminder_already_finished_is_never_sent(pool: PgPool) {
    let s = setup(pool).await;
    let drafts = s.drafts().await;
    s.approve(&drafts[0].0).await;
    // Finished another way (sent by an earlier draft, or skipped) while it waited.
    sqlx::query("update reminders set finished_at = now(), outcome = 'skipped' where id = $1")
        .bind(drafts[0].0.reminder.as_uuid())
        .execute(&s.pool)
        .await
        .unwrap();

    assert_eq!(s.send().await, 0);
    assert!(s.sent_bodies().await.is_empty());
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e7_an_email_that_would_name_care_is_refused_at_approval_with_the_reason(pool: PgPool) {
    let s = setup(pool).await;
    let (draft, patient) = s.drafts().await.remove(0);
    // A made-up first name that is also a clinical word, filled in by code.
    sqlx::query("update patients set given_name = 'Biopsy' where fhir_id = $1")
        .bind(&patient)
        .execute(&s.pool)
        .await
        .unwrap();

    let refused = s
        .reviews
        .approve(
            &s.coordinator,
            draft.reminder,
            draft.draft,
            &care_reminders::review::content_hash(&draft.text),
        )
        .await;

    let err = refused.unwrap_err();
    assert!(
        matches!(err, care_reminders::review::ReviewError::CantFill(_)),
        "{err:?}"
    );
    assert!(err.to_string().to_lowercase().contains("biopsy"), "{err}");
    assert_eq!(s.count("select count(*) from reminder_messages").await, 0);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e1_an_email_refused_when_released_is_logged_with_its_reason_not_as_withdrawn(pool: PgPool) {
    let s = setup(pool).await;
    let (draft, _) = s.drafts().await.remove(0);
    s.approve(&draft).await;
    // As if the approval stopped before releasing the email, and the
    // patient's address changed meanwhile.
    sqlx::query("update outbox_messages set status = 'draft', approved_hash = null, approved_at = null, approved_by = null where reply_to is not null")
        .execute(&s.pool)
        .await
        .unwrap();
    sqlx::query("update patients set email = 'new@example.test'")
        .execute(&s.pool)
        .await
        .unwrap();

    assert_eq!(s.send().await, 0);
    let delivery = care_reminders::sending::delivery(&s.pool, s.coordinator.tenant, draft.reminder)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(delivery.status, "not_sent");
    assert!(delivery.detail.unwrap().contains("address"));
    assert_eq!(
        s.count("select count(*) from audit_events where action = 'reminder.refused'")
            .await,
        1
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e1_the_same_text_can_be_approved_again_once_its_email_was_refused(pool: PgPool) {
    let s = setup(pool).await;
    let (draft, patient) = s.drafts().await.remove(0);
    s.approve(&draft).await;
    let old: String = sqlx::query_scalar("select email from patients where fhir_id = $1")
        .bind(&patient)
        .fetch_one(&s.pool)
        .await
        .unwrap();
    set_email(&s.pool, &patient, "moved@example.test").await;
    assert_eq!(s.send().await, 0, "refused: the address changed");
    set_email(&s.pool, &patient, &old).await;

    s.approve(&draft).await;

    assert_eq!(s.send().await, 1);
    assert_eq!(s.count("select count(*) from reminder_messages").await, 2);
}

async fn set_email(pool: &PgPool, patient: &str, address: &str) {
    sqlx::query("update patients set email = $1 where fhir_id = $2")
        .bind(address)
        .bind(patient)
        .execute(pool)
        .await
        .unwrap();
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn c6_an_email_whose_date_passes_while_it_waits_is_held_back_for_approving_again(pool: PgPool) {
    let s = setup(pool).await;
    let (draft, _) = s.drafts().await.remove(0);
    // Due by Saturday 14 November; approved that Saturday, so it waits for Monday.
    sqlx::query(
        "update follow_ups f set due_by = '2026-11-14' from reminders r where r.follow_up_id = f.id and r.id = $1",
    )
    .bind(draft.reminder.as_uuid())
    .execute(&s.pool)
    .await
    .unwrap();
    s.clock.advance(Duration::days(4) + Duration::hours(1));
    s.approve(&draft).await;

    s.clock.advance(Duration::days(2));
    let monday = s.send().await;

    assert_eq!(monday, 0, "{:?}", s.sent_bodies().await);
    let delivery = care_reminders::sending::delivery(&s.pool, s.coordinator.tenant, draft.reminder)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(delivery.status, "not_sent");
    assert!(delivery.detail.unwrap().contains("has passed"));
    // Approved again, it asks the patient to book as soon as they can.
    s.approve(&draft).await;
    assert_eq!(s.send().await, 1);
    let bodies = s.sent_bodies().await;
    assert!(
        bodies[0].contains("as soon as you can") && !bodies[0].contains("November 14"),
        "{bodies:?}"
    );
}
