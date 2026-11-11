//! E6, and C1 and C7 end to end: reminders are drafted on their planned days
//! (0, 7 and 21), each only once the one before is finished and only while
//! the follow-up hasn't stopped; every draft is checked and stored with its
//! problems.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::checks::Check;
use care_reminders::reminders::{Drafter, Reminders};
use common::{Fake, GOOD_DRAFT, clock, practice};
use sqlx::PgPool;
use time::Duration;

/// Marks every reminder `number` as finished (skipped), as the coordinator
/// or sending will.
async fn finish(pool: &PgPool, number: i16) {
    sqlx::query("update reminders set finished_at = now(), outcome = 'skipped' where number = $1")
        .bind(number)
        .execute(pool)
        .await
        .unwrap();
}

async fn count(pool: &PgPool, sql: &'static str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap()
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e6_the_first_reminder_is_drafted_on_day_zero_and_only_once(pool: PgPool) {
    let clock = clock();
    practice(&pool, clock.clone()).await;
    let drafter = Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(GOOD_DRAFT))));

    assert_eq!(drafter.draft_due().await.unwrap(), 40);
    assert_eq!(drafter.draft_due().await.unwrap(), 0);
    assert_eq!(count(&pool, "select count(*) from drafts").await, 40);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e6_reminders_two_and_three_wait_for_day_seven_and_twenty_one_and_for_the_one_before(pool: PgPool) {
    let clock = clock();
    practice(&pool, clock.clone()).await;
    let drafter = Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(GOOD_DRAFT))));
    drafter.draft_due().await.unwrap();

    clock.advance(Duration::days(7));
    let while_first_open = drafter.draft_due().await.unwrap();
    finish(&pool, 1).await;
    clock.advance(Duration::days(-1));
    let day_six = drafter.draft_due().await.unwrap();
    clock.advance(Duration::days(1));
    let day_seven = drafter.draft_due().await.unwrap();
    finish(&pool, 2).await;
    clock.advance(Duration::days(14));
    let day_twenty_one = drafter.draft_due().await.unwrap();
    finish(&pool, 3).await;
    clock.advance(Duration::days(60));
    let after = drafter.draft_due().await.unwrap();

    assert_eq!(
        (while_first_open, day_six, day_seven, day_twenty_one, after),
        (0, 0, 40, 40, 0)
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e6_nothing_more_is_drafted_once_the_follow_up_has_stopped(pool: PgPool) {
    let clock = clock();
    practice(&pool, clock.clone()).await;
    let drafter = Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(GOOD_DRAFT))));
    drafter.draft_due().await.unwrap();
    finish(&pool, 1).await;
    sqlx::query("update follow_ups set stopped_at = now(), stop = 'replied'")
        .execute(&pool)
        .await
        .unwrap();

    clock.advance(Duration::days(7));

    assert_eq!(drafter.draft_due().await.unwrap(), 0);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn c7_a_blocked_draft_is_stored_with_each_check_that_failed(pool: PgPool) {
    let clock = clock();
    let family = practice(&pool, clock.clone()).await;
    let bad = "Hello {first_name}, your cholesterol test is due on Tuesday. Call {clinic_phone}. {opt_out_link}";
    Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(bad))))
        .draft_due()
        .await
        .unwrap();

    let drafts = Reminders::new(pool.clone()).latest_drafts(family).await.unwrap();

    assert_eq!(drafts.len(), 40);
    let checks: Vec<Check> = drafts[0].problems.iter().map(|p| p.check).collect();
    assert!(
        checks.contains(&Check::ClinicalWords) && checks.contains(&Check::NumbersAndDates),
        "{checks:?}"
    );
    assert!(drafts.iter().all(|d| d.blocked()));
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn c1_an_answer_that_cant_be_read_or_no_answer_blocks_the_draft_and_is_logged_with_its_cost(pool: PgPool) {
    let clock = clock();
    let family = practice(&pool, clock.clone()).await;
    Drafter::new(pool.clone(), clock.clone(), Some(Fake(None)))
        .draft_due()
        .await
        .unwrap();

    let drafts = Reminders::new(pool.clone()).latest_drafts(family).await.unwrap();

    assert!(
        drafts
            .iter()
            .all(|d| d.blocked() && d.problems[0].check == Check::Answer),
        "{:?}",
        drafts[0]
    );
    assert_eq!(
        count(&pool, "select count(*) from llm_calls where outcome = 'failed'").await,
        40
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn c1_with_no_model_the_clinics_template_is_drafted_and_passes(pool: PgPool) {
    let clock = clock();
    let family = practice(&pool, clock.clone()).await;
    Drafter::<Fake>::new(pool.clone(), clock.clone(), None)
        .draft_due()
        .await
        .unwrap();

    let drafts = Reminders::new(pool.clone()).latest_drafts(family).await.unwrap();

    assert!(
        drafts.iter().all(|d| !d.blocked() && d.source == "template"),
        "{:?}",
        drafts[0]
    );
}
