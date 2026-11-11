//! A4: one clinic never sees another's data: every query is scoped to the
//! clinic.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::records::{Records, Saved};
use common::{clinic, clock, records};
use sqlx::PgPool;

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn a4_each_clinic_sees_only_its_own_patients(pool: PgPool) {
    let family = clinic(&pool, "Example Family Practice", "family").await;
    let other = clinic(&pool, "Example Other Practice", "other").await;
    let store = Records::new(pool.clone(), clock());
    let all = records();
    for record in &all {
        store.save(family, record).await.unwrap();
    }
    store.save(other, &all[0]).await.unwrap();

    let ours = store.list(family).await.unwrap();
    let theirs = store.list(other).await.unwrap();

    assert_eq!(ours.len(), 25);
    assert_eq!(theirs.len(), 1);
    // The same patient in two clinics is two separate records.
    let shared = ours.iter().find(|p| p.fhir_id == theirs[0].fhir_id).unwrap();
    assert_ne!(shared.id, theirs[0].id);
    assert!(store.get(other, shared.id).await.unwrap().is_none());
    assert_eq!(store.get(family, shared.id).await.unwrap().unwrap(), all[0]);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn h2_fetching_the_same_record_again_updates_it_and_adds_nothing(pool: PgPool) {
    let family = clinic(&pool, "Example Family Practice", "family").await;
    let store = Records::new(pool.clone(), clock());
    let mut record = records().remove(0);
    assert_eq!(store.save(family, &record).await.unwrap(), Saved::New);
    assert_eq!(store.save(family, &record).await.unwrap(), Saved::Unchanged);

    record.appointments.clear();
    record.results.clear();
    let saved = store.save(family, &record).await.unwrap();

    assert_eq!(saved, Saved::Updated);
    let [patient] = store.list(family).await.unwrap().try_into().unwrap();
    assert_eq!(store.get(family, patient.id).await.unwrap().unwrap(), record);
}
