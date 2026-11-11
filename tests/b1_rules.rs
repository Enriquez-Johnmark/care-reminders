//! B1 (and B3's "already booked"): each rule type selects exactly the
//! patients its settings describe, and no others, the planted tricky cases
//! included.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

mod common;

use care_reminders::demo_data::DEMO_DAY;
use care_reminders::fhir::{Kind, PatientRecord};
use care_reminders::rules::{RuleSettings, default_rules, due};
use common::records;
use time::Duration;
use time::macros::date;

/// The patients (by FHIR id) `settings` selects among the made-up ones on
/// the demo day, once per follow-up.
fn selected(settings: &RuleSettings) -> Vec<String> {
    records()
        .iter()
        .flat_map(|record| {
            due(settings, record, DEMO_DAY)
                .into_iter()
                .map(|_| record.patient.fhir_id.clone())
        })
        .collect()
}

fn rule(kind: Kind, name: &str) -> RuleSettings {
    default_rules()
        .into_iter()
        .find(|(k, n, _)| *k == kind && n.contains(name))
        .unwrap()
        .2
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

#[test]
fn b1_after_a_result_selects_each_recent_abnormal_listed_result_and_no_one_who_booked() {
    let chosen = selected(&rule(Kind::AfterResult, "result"));

    // p02 and p14 have two abnormal listed results each; p06 (Felix) booked
    // the follow-up; p13, p24 have normal results; p16's glucose isn't listed.
    assert_eq!(
        chosen,
        ids(&[
            "p01", "p02", "p02", "p03", "p08", "p09", "p10", "p11", "p14", "p14", "p15", "p16", "p17", "p18", "p20",
            "p21", "p22", "p25"
        ])
    );
}

#[test]
fn b1_a_screening_recall_selects_the_ages_and_sex_its_settings_name_overdue_and_not_opted_out() {
    let colorectal = selected(&rule(Kind::ScreeningRecall, "Colorectal"));
    let breast = selected(&rule(Kind::ScreeningRecall, "Breast"));

    // p11 (Karen) opted out of screening reminders; p23 (Wendy) had a
    // routine visit, but no screening is recorded.
    assert_eq!(
        colorectal,
        ids(&[
            "p01", "p03", "p04", "p05", "p08", "p12", "p16", "p19", "p20", "p23", "p25"
        ])
    );
    assert_eq!(breast, ids(&["p05", "p07", "p13", "p17", "p23"]));
}

#[test]
fn b1_after_a_discharge_selects_recent_discharges_but_not_one_too_long_ago() {
    let chosen = selected(&rule(Kind::AfterDischarge, "discharge"));

    // p12 (Leo) was discharged 40 days ago.
    assert_eq!(chosen, ids(&["p04", "p05", "p07", "p15", "p19", "p22"]));
}

#[test]
fn b1_the_default_rules_select_forty_follow_ups_in_the_demo() {
    let total: usize = default_rules()
        .iter()
        .map(|(_, _, settings)| selected(settings).len())
        .sum();

    assert_eq!(total, 40);
}

#[test]
fn b1_each_follow_up_has_a_reason_and_a_due_by_date() {
    let ana = &records()[0];

    let found = due(&rule(Kind::AfterResult, "result"), ana, DEMO_DAY);

    let [one] = found.as_slice() else { panic!("{found:?}") };
    assert_eq!(one.reason, "Abnormal Hemoglobin A1c result on 2026-10-29");
    assert_eq!(one.due_by, date!(2026 - 10 - 29) + Duration::days(30));
    assert_eq!(one.source, "Observation/p01-result-1");
}

#[test]
fn b3_a_booked_or_fulfilled_follow_up_after_the_result_counts_but_a_cancelled_one_does_not() {
    let felix: PatientRecord = records()[5].clone();
    let settings = rule(Kind::AfterResult, "result");
    let mut cancelled = felix.clone();
    cancelled.appointments[0].status = care_reminders::fhir::AppointmentStatus::Cancelled;
    let mut before = felix.clone();
    before.appointments[0].start -= Duration::days(60);

    assert!(due(&settings, &felix, DEMO_DAY).is_empty(), "booked");
    assert_eq!(due(&settings, &cancelled, DEMO_DAY).len(), 1, "cancelled");
    assert_eq!(due(&settings, &before, DEMO_DAY).len(), 1, "booked before the result");
}
