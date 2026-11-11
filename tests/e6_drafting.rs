//! E6, and C1 and C7 end to end: reminders are drafted on their planned days
//! (0, 7 and 21), each only once the one before is finished and only while
//! the follow-up hasn't stopped; every draft is checked and stored with its
//! problems.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::checks::Check;
use care_reminders::engine::clock::Clock;
use care_reminders::reminders::{Drafter, Reminders};
use common::{Fake, GOOD_DRAFT, clock, practice};
use sqlx::PgPool;
use time::Duration;

/// Marks every reminder `number` as finished (skipped) at `at`, as the
/// coordinator or sending will.
async fn finish(pool: &PgPool, number: i16, at: time::OffsetDateTime) {
    sqlx::query("update reminders set finished_at = $2, outcome = 'skipped' where number = $1")
        .bind(number)
        .bind(at)
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
    // Reminder 1 went out on day 0.
    finish(&pool, 1, clock.now() - Duration::days(7)).await;
    clock.advance(Duration::days(-1));
    let day_six = drafter.draft_due().await.unwrap();
    clock.advance(Duration::days(1));
    let day_seven = drafter.draft_due().await.unwrap();
    finish(&pool, 2, clock.now()).await;
    clock.advance(Duration::days(13));
    let day_twenty = drafter.draft_due().await.unwrap();
    clock.advance(Duration::days(1));
    let day_twenty_one = drafter.draft_due().await.unwrap();
    finish(&pool, 3, clock.now()).await;
    clock.advance(Duration::days(60));
    let after = drafter.draft_due().await.unwrap();

    assert_eq!(
        (while_first_open, day_six, day_seven, day_twenty, day_twenty_one, after),
        (0, 0, 40, 0, 40, 0)
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e6_nothing_more_is_drafted_once_the_follow_up_has_stopped(pool: PgPool) {
    let clock = clock();
    practice(&pool, clock.clone()).await;
    let drafter = Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(GOOD_DRAFT))));
    drafter.draft_due().await.unwrap();
    finish(&pool, 1, clock.now()).await;
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
async fn c1_an_answer_that_cant_be_read_blocks_the_draft_and_is_logged_with_its_cost(pool: PgPool) {
    let clock = clock();
    let family = practice(&pool, clock.clone()).await;
    Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(""))))
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
        count(&pool, "select count(*) from llm_calls where outcome = 'invalid'").await,
        40
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn c1_when_the_model_doesnt_answer_the_clinics_template_stands_in_with_a_warning(pool: PgPool) {
    let clock = clock();
    let family = practice(&pool, clock.clone()).await;
    Drafter::new(pool.clone(), clock.clone(), Some(Fake(None)))
        .draft_due()
        .await
        .unwrap();

    let drafts = Reminders::new(pool.clone()).latest_drafts(family).await.unwrap();

    assert!(
        drafts.iter().all(|d| !d.blocked() && d.source == "template"),
        "{:?}",
        drafts[0]
    );
    assert!(
        drafts[0].warnings.iter().any(|w| w.contains("didn't answer")),
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

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e6_a_reminder_finished_late_keeps_the_gap_before_the_next(pool: PgPool) {
    let clock = clock();
    practice(&pool, clock.clone()).await;
    let drafter = Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(GOOD_DRAFT))));
    drafter.draft_due().await.unwrap();
    clock.advance(Duration::days(25));
    sqlx::query("update reminders set finished_at = $1, outcome = 'skipped'")
        .bind(clock.now())
        .execute(&pool)
        .await
        .unwrap();

    clock.advance(Duration::days(6));
    let six_days_on = drafter.draft_due().await.unwrap();
    clock.advance(Duration::days(1));
    let seven_days_on = drafter.draft_due().await.unwrap();

    assert_eq!((six_days_on, seven_days_on), (0, 40));
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e6_planned_days_are_the_clinics_own_days(pool: PgPool) {
    // 9 am on 10 November in Manila is 1 am in UTC.
    let clock = std::sync::Arc::new(care_reminders::engine::clock::FixedClock::new(
        time::macros::datetime!(2026-11-10 01:00 UTC),
    ));
    let family = common::clinic(&pool, "Example Family Practice", "family").await;
    care_reminders::settings::set_time_zone(&pool, family, "Asia/Manila")
        .await
        .unwrap();
    common::practice_in(&pool, clock.clone(), family).await;
    let drafter = Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(GOOD_DRAFT))));
    drafter.draft_due().await.unwrap();
    finish(&pool, 1, clock.now()).await;

    // 1 am on 17 November in Manila, still 16 November in UTC.
    clock.advance(Duration::days(6) + Duration::hours(16));
    let day_seven_in_manila = drafter.draft_due().await.unwrap();

    assert_eq!(day_seven_in_manila, 40);
}

#[test]
fn e6_an_unknown_time_zone_is_refused() {
    assert!(care_reminders::settings::time_zone("Mars/Olympus").is_err());
    assert!(care_reminders::settings::time_zone("Asia/Manila").is_ok());
}
