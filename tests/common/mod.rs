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
