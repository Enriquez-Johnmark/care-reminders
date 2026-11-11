//! G1–G3: every step is in an append-only log; each follow-up has its own
//! timeline, and the clinic-wide log filters by person and step.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::activity::Activity;
use care_reminders::engine::accounts::{Role, Session};
use care_reminders::follow_ups::FollowUps;
use care_reminders::reminders::{Drafter, Reminders};
use care_reminders::review::{Reviews, content_hash};
use common::{Fake, GOOD_DRAFT, clock, practice};
use sqlx::PgPool;

/// The practice with its first reminders drafted, and its coordinator, who
/// has approved one reminder, edited a second and skipped a third.
async fn reviewed(pool: &PgPool) -> (Session, Vec<care_reminders::reminders::LatestDraft>) {
    let clock = clock();
    let family = practice(pool, clock.clone()).await;
    Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(GOOD_DRAFT))))
        .draft_due()
        .await
        .unwrap();
    care_reminders::settings::set_phone(pool, family, "(555) 010-0100")
        .await
        .unwrap();
    let reviews = Reviews::new(pool.clone(), clock);
    let member = reviews
        .add_coordinator(family, "carla@family.test", "Carla Reyes", "admin@family.test")
        .await
        .unwrap();
    let carla = Session {
        tenant: family,
        member,
        email: "carla@family.test".into(),
        role: Role::Staff,
    };
    let mut drafts = Reminders::new(pool.clone()).latest_drafts(family).await.unwrap();
    drafts.sort_by_key(|d| d.reminder.as_uuid());
    reviews
        .approve(
            &carla,
            drafts[0].reminder,
            drafts[0].draft,
            &content_hash(&drafts[0].text),
        )
        .await
        .unwrap();
    reviews.edit(&carla, drafts[1].reminder, GOOD_DRAFT).await.unwrap();
    reviews
        .skip(&carla, drafts[2].reminder, "Called instead")
        .await
        .unwrap();
    (carla, drafts)
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn g1_each_step_so_far_is_logged_with_who_and_when(pool: PgPool) {
    let (carla, _) = reviewed(&pool).await;

    let log = Activity::new(pool.clone())
        .clinic_log(carla.tenant, None, None)
        .await
        .unwrap();

    for (action, actor) in [
        ("follow_up.selected", "operator"),
        ("reminder.drafted", "model"),
        ("reminder.checked", "system"),
        ("reminder.approved", "carla@family.test"),
        ("reminder.edited", "carla@family.test"),
        ("reminder.skipped", "carla@family.test"),
    ] {
        assert!(
            log.iter().any(|e| e.action == action && e.actor == actor),
            "{action} by {actor}"
        );
    }
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn g2_the_database_refuses_to_change_or_delete_a_log_entry(pool: PgPool) {
    reviewed(&pool).await;

    let changed = sqlx::query("update audit_events set actor = 'someone else'")
        .execute(&pool)
        .await;
    let deleted = sqlx::query("delete from audit_events").execute(&pool).await;

    assert!(changed.is_err() && deleted.is_err());
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn g3_a_follow_ups_timeline_lists_its_steps_in_order(pool: PgPool) {
    let (carla, drafts) = reviewed(&pool).await;
    let follow_up: uuid::Uuid = sqlx::query_scalar("select follow_up_id from reminders where id = $1")
        .bind(drafts[0].reminder.as_uuid())
        .fetch_one(&pool)
        .await
        .unwrap();

    let timeline = Activity::new(pool.clone())
        .timeline(carla.tenant, follow_up.into())
        .await
        .unwrap();

    let actions: Vec<&str> = timeline.iter().map(|e| e.action.as_str()).collect();
    assert_eq!(
        actions,
        [
            "follow_up.selected",
            "reminder.drafted",
            "reminder.checked",
            "reminder.approved"
        ]
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn g3_the_clinic_wide_log_filters_by_person_and_by_step(pool: PgPool) {
    let (carla, _) = reviewed(&pool).await;
    let activity = Activity::new(pool.clone());

    let by_carla = activity
        .clinic_log(carla.tenant, Some("carla@family.test"), None)
        .await
        .unwrap();
    let skips = activity
        .clinic_log(carla.tenant, None, Some("reminder.skipped"))
        .await
        .unwrap();

    // Approved, edited (also logged as her draft) and skipped.
    assert_eq!(by_carla.len(), 4, "{by_carla:?}");
    assert!(by_carla.iter().all(|e| e.actor == "carla@family.test"));
    assert_eq!(skips.len(), 1);
    assert_eq!(skips[0].details["reason"], "Called instead");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn a4_a_clinic_never_sees_another_clinics_activity(pool: PgPool) {
    let (carla, _) = reviewed(&pool).await;
    let other = common::clinic(&pool, "Example Other Practice", "other").await;
    FollowUps::new(pool.clone(), clock())
        .refresh(other, "operator")
        .await
        .unwrap();

    let theirs = Activity::new(pool.clone()).clinic_log(other, None, None).await.unwrap();

    assert!(theirs.iter().all(|e| e.actor != carla.email), "{theirs:?}");
    assert!(theirs.is_empty(), "{theirs:?}");
}
