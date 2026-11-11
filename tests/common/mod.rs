//! What several test files share: a fixed clock, clinics and the made-up
//! patients' records.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]
#![allow(dead_code, reason = "each test file uses part of this")]

pub mod outbox;

use std::sync::Arc;

use care_reminders::demo_data::{self, DEMO_DAY};
use care_reminders::engine::accounts::Accounts;
use care_reminders::engine::clock::FixedClock;
use care_reminders::fhir::PatientRecord;
use care_reminders::records::ClinicId;
use sqlx::PgPool;
use time::macros::datetime;

/// The time every test starts at: 9 am on the demo day.
pub fn clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(datetime!(2026-11-10 09:00 UTC)))
}

/// A clinic owned by `admin@<slug>.test`.
pub async fn clinic(pool: &PgPool, name: &str, slug: &str) -> ClinicId {
    Accounts::new(pool.clone(), clock())
        .create(name, &format!("admin@{slug}.test"))
        .await
        .unwrap()
}

/// The made-up patients' records, made for the demo day.
pub fn records() -> Vec<PatientRecord> {
    demo_data::generate(DEMO_DAY)
        .iter()
        .map(|file| PatientRecord::read(&file.json).unwrap())
        .collect()
}

use care_reminders::engine::llm::{LanguageModel, LlmError, LlmReply, LlmRequest};
use care_reminders::follow_ups::FollowUps;
use care_reminders::records::Records;
use care_reminders::rules::Rules;

/// A stand-in for Claude: answers every request with `text`, as the JSON
/// asked for, or fails when it is `None`.
pub struct Fake(pub Option<&'static str>);

/// A good reminder draft, as a model might write it.
pub const GOOD_DRAFT: &str = "Hello {first_name}, this is {clinic_name}. It's time to book a follow-up visit \
                              with us. Please call {clinic_phone} by {due_by}. To stop these messages, go here: \
                              {opt_out_link}";

impl LanguageModel for Fake {
    fn model(&self) -> &str {
        "claude-test"
    }

    async fn complete(&self, _request: &LlmRequest) -> Result<LlmReply, LlmError> {
        match self.0 {
            Some(text) => Ok(LlmReply {
                text: serde_json::json!({ "text": text }).to_string(),
                model: "claude-test".into(),
                request_id: Some("req_test".into()),
                input_tokens: 300,
                output_tokens: 60,
            }),
            None => Err(LlmError::Unavailable("the test model is off".into())),
        }
    }
}

/// Example Family Practice with its default rules, the made-up patients and
/// their 40 follow-ups chosen.
pub async fn practice(pool: &PgPool, clock: Arc<FixedClock>) -> ClinicId {
    let family = clinic(pool, "Example Family Practice", "family").await;
    practice_in(pool, clock, family).await;
    family
}

/// [`practice`]'s rules, patients and follow-ups, in the clinic `family`.
pub async fn practice_in(pool: &PgPool, clock: Arc<FixedClock>, family: ClinicId) {
    Rules::new(pool.clone(), clock.clone())
        .install_defaults(family, "operator")
        .await
        .unwrap();
    let store = Records::new(pool.clone(), clock.clone());
    for record in records() {
        store.save(family, &record).await.unwrap();
    }
    FollowUps::new(pool.clone(), clock)
        .refresh(family, "operator")
        .await
        .unwrap();
}

use serde_json::{Value, json};

/// Where the app thinks it is served; posts must come from here.
pub const BASE_URL: &str = "http://reminders.example.test";

/// The web app on `pool`, drafting with `model` when asked for a new draft.
pub fn app(pool: &PgPool, clock: Arc<FixedClock>) -> axum::Router {
    let state = care_reminders::web::AppState::new(pool.clone(), clock, BASE_URL, sending())
        .with_inbound_secret(INBOUND_SECRET);
    care_reminders::web::router(state)
}

/// An API answer.
pub struct Answer {
    pub status: u16,
    pub cookie: Option<String>,
    pub body: Value,
}

/// Sends `method path` with an optional JSON body and cookie, from our origin.
pub async fn call(app: &axum::Router, method: &str, path: &str, body: Option<Value>, cookie: Option<&str>) -> Answer {
    use tower::ServiceExt;
    let mut request = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header("origin", BASE_URL);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    let request = match body {
        Some(body) => request
            .header("content-type", "application/json")
            .body(axum::body::Body::from(body.to_string())),
        None => request.body(axum::body::Body::empty()),
    }
    .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let cookie = response
        .headers()
        .get("set-cookie")
        .map(|value| value.to_str().unwrap().split(';').next().unwrap().to_owned());
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 22).await.unwrap();
    Answer {
        status,
        cookie,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

/// The token in the newest login email to `email`, waiting up to two seconds
/// (the email is created after the answer).
pub async fn login_token(pool: &PgPool, email: &str) -> Option<String> {
    for _ in 0..40 {
        let body: Option<String> = sqlx::query_scalar(
            "select body_text from outbox_messages where lower(to_address) = lower($1) and body_text like '%/login/%'
             order by created_at desc limit 1",
        )
        .bind(email)
        .fetch_optional(pool)
        .await
        .unwrap();
        if let Some(body) = body {
            let link = body.split_whitespace().find(|word| word.contains("/login/"))?;
            return Some(link.rsplit('/').next()?.to_owned());
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    None
}

/// Logs in as `email` and returns the session cookie.
pub async fn log_in(app: &axum::Router, pool: &PgPool, email: &str) -> String {
    call(app, "POST", "/api/login", Some(json!({ "email": email })), None).await;
    let token = login_token(pool, email).await.unwrap();
    call(app, "POST", &format!("/api/login/{token}"), None, None)
        .await
        .cookie
        .unwrap()
}

/// A test signing key, never used anywhere real.
pub fn master() -> care_reminders::engine::files::MasterKey {
    care_reminders::engine::files::MasterKey::from_hex(&"ab".repeat(32)).unwrap()
}

/// Reminder links signed with the test key, under [`BASE_URL`].
pub fn sending() -> care_reminders::sending::Sending {
    care_reminders::sending::Sending::new(&master(), BASE_URL, "replies.example.test")
}

/// Fetches a server-rendered page, from our origin: its status and HTML.
pub async fn fetch(app: &axum::Router, method: &str, path: &str) -> (u16, String) {
    use tower::ServiceExt;
    let request = axum::http::Request::builder()
        .method(method)
        .uri(path)
        .header("origin", BASE_URL)
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = axum::body::to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

/// The test inbound provider's signing secret.
pub const INBOUND_SECRET: &str = "test-inbound-secret";

/// Posts `raw` to the reply webhook, signed with `secret`; the status.
pub async fn post_inbound(app: &axum::Router, raw: &str, secret: &str) -> u16 {
    use hmac::{Hmac, KeyInit, Mac};
    use tower::ServiceExt;
    let signature: String = <Hmac<sha2::Sha256> as KeyInit>::new_from_slice(secret.as_bytes())
        .unwrap()
        .chain_update(raw.as_bytes())
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let request = axum::http::Request::builder()
        .method("POST")
        .uri("/hooks/inbound-email")
        .header("x-care-reminders-signature", signature)
        .body(axum::body::Body::from(raw.to_owned()))
        .unwrap();
    app.clone().oneshot(request).await.unwrap().status().as_u16()
}
