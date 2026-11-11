//! H2 (the reading part): a patient's record is read from a FHIR R4 bundle
//! into strict types, and anything broken is refused with the reason.

#![allow(clippy::unwrap_used, reason = "test code: a failed unwrap is a failed test")]

use care_reminders::fhir::{AppointmentStatus, FhirError, Kind, PatientRecord, Sex};
use serde_json::{Value, json};
use time::macros::{date, datetime};

const TEST_DATA: &str = "http://terminology.hl7.org/CodeSystem/v3-ActReason";

fn meta() -> Value {
    json!({ "security": [{ "system": TEST_DATA, "code": "HTEST" }] })
}

fn patient() -> Value {
    json!({
        "resourceType": "Patient", "id": "p1", "meta": meta(),
        "name": [{ "given": ["Ana"], "family": "Reyes (made up)" }],
        "gender": "female", "birthDate": "1970-03-02",
        "telecom": [{ "system": "email", "value": "ana.reyes@example.test" }]
    })
}

fn result(id: &str, interpretation: &str) -> Value {
    json!({
        "resourceType": "Observation", "id": id, "meta": meta(), "status": "final",
        "category": [{ "coding": [{ "code": "laboratory" }] }],
        "code": { "coding": [{ "system": "http://loinc.org", "code": "4548-4", "display": "Hemoglobin A1c" }] },
        "subject": { "reference": "Patient/p1" },
        "effectiveDateTime": "2026-10-20",
        "interpretation": [{ "coding": [{ "code": interpretation }] }]
    })
}

fn stay() -> Value {
    json!({
        "resourceType": "Encounter", "id": "e1", "meta": meta(), "status": "finished",
        "class": { "code": "IMP" }, "subject": { "reference": "Patient/p1" },
        "period": { "start": "2026-11-01", "end": "2026-11-05" }
    })
}

fn screening() -> Value {
    json!({
        "resourceType": "Procedure", "id": "s1", "meta": meta(), "status": "completed",
        "code": { "coding": [{ "system": "http://snomed.info/sct", "code": "73761001", "display": "Colonoscopy" }] },
        "subject": { "reference": "Patient/p1" }, "performedDateTime": "2016-05-10"
    })
}

fn appointment() -> Value {
    json!({
        "resourceType": "Appointment", "id": "a1", "meta": meta(), "status": "booked",
        "appointmentType": { "coding": [{ "code": "FOLLOWUP" }] },
        "start": "2026-11-20T09:30:00Z",
        "participant": [{ "actor": { "reference": "Patient/p1" }, "status": "accepted" }]
    })
}

fn bundle(resources: Vec<Value>) -> String {
    let entry: Vec<Value> = resources.into_iter().map(|r| json!({ "resource": r })).collect();
    json!({ "resourceType": "Bundle", "type": "collection", "meta": meta(), "entry": entry }).to_string()
}

fn whole() -> String {
    bundle(vec![patient(), result("o1", "H"), stay(), screening(), appointment()])
}

#[test]
fn h2_a_patients_record_is_read_into_strict_types() {
    let record = PatientRecord::read(&whole()).unwrap();

    assert_eq!(record.patient.fhir_id, "p1");
    assert_eq!(record.patient.given_name, "Ana");
    assert_eq!(record.patient.name, "Ana Reyes (made up)");
    assert_eq!(record.patient.sex, Sex::Female);
    assert_eq!(record.patient.birth_date, date!(1970 - 03 - 02));
    assert_eq!(record.patient.email.as_deref(), Some("ana.reyes@example.test"));
    assert!(!record.patient.no_email);
    assert!(record.patient.opted_out.is_empty());
    let [result] = record.results.as_slice() else {
        panic!("{:?}", record.results)
    };
    assert_eq!((result.code.as_str(), result.abnormal), ("4548-4", true));
    assert_eq!(result.effective, date!(2026 - 10 - 20));
    assert_eq!(record.stays[0].discharged, Some(date!(2026 - 11 - 05)));
    assert_eq!(record.screenings[0].code, "73761001");
    assert_eq!(record.screenings[0].performed, date!(2016 - 05 - 10));
    assert_eq!(record.appointments[0].status, AppointmentStatus::Booked);
    assert_eq!(record.appointments[0].kind, "FOLLOWUP");
    assert_eq!(record.appointments[0].start, datetime!(2026-11-20 09:30 UTC));
}

#[test]
fn h2_a_normal_result_is_not_abnormal_and_one_without_an_interpretation_is_refused() {
    let normal = PatientRecord::read(&bundle(vec![patient(), result("o1", "N")])).unwrap();
    assert!(!normal.results[0].abnormal);

    let mut unread = result("o2", "H");
    unread.as_object_mut().unwrap().remove("interpretation");
    assert_eq!(
        PatientRecord::read(&bundle(vec![patient(), unread])),
        Err(FhirError::Resource(
            "Observation/o2".into(),
            "no interpretation (normal or abnormal)".into()
        ))
    );
}

#[test]
fn h2_the_patients_wishes_are_read_from_the_record() {
    let mut wishes = patient();
    wishes["extension"] = json!([
        { "url": "http://example.test/fhir/StructureDefinition/no-email", "valueBoolean": true },
        { "url": "http://example.test/fhir/StructureDefinition/reminder-opt-out", "valueCode": "screening-recall" }
    ]);

    let record = PatientRecord::read(&bundle(vec![wishes])).unwrap();

    assert!(record.patient.no_email);
    assert_eq!(record.patient.opted_out, [Kind::ScreeningRecall]);
}

#[test]
fn h2_something_that_isnt_a_bundle_is_refused() {
    assert!(matches!(PatientRecord::read("not json"), Err(FhirError::NotJson(_))));
    assert_eq!(PatientRecord::read(&patient().to_string()), Err(FhirError::NotABundle));
}

#[test]
fn h2_a_bundle_needs_exactly_one_patient() {
    assert_eq!(
        PatientRecord::read(&bundle(vec![result("o1", "H")])),
        Err(FhirError::PatientCount("no patient"))
    );
    let mut second = patient();
    second["id"] = json!("p2");
    assert_eq!(
        PatientRecord::read(&bundle(vec![patient(), second])),
        Err(FhirError::PatientCount("more than one patient"))
    );
}

#[test]
fn h2_a_resource_about_another_patient_is_refused() {
    let mut theirs = appointment();
    theirs["participant"][0]["actor"]["reference"] = json!("Patient/p9");

    assert_eq!(
        PatientRecord::read(&bundle(vec![patient(), theirs])),
        Err(FhirError::Resource(
            "Appointment/a1".into(),
            "is about another patient".into()
        ))
    );
}

#[test]
fn h2_a_resource_with_a_broken_date_is_refused() {
    let mut broken = screening();
    broken["performedDateTime"] = json!("last spring");

    assert_eq!(
        PatientRecord::read(&bundle(vec![patient(), broken])),
        Err(FhirError::Resource("Procedure/s1".into(), "no valid date".into()))
    );
}

#[test]
fn h2_an_unfinished_screening_and_a_visit_that_isnt_a_hospital_stay_are_left_out() {
    let mut planned = screening();
    planned["status"] = json!("preparation");
    let mut visit = stay();
    visit["class"] = json!({ "code": "AMB" });

    let record = PatientRecord::read(&bundle(vec![patient(), planned, visit])).unwrap();

    assert!(record.screenings.is_empty() && record.stays.is_empty());
}
