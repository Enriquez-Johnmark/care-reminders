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
