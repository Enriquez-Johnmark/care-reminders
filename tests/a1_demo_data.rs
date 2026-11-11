//! A1: the generator writes the same made-up data every time, valid FHIR R4,
//! each resource marked as made up, with the planted tricky cases.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use std::path::Path;

use care_reminders::demo_data::{self, DEMO_DAY};
use care_reminders::fhir::{AppointmentStatus, Kind, PatientRecord};
use time::macros::date;

fn records() -> Vec<PatientRecord> {
    demo_data::generate(DEMO_DAY)
        .iter()
        .map(|file| PatientRecord::read(&file.json).unwrap())
        .collect()
}

#[test]
fn a1_the_same_day_gives_the_same_files_every_time() {
    assert_eq!(demo_data::generate(DEMO_DAY), demo_data::generate(DEMO_DAY));
}

#[test]
fn a1_twenty_five_made_up_patients_each_readable_and_named_as_made_up() {
    let records = records();

    assert_eq!(records.len(), 25);
    assert!(
        records.iter().all(|r| r.patient.name.ends_with("(made up)")),
        "{records:?}"
    );
    let emails = records.iter().filter_map(|r| r.patient.email.as_deref());
    assert!(emails.into_iter().all(|e| e.ends_with("@example.test")));
}

#[test]
fn a1_every_resource_is_marked_as_test_data() {
    let dir = tempfile::tempdir().unwrap();
    for file in demo_data::generate(DEMO_DAY) {
        std::fs::write(dir.path().join(&file.name), &file.json).unwrap();
    }

    assert_eq!(care_reminders::made_up::problems(dir.path()), Vec::<String>::new());
}

#[test]
fn a1_the_tricky_cases_are_planted() {
    let records = records();
    let one = |what: &str, found: &dyn Fn(&PatientRecord) -> bool| {
        assert_eq!(records.iter().filter(|r| found(r)).count(), 1, "{what}");
    };

    one("already booked", &|r| {
        r.results.iter().any(|x| x.abnormal)
            && r.appointments
                .iter()
                .any(|a| a.status == AppointmentStatus::Booked && a.kind == "FOLLOWUP")
    });
    one("opted out", &|r| !r.patient.opted_out.is_empty());
    one("asked not to be emailed", &|r| r.patient.no_email);
    one("a sensitive reason", &|r| {
        r.results
            .iter()
            .any(|x| x.code == demo_data::SENSITIVE_TEST && x.abnormal)
    });
    one("discharged too long ago", &|r| {
        r.stays
            .iter()
            .any(|s| s.discharged.is_some_and(|d| (DEMO_DAY - d).whole_days() > 30))
    });
    assert_eq!(
        records.iter().flat_map(|r| &r.patient.opted_out).collect::<Vec<_>>(),
        [&Kind::ScreeningRecall]
    );
}

#[test]
fn a1_dates_follow_the_day_the_data_is_made_for() {
    let later = date!(2027 - 01 - 15);
    let shift = later - DEMO_DAY;
    let (then, now) = (records(), demo_data::generate(later));
    let now: Vec<PatientRecord> = now.iter().map(|f| PatientRecord::read(&f.json).unwrap()).collect();

    assert_eq!(now[0].results[0].effective, then[0].results[0].effective + shift);
}

#[test]
fn a1_the_committed_files_are_the_generators() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join(demo_data::DIR);
    for file in demo_data::generate(DEMO_DAY) {
        let committed = std::fs::read_to_string(dir.join(&file.name)).unwrap();
        assert_eq!(committed, file.json, "{}: run `cargo run -- demo-data`", file.name);
    }
}

#[test]
fn a1_every_patient_keeps_their_age_whatever_the_day_even_around_a_leap_day() {
    let mut day = date!(2028 - 01 - 01);
    while day.year() == 2028 {
        for file in demo_data::generate(day) {
            let birth = PatientRecord::read(&file.json).unwrap().patient.birth_date;
            assert!(day.year() - birth.year() >= 18, "{day}: {} born {birth}", file.name);
        }
        day = day.next_day().unwrap();
    }
}
