//! What several test files share: a fixed clock, clinics and the made-up
//! patients' records.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]
#![allow(dead_code, reason = "each test file uses part of this")]

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
    family
}
