//! The review pages' API: the queue, one reminder, the coordinator's
//! actions, and the administrator's activity log, each clinic only its own.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::reminders::Drafter;
use care_reminders::review::Reviews;
use common::{Fake, GOOD_DRAFT, app, call, clock, log_in, practice};
use serde_json::{Value, json};
use sqlx::PgPool;

struct Setup {
    pool: PgPool,
    app: axum::Router,
    coordinator: String,
    administrator: String,
}

async fn setup(pool: PgPool) -> Setup {
    let clock = clock();
    let family = practice(&pool, clock.clone()).await;
    care_reminders::settings::set_phone(&pool, family, "(555) 010-0100")
        .await
        .unwrap();
    Drafter::new(pool.clone(), clock.clone(), Some(Fake(Some(GOOD_DRAFT))))
        .draft_due()
        .await
        .unwrap();
    Reviews::new(pool.clone(), clock.clone())
        .add_coordinator(family, "carla@family.test", "Carla Reyes", "admin@family.test")
        .await
        .unwrap();
    let app = app(&pool, clock);
    let coordinator = log_in(&app, &pool, "carla@family.test").await;
    let administrator = log_in(&app, &pool, "admin@family.test").await;
    Setup {
        pool,
        app,
        coordinator,
        administrator,
    }
}

impl Setup {
    async fn get(&self, path: &str, who: &str) -> Value {
        call(&self.app, "GET", path, None, Some(who)).await.body
    }
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d1_the_queue_lists_each_reminder_waiting_and_the_review_page_shows_the_text_filled(pool: PgPool) {
    let s = setup(pool).await;

    let queue = s.get("/api/queue", &s.coordinator).await;
    let first = &queue[0];
    let detail = s
        .get(
            &format!("/api/reminders/{}", first["id"].as_str().unwrap()),
            &s.coordinator,
        )
        .await;

    assert_eq!(queue.as_array().unwrap().len(), 40);
    assert_eq!(first["status"], "waiting");
    assert!(
        queue
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["contact_another_way"] == true),
        "Jun asked not to be emailed"
    );
    let preview = detail["preview"].as_str().unwrap();
    assert!(
        preview.contains("(555) 010-0100") && preview.contains("Example Family Practice"),
        "{preview}"
    );
    assert!(!preview.contains('{'), "{preview}");
    assert_eq!(detail["draft"]["problems"], json!([]));
    // This app has no model, so there are no new drafts to ask for.
    assert_eq!(detail["new_drafts_left"], 0);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d1_the_administrator_cant_approve_and_the_coordinator_can(pool: PgPool) {
    let s = setup(pool).await;
    let id = s.get("/api/queue", &s.coordinator).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let detail = s.get(&format!("/api/reminders/{id}"), &s.coordinator).await;
    let body = json!({ "draft_id": detail["draft"]["id"], "content_hash": detail["draft"]["content_hash"] });
    let path = format!("/api/reminders/{id}/approve");

    let by_admin = call(&s.app, "POST", &path, Some(body.clone()), Some(&s.administrator)).await;
    let by_coordinator = call(&s.app, "POST", &path, Some(body), Some(&s.coordinator)).await;

    assert_eq!((by_admin.status, by_coordinator.status), (403, 204));
    let after = s.get(&format!("/api/reminders/{id}"), &s.coordinator).await;
    assert!(after["approved_at"].is_string());
    let queue = s.get("/api/queue", &s.coordinator).await;
    assert_eq!(queue.as_array().unwrap().last().unwrap()["status"], "approved");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn d4_an_edit_through_the_api_is_checked_and_says_why_it_was_blocked(pool: PgPool) {
    let s = setup(pool).await;
    let id = s.get("/api/queue", &s.coordinator).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let edited = call(
        &s.app,
        "POST",
        &format!("/api/reminders/{id}/edit"),
        Some(json!({ "text": format!("{GOOD_DRAFT} It's about your cholesterol.") })),
        Some(&s.coordinator),
    )
    .await;

    assert_eq!(edited.status, 200);
    assert_eq!(edited.body["blocked"], true);
    assert_eq!(edited.body["problems"][0]["check"], "clinical-words");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn g3_only_the_administrator_reads_the_clinic_wide_activity(pool: PgPool) {
    let s = setup(pool).await;

    let by_coordinator = call(&s.app, "GET", "/api/activity", None, Some(&s.coordinator)).await;
    let by_admin = call(
        &s.app,
        "GET",
        "/api/activity?action=follow_up.selected",
        None,
        Some(&s.administrator),
    )
    .await;

    assert_eq!(by_coordinator.status, 403);
    assert_eq!(by_admin.body.as_array().unwrap().len(), 40);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn a4_another_clinics_reminder_is_not_found_through_the_api(pool: PgPool) {
    let s = setup(pool).await;
    let id = s.get("/api/queue", &s.coordinator).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let follow_up = s.get("/api/queue", &s.coordinator).await[0]["follow_up"]
        .as_str()
        .unwrap()
        .to_owned();
    common::clinic(&s.pool, "Example Other Practice", "other").await;
    let outsider = log_in(&s.app, &s.pool, "admin@other.test").await;

    for path in [
        format!("/api/reminders/{id}"),
        format!("/api/follow-ups/{follow_up}/timeline"),
    ] {
        assert_eq!(
            call(&s.app, "GET", &path, None, Some(&outsider)).await.status,
            404,
            "{path}"
        );
    }
    let skipped = call(
        &s.app,
        "POST",
        &format!("/api/reminders/{id}/skip"),
        Some(json!({ "reason": "x" })),
        Some(&outsider),
    )
    .await;
    assert_eq!(skipped.status, 404);
    assert!(s.get("/api/queue", &outsider).await.as_array().unwrap().is_empty());
}
