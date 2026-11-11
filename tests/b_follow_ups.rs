//! B2–B5: follow-ups chosen by the clinic's rules from the fetched records,
//! with their reason and rule version, never twice, stopped when the patient
//! books, and never by the model.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use std::sync::Arc;

use care_reminders::engine::clock::FixedClock;
use care_reminders::fhir::{Appointment, AppointmentStatus};
use care_reminders::follow_ups::{FollowUps, Stop};
use care_reminders::records::{ClinicId, Records};
use care_reminders::rules::{RuleSettings, Rules};
use common::{clinic, clock, records};
use sqlx::PgPool;
use time::macros::datetime;

/// Example Family Practice with its default rules and the made-up patients.
async fn practice(pool: &PgPool, clock: Arc<FixedClock>) -> ClinicId {
    let family = clinic(pool, "Example Family Practice", "family").await;
    Rules::new(pool.clone(), clock.clone())
        .install_defaults(family, "operator")
        .await
        .unwrap();
    let store = Records::new(pool.clone(), clock);
    for record in records() {
        store.save(family, &record).await.unwrap();
    }
    family
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn b4_choosing_again_never_creates_the_same_follow_up_twice(pool: PgPool) {
    let family = practice(&pool, clock()).await;
    let follow_ups = FollowUps::new(pool.clone(), clock());

    let first = follow_ups.refresh(family, "operator").await.unwrap();
    let second = follow_ups.refresh(family, "operator").await.unwrap();

    assert_eq!((first.selected, second.selected), (40, 0));
    assert_eq!(follow_ups.list(family).await.unwrap().len(), 40);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn b2_each_follow_up_shows_its_reason_and_the_rule_version_that_selected_it(pool: PgPool) {
    let clock = clock();
    let family = practice(&pool, clock.clone()).await;
    let follow_ups = FollowUps::new(pool.clone(), clock.clone());
    follow_ups.refresh(family, "operator").await.unwrap();
    let rules = Rules::new(pool.clone(), clock.clone());
    let discharge = rules
        .current(family)
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.name.contains("discharge"))
        .unwrap();
    let wider = RuleSettings::AfterDischarge {
        within_days: 7,
        look_back_days: 60,
    };
    rules
        .change(family, discharge.id, &wider, "admin@family.test")
        .await
        .unwrap();

    follow_ups.refresh(family, "operator").await.unwrap();

    let list = follow_ups.list(family).await.unwrap();
    let dan = list
        .iter()
        .find(|f| f.patient_name.starts_with("Dan ") && f.rule_name.contains("discharge"))
        .unwrap();
    assert_eq!(dan.reason, "Discharged from hospital on 2026-11-07");
    assert_eq!(
        dan.rule_version, 1,
        "an earlier follow-up keeps the version that chose it"
    );
    // The wider look-back now reaches Leo, discharged 40 days ago.
    let leo = list
        .iter()
        .find(|f| f.patient_name.starts_with("Leo ") && f.rule_name.contains("discharge"))
        .unwrap();
    assert_eq!(leo.rule_version, 2);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn b3_a_patient_who_books_later_is_stopped_at_the_next_fetch(pool: PgPool) {
    let clock = clock();
    let family = practice(&pool, clock.clone()).await;
    let follow_ups = FollowUps::new(pool.clone(), clock.clone());
    follow_ups.refresh(family, "operator").await.unwrap();
    let mut ana = records().remove(0);
    ana.appointments.push(Appointment {
        fhir_id: "p01-appointment-9".into(),
        status: AppointmentStatus::Booked,
        kind: "FOLLOWUP".into(),
        start: datetime!(2026-11-20 09:30 UTC),
    });
    Records::new(pool.clone(), clock.clone())
        .save(family, &ana)
        .await
        .unwrap();

    let refreshed = follow_ups.refresh(family, "operator").await.unwrap();

    assert_eq!(refreshed.booked, 1);
    let anas: Vec<_> = follow_ups
        .list(family)
        .await
        .unwrap()
        .into_iter()
        .filter(|f| f.patient_name.starts_with("Ana "))
        .collect();
    let stops: Vec<(String, Option<Stop>)> = anas.iter().map(|f| (f.rule_name.clone(), f.stopped)).collect();
    assert!(
        stops.contains(&("Follow-up after an abnormal result".into(), Some(Stop::Booked))),
        "{stops:?}"
    );
    assert!(
        stops.contains(&("Colorectal screening recall".into(), None)),
        "a follow-up visit isn't a screening: {stops:?}"
    );
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn b5_choosing_who_is_due_works_with_no_model_and_the_code_never_names_one(pool: PgPool) {
    let family = practice(&pool, clock()).await;

    // `FollowUps` takes no model at all: there is nothing to configure.
    let chosen = FollowUps::new(pool.clone(), clock())
        .refresh(family, "operator")
        .await
        .unwrap();

    assert_eq!(chosen.selected, 40);
    for file in ["src/rules.rs", "src/follow_ups.rs"] {
        let code = std::fs::read_to_string(format!("{}/{file}", env!("CARGO_MANIFEST_DIR"))).unwrap();
        for model in ["llm", "LanguageModel", "drafting"] {
            assert!(!code.contains(model), "{file} names {model}");
        }
    }
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn b2_a_patient_whose_record_isnt_current_is_not_chosen(pool: PgPool) {
    let family = practice(&pool, clock()).await;
    Records::new(pool.clone(), clock())
        .mark_not_current(family, "p01", "not on the FHIR server at the last fetch")
        .await
        .unwrap();

    let chosen = FollowUps::new(pool.clone(), clock())
        .refresh(family, "operator")
        .await
        .unwrap();

    // Ana had two follow-ups (a result and colorectal screening).
    assert_eq!(chosen.selected, 38);
}

#[sqlx::test(migrator = "care_reminders::MIGRATOR")]
async fn a4_a_clinic_sees_only_its_own_follow_ups(pool: PgPool) {
    let family = practice(&pool, clock()).await;
    let other = clinic(&pool, "Example Other Practice", "other").await;
    FollowUps::new(pool.clone(), clock())
        .refresh(family, "operator")
        .await
        .unwrap();

    assert!(
        FollowUps::new(pool.clone(), clock())
            .list(other)
            .await
            .unwrap()
            .is_empty()
    );
}
