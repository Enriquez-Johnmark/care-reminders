//! A2, C1 and C6: what the model is asked, how its answer is read, and how
//! the fill-ins are filled.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::checks::check;
use care_reminders::demo_data::DEMO_DAY;
use care_reminders::drafting::{FillIns, Tone, fill, parse_answer, request, template};
use care_reminders::fhir::Kind;
use care_reminders::rules::{default_rules, due};
use common::records;
use serde_json::Value;
use time::macros::date;

const KINDS: [Kind; 3] = [Kind::AfterResult, Kind::ScreeningRecall, Kind::AfterDischarge];

#[test]
fn a2_the_request_carries_only_the_kind_which_reminder_and_the_tone() {
    for kind in KINDS {
        for number in 1..=3 {
            let sent = request(kind, number, Tone::Warm);

            let user: Value = serde_json::from_str(&sent.user).unwrap();
            let mut keys: Vec<&str> = user.as_object().unwrap().keys().map(String::as_str).collect();
            keys.sort_unstable();
            assert_eq!(keys, ["kind", "of", "reminder", "tone"]);
            assert_eq!(user["kind"], kind.code());
            assert_eq!(user["reminder"], number);
        }
    }
}

#[test]
fn a2_no_name_date_reason_address_or_record_number_reaches_the_model() {
    let records = records();
    let rules = default_rules();
    let mut secrets: Vec<String> = Vec::new();
    for record in &records {
        secrets.push(record.patient.given_name.clone());
        secrets.push(record.patient.name.split(' ').nth(1).unwrap().to_owned());
        secrets.push(record.patient.fhir_id.clone());
        secrets.extend(record.patient.email.clone());
        for (_, _, settings) in &rules {
            for found in due(settings, record, DEMO_DAY) {
                secrets.push(found.reason.clone());
                secrets.push(found.due_by.to_string());
            }
        }
    }
    for kind in KINDS {
        for number in 1..=3 {
            let sent = request(kind, number, Tone::Plain);
            let everything = format!("{} {}", sent.system, sent.user);
            for secret in &secrets {
                assert!(!everything.contains(secret.as_str()), "{secret:?} reached the model");
            }
            assert!(!everything.contains("2026"), "a date reached the model");
        }
    }
}

#[test]
fn c1_the_answer_is_parsed_into_strict_types_and_anything_else_is_refused() {
    assert_eq!(
        parse_answer(r#"{"text": "Hello {first_name}."}"#).unwrap(),
        "Hello {first_name}."
    );
    for broken in [
        "Hello there",
        r#"{"text": ""}"#,
        r#"{"text": "Hi", "extra": "field"}"#,
        r#"{"message": "Hi"}"#,
        r#"{"text": 42}"#,
        r#"[{"text": "Hi"}]"#,
    ] {
        assert!(parse_answer(broken).is_err(), "{broken} was accepted");
    }
    assert!(
        parse_answer(&format!(r#"{{"text": "{}"}}"#, "word ".repeat(400))).is_err(),
        "too long"
    );
}

#[test]
fn c6_fill_ins_come_only_from_the_clinics_data_exactly_as_stored() {
    let values = FillIns {
        first_name: "Ana".into(),
        clinic_name: "Example Family Practice".into(),
        clinic_phone: "(555) 010-0100".into(),
        due_by: date!(2026 - 12 - 09),
        opt_out_link: "https://reminders.example.test/o/abc".into(),
    };

    let filled = fill(&template(1), &values).unwrap();

    assert!(filled.contains("Hello Ana,"), "{filled}");
    assert!(filled.contains("Example Family Practice"), "{filled}");
    assert!(filled.contains("(555) 010-0100"), "{filled}");
    assert!(filled.contains("December 9, 2026"), "{filled}");
    assert!(filled.contains("https://reminders.example.test/o/abc"), "{filled}");
    assert!(!filled.contains('{') && !filled.contains('}'), "{filled}");
}

#[test]
fn c6_a_draft_with_an_unknown_fill_in_is_never_filled() {
    let values = FillIns {
        first_name: "Ana".into(),
        clinic_name: "Example Family Practice".into(),
        clinic_phone: "(555) 010-0100".into(),
        due_by: date!(2026 - 12 - 09),
        opt_out_link: "https://reminders.example.test/o/abc".into(),
    };

    assert!(fill("Hello {full_name}, call {clinic_phone}. {opt_out_link}", &values).is_err());
}

#[test]
fn c5_the_clinics_templates_pass_every_check() {
    for number in 1..=3 {
        let checked = check(&template(number));
        assert!(
            checked.problems.is_empty() && checked.warnings.is_empty(),
            "{number}: {checked:?}"
        );
    }
}

fn values() -> FillIns {
    FillIns {
        first_name: "Ana".into(),
        clinic_name: "Example Family Practice".into(),
        clinic_phone: "(555) 010-0100".into(),
        due_by: date!(2026 - 12 - 09),
        opt_out_link: "https://reminders.example.test/o/abc".into(),
    }
}

#[test]
fn c6_a_value_that_looks_like_a_fill_in_is_kept_exactly_as_stored() {
    let odd = FillIns {
        first_name: "{clinic_phone}".into(),
        ..values()
    };

    let filled = fill(&template(1), &odd).unwrap();

    assert!(filled.starts_with("Hello {clinic_phone}, this is"), "{filled}");
}

#[test]
fn c6_an_empty_value_is_never_filled_in() {
    let no_phone = FillIns {
        clinic_phone: String::new(),
        ..values()
    };

    assert!(fill(&template(1), &no_phone).is_err());
}
