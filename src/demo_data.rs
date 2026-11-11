//! The made-up data (A1): 25 patients of Example Family Practice, a made-up
//! US primary-care clinic, as FHIR R4 bundles, one per patient. Every bundle
//! and resource carries the HL7 security label `HTEST` ("test health data"),
//! so no tool can take it for a real record, and every name says "(made up)".
//!
//! Dates count back from the day the data is made for, so the demo is
//! current whenever it is loaded; the committed files are made for
//! [`DEMO_DAY`]. The same day always gives the same files.
//!
//! Five tricky cases are planted on purpose: a patient who already booked,
//! one who opted out of screening reminders, one who asked not to be emailed,
//! one whose result is sensitive, and one discharged too long ago to be due.

use serde_json::{Value, json};
use time::{Date, Duration, macros::date};

/// Where the generated files are committed.
pub const DIR: &str = "demo/patients";

/// The day the committed files are made for.
pub const DEMO_DAY: Date = date!(2026 - 11 - 10);

/// The test whose abnormal result is sensitive: a reminder must never hint
/// at it (E7).
pub const SENSITIVE_TEST: &str = "43304-5";

const TEST_DATA: &str = "http://terminology.hl7.org/CodeSystem/v3-ActReason";
const LOINC: &str = "http://loinc.org";
const SNOMED: &str = "http://snomed.info/sct";

/// One generated file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DemoFile {
    /// Its file name, e.g. `patient-01.json`.
    pub name: String,
    /// The bundle, as pretty-printed JSON with a final newline.
    pub json: String,
}

/// A lab test: its LOINC code and name.
type Test = (&'static str, &'static str);
const HBA1C: Test = ("4548-4", "Hemoglobin A1c");
const LDL: Test = ("13457-7", "LDL cholesterol (calculated)");
const TSH: Test = ("3016-3", "Thyrotropin (TSH)");
const POTASSIUM: Test = ("2823-3", "Potassium");
const GLUCOSE: Test = ("2345-7", "Glucose");
const CHLAMYDIA: Test = (SENSITIVE_TEST, "Chlamydia trachomatis rRNA");

/// A screening: its SNOMED CT code and name.
type Screen = (&'static str, &'static str);
const COLONOSCOPY: Screen = ("73761001", "Colonoscopy");
const MAMMOGRAM: Screen = ("71651007", "Mammography");

/// One made-up patient and their care, in days before (or, for
/// appointments, after) the day the data is made for.
struct Person {
    given: &'static str,
    family: &'static str,
    female: bool,
    age: i32,
    /// Test, abnormal, days ago.
    results: &'static [(Test, bool, i64)],
    /// Days ago admitted, days ago discharged.
    stays: &'static [(i64, i64)],
    /// Screening, days ago.
    screenings: &'static [(Screen, i64)],
    /// Type, status, days from now.
    appointments: &'static [(&'static str, &'static str, i64)],
    no_email: bool,
    opted_out: Option<&'static str>,
}

const fn person(given: &'static str, family: &'static str, female: bool, age: i32) -> Person {
    Person {
        given,
        family,
        female,
        age,
        results: &[],
        stays: &[],
        screenings: &[],
        appointments: &[],
        no_email: false,
        opted_out: None,
    }
}

const PEOPLE: &[Person] = &[
    Person {
        results: &[(HBA1C, true, 12)],
        screenings: &[(MAMMOGRAM, 400)],
        ..person("Ana", "Reyes", true, 52)
    },
    Person {
        results: &[(LDL, true, 20), (TSH, true, 33)],
        screenings: &[(COLONOSCOPY, 3000)],
        ..person("Ben", "Cruz", false, 61)
    },
    Person {
        results: &[(TSH, true, 5)],
        ..person("Carla", "Santos", true, 47)
    },
    Person {
        stays: &[(7, 3)],
        screenings: &[(COLONOSCOPY, 4000)],
        ..person("Dan", "Mercado", false, 68)
    },
    Person {
        stays: &[(9, 4)],
        ..person("Elena", "Villanueva", true, 58)
    },
    // Planted: already booked the follow-up for an abnormal result.
    Person {
        results: &[(HBA1C, true, 30)],
        appointments: &[("FOLLOWUP", "booked", 6)],
        ..person("Felix", "Garcia", false, 39)
    },
    Person {
        stays: &[(11, 5)],
        screenings: &[(MAMMOGRAM, 900), (COLONOSCOPY, 1000)],
        ..person("Gina", "Bautista", true, 66)
    },
    Person {
        results: &[(POTASSIUM, true, 8)],
        ..person("Hector", "Ramos", false, 73)
    },
    // Planted: a sensitive result; no reminder may hint at it.
    Person {
        results: &[(CHLAMYDIA, true, 10)],
        ..person("Iris", "Navarro", true, 29)
    },
    // Planted: asked not to be emailed.
    Person {
        results: &[(LDL, true, 15)],
        screenings: &[(COLONOSCOPY, 2000)],
        no_email: true,
        ..person("Jun", "Torres", false, 55)
    },
    // Planted: opted out of screening reminders.
    Person {
        results: &[(HBA1C, true, 25)],
        opted_out: Some("screening-recall"),
        ..person("Karen", "Flores", true, 63)
    },
    // Planted: discharged too long ago to be due.
    Person {
        stays: &[(45, 40)],
        ..person("Leo", "Aquino", false, 48)
    },
    Person {
        results: &[(TSH, false, 14)],
        screenings: &[(MAMMOGRAM, 800), (COLONOSCOPY, 500)],
        appointments: &[("ROUTINE", "cancelled", -20)],
        ..person("Mia", "Domingo", true, 51)
    },
    Person {
        results: &[(HBA1C, true, 40), (LDL, true, 40)],
        screenings: &[(COLONOSCOPY, 1200)],
        ..person("Noel", "Castillo", false, 70)
    },
    Person {
        results: &[(TSH, true, 11)],
        stays: &[(5, 2)],
        ..person("Olivia", "Mendoza", true, 44)
    },
    Person {
        results: &[(GLUCOSE, false, 30), (HBA1C, true, 2)],
        ..person("Paolo", "Rivera", false, 57)
    },
    Person {
        results: &[(HBA1C, true, 18)],
        screenings: &[(MAMMOGRAM, 1000), (COLONOSCOPY, 1500)],
        ..person("Queenie", "Lim", true, 69)
    },
    Person {
        results: &[(TSH, true, 22)],
        ..person("Rafael", "Ocampo", false, 34)
    },
    Person {
        stays: &[(10, 6)],
        screenings: &[(MAMMOGRAM, 100)],
        ..person("Sofia", "Herrera", true, 72)
    },
    Person {
        results: &[(POTASSIUM, false, 21), (LDL, true, 14)],
        ..person("Tomas", "Pascual", false, 46)
    },
    Person {
        results: &[(LDL, true, 9)],
        screenings: &[(COLONOSCOPY, 3500), (MAMMOGRAM, 300)],
        ..person("Uma", "Dela Cruz", true, 55)
    },
    Person {
        results: &[(LDL, true, 60)],
        stays: &[(4, 1)],
        screenings: &[(COLONOSCOPY, 3000)],
        ..person("Victor", "Lopez", false, 64)
    },
    Person {
        appointments: &[("ROUTINE", "fulfilled", -200)],
        ..person("Wendy", "Tan", true, 50)
    },
    Person {
        results: &[(HBA1C, false, 16)],
        ..person("Xavier", "Morales", false, 41)
    },
    Person {
        results: &[(TSH, true, 3)],
        screenings: &[(MAMMOGRAM, 600), (COLONOSCOPY, 4500)],
        ..person("Yolanda", "Ferrer", true, 59)
    },
];

/// The made-up patients' bundles, made for `day`.
pub fn generate(day: Date) -> Vec<DemoFile> {
    PEOPLE
        .iter()
        .enumerate()
        .map(|(index, person)| {
            let bundle = bundle(index + 1, person, day);
            let json = serde_json::to_string_pretty(&bundle).unwrap_or_default() + "\n";
            DemoFile {
                name: format!("patient-{:02}.json", index + 1),
                json,
            }
        })
        .collect()
}

fn meta() -> Value {
    json!({ "security": [{ "system": TEST_DATA, "code": "HTEST", "display": "test health data" }] })
}

fn bundle(number: usize, person: &Person, day: Date) -> Value {
    let id = format!("p{number:02}");
    let subject = json!({ "reference": format!("Patient/{id}") });
    let ago = |days: i64| (day - Duration::days(days)).to_string();
    let mut resources = vec![patient(&id, number, person, day)];
    for (n, ((code, name), abnormal, days)) in person.results.iter().enumerate() {
        resources.push(json!({
            "resourceType": "Observation", "id": format!("{id}-result-{}", n + 1), "meta": meta(),
            "status": "final",
            "category": [{ "coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/observation-category", "code": "laboratory"
            }] }],
            "code": { "coding": [{ "system": LOINC, "code": code, "display": name }], "text": name },
            "subject": subject, "effectiveDateTime": ago(*days),
            "interpretation": [{ "coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/v3-ObservationInterpretation",
                "code": if *abnormal { "A" } else { "N" },
            }] }]
        }));
    }
    for (n, (admitted, discharged)) in person.stays.iter().enumerate() {
        resources.push(json!({
            "resourceType": "Encounter", "id": format!("{id}-stay-{}", n + 1), "meta": meta(),
            "status": "finished",
            "class": { "system": "http://terminology.hl7.org/CodeSystem/v3-ActCode", "code": "IMP" },
            "subject": subject, "period": { "start": ago(*admitted), "end": ago(*discharged) }
        }));
    }
    for (n, ((code, name), days)) in person.screenings.iter().enumerate() {
        resources.push(json!({
            "resourceType": "Procedure", "id": format!("{id}-screening-{}", n + 1), "meta": meta(),
            "status": "completed",
            "code": { "coding": [{ "system": SNOMED, "code": code, "display": name }], "text": name },
            "subject": subject, "performedDateTime": ago(*days)
        }));
    }
    for (n, (kind, status, days)) in person.appointments.iter().enumerate() {
        let start = day + Duration::days(*days);
        resources.push(json!({
            "resourceType": "Appointment", "id": format!("{id}-appointment-{}", n + 1), "meta": meta(),
            "status": status,
            "appointmentType": { "coding": [{
                "system": "http://terminology.hl7.org/CodeSystem/v2-0276", "code": kind
            }] },
            "start": format!("{start}T09:30:00Z"), "end": format!("{start}T10:00:00Z"),
            "participant": [{ "actor": { "reference": format!("Patient/{id}") }, "status": "accepted" }]
        }));
    }
    let entry: Vec<Value> = resources
        .into_iter()
        .map(|resource| {
            let url = format!("urn:example:{}", resource["id"].as_str().unwrap_or_default());
            json!({ "fullUrl": url, "resource": resource })
        })
        .collect();
    json!({ "resourceType": "Bundle", "id": format!("record-{id}"), "type": "collection", "meta": meta(), "entry": entry })
}

fn patient(id: &str, number: usize, person: &Person, day: Date) -> Value {
    let family = format!("{} (made up)", person.family);
    // Birthdays spread over the year, so ages are exact on `day`.
    let birthday = day - Duration::days((number as i64 * 37) % 300 + 1);
    // 29 February moves to the 28th in a birth year that has no leap day.
    let year = birthday.year() - person.age;
    let birth = birthday
        .replace_year(year)
        .or_else(|_| (birthday - Duration::days(1)).replace_year(year))
        .unwrap_or(birthday);
    let email = format!(
        "{}.{}@example.test",
        person.given.to_lowercase(),
        person.family.to_lowercase().replace(' ', "")
    );
    let mut extension = Vec::new();
    if person.no_email {
        extension.push(json!({ "url": crate::fhir::NO_EMAIL, "valueBoolean": true }));
    }
    if let Some(kind) = person.opted_out {
        extension.push(json!({ "url": crate::fhir::OPT_OUT, "valueCode": kind }));
    }
    let mut patient = json!({
        "resourceType": "Patient", "id": id, "meta": meta(),
        "name": [{ "given": [person.given], "family": family }],
        "gender": if person.female { "female" } else { "male" },
        "birthDate": birth.to_string(),
        "telecom": [{ "system": "email", "value": email }]
    });
    if !extension.is_empty() {
        patient["extension"] = Value::Array(extension);
    }
    patient
}
