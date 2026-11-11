//! H1: logging in works only through a fresh emailed link, and every page
//! and API needs a login.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::review::Reviews;
use common::{app, call, clinic, clock, log_in, login_token};
use serde_json::json;
use sqlx::PgPool;

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn h1_a_login_link_logs_in_once(pool: PgPool) {
    clinic(&pool, "Example Family Practice", "family").await;
    let app = app(&pool, clock());

    let cookie = log_in(&app, &pool, "admin@family.test").await;

    let me = call(&app, "GET", "/api/me", None, Some(&cookie)).await;
    assert_eq!(me.status, 200);
    assert_eq!(
        (
            me.body["email"].as_str(),
            me.body["role"].as_str(),
            me.body["clinic_name"].as_str()
        ),
        (
            Some("admin@family.test"),
            Some("administrator"),
            Some("Example Family Practice")
        )
    );
    let token = login_token(&pool, "admin@family.test").await.unwrap();
    let again = call(&app, "POST", &format!("/api/login/{token}"), None, None).await;
    assert_eq!(again.status, 422, "used once only");
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn h1_a_link_older_than_fifteen_minutes_is_refused(pool: PgPool) {
    clinic(&pool, "Example Family Practice", "family").await;
    let clock = clock();
    let app = app(&pool, clock.clone());
    call(
        &app,
        "POST",
        "/api/login",
        Some(json!({ "email": "admin@family.test" })),
        None,
    )
    .await;
    let token = login_token(&pool, "admin@family.test").await.unwrap();

    clock.advance(time::Duration::minutes(16));

    assert_eq!(
        call(&app, "POST", &format!("/api/login/{token}"), None, None)
            .await
            .status,
        422
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn h1_the_coordinator_logs_in_as_a_coordinator(pool: PgPool) {
    let family = clinic(&pool, "Example Family Practice", "family").await;
    Reviews::new(pool.clone(), clock())
        .add_coordinator(family, "carla@family.test", "Carla Reyes", "admin@family.test")
        .await
        .unwrap();
    let app = app(&pool, clock());

    let cookie = log_in(&app, &pool, "carla@family.test").await;

    let me = call(&app, "GET", "/api/me", None, Some(&cookie)).await;
    assert_eq!(
        (me.body["role"].as_str(), me.body["name"].as_str()),
        (Some("coordinator"), Some("Carla Reyes"))
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn h1_every_api_needs_a_login(pool: PgPool) {
    let app = app(&pool, clock());
    let id = uuid::Uuid::now_v7();
    for (method, path) in [
        ("GET", "/api/me".to_owned()),
        ("GET", "/api/queue".to_owned()),
        ("GET", format!("/api/reminders/{id}")),
        ("POST", format!("/api/reminders/{id}/approve")),
        ("POST", format!("/api/reminders/{id}/edit")),
        ("POST", format!("/api/reminders/{id}/template")),
        ("POST", format!("/api/reminders/{id}/redraft")),
        ("POST", format!("/api/reminders/{id}/skip")),
        ("GET", format!("/api/follow-ups/{id}/timeline")),
        ("GET", "/api/activity".to_owned()),
        ("POST", "/api/fhir-server/fetch".to_owned()),
    ] {
        let answer = call(&app, method, &path, Some(json!({})), None).await;
        assert_eq!(answer.status, 401, "{method} {path}");
    }
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn h1_a_post_from_another_site_is_refused(pool: PgPool) {
    use tower::ServiceExt;
    let app = app(&pool, clock());
    let request = axum::http::Request::post("/api/login")
        .header("origin", "https://elsewhere.example.test")
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({ "email": "admin@family.test" }).to_string(),
        ))
        .unwrap();

    let answer = app.oneshot(request).await.unwrap();

    assert_eq!(answer.status().as_u16(), 403);
}
