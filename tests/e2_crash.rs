//! E2: a crash during sending never sends a reminder twice. The real binary
//! is killed after the mail provider accepted the email but before the send
//! was recorded, then run again. Needs `--features failpoints`.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use std::process::{Command, ExitStatus};

use care_reminders::engine::accounts::{Role, Session};
use care_reminders::reminders::{Drafter, Reminders};
use care_reminders::review::{Reviews, content_hash};
use common::{Fake, clock, practice};
use serde_json::json;
use sqlx::PgPool;
use time::Duration;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const FAILPOINT: &str = "after_send_before_record";
/// Tuesday 10 November, 10 am in New York: inside the sending hours.
const NOW: &str = "2026-11-10T15:00:00Z";

async fn database_url(pool: &PgPool) -> String {
    let name: String = sqlx::query_scalar("select current_database()")
        .fetch_one(pool)
        .await
        .unwrap();
    let base = std::env::var("DATABASE_URL").unwrap();
    format!("{}/{name}", base.rsplit_once('/').unwrap().0)
}

/// Runs `care-reminders outbox-drain` against this test's database.
async fn drain(pool: &PgPool, mailpit: &MockServer, failpoint: Option<&str>) -> ExitStatus {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_care-reminders"));
    cmd.arg("outbox-drain")
        .env("DATABASE_URL", database_url(pool).await)
        .env("MAILPIT_URL", mailpit.uri())
        .env("CARE_REMINDERS_NOW", NOW)
        .env("RUST_LOG", "warn");
    match failpoint {
        Some(name) => cmd.env("NUDGEWRIGHT_FAILPOINT", name),
        None => cmd.env_remove("NUDGEWRIGHT_FAILPOINT"),
    };
    tokio::task::spawn_blocking(move || cmd.status().unwrap())
        .await
        .unwrap()
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn e2_a_crash_during_sending_never_sends_a_reminder_twice(pool: PgPool) {
    let mailpit = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/send"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "ID": "mp_1" })))
        .mount(&mailpit)
        .await;
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
    let reviews = Reviews::new(pool.clone(), clock).with_sending(common::sending());
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
    let draft = Reminders::new(pool.clone())
        .latest_drafts(family)
        .await
        .unwrap()
        .remove(0);
    reviews
        .approve(&carla, draft.reminder, draft.draft, &content_hash(&draft.text))
        .await
        .unwrap();

    let crashed = drain(&pool, &mailpit, Some(FAILPOINT)).await;
    assert!(!crashed.success(), "the failpoint should have killed the process");
    drain(&pool, &mailpit, None).await;

    assert_eq!(
        mailpit.received_requests().await.unwrap().len(),
        1,
        "the reminder was sent twice"
    );
    let status: String = sqlx::query_scalar("select status from outbox_messages where reply_to is not null")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        status, "unconfirmed",
        "an unknown outcome is never retried on a provider that can't block duplicates"
    );
}
