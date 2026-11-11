//! Reading one patient's record from a FHIR R4 bundle into strict types (H2).
//!
//! What a FHIR server sends is untrusted. Only the fields the rules use are
//! read, and each is checked: a bundle holds exactly one patient, and every
//! other resource in it is about that patient. Resources of other types are
//! left out; a resource of a type the rules use that can't be read is
//! refused with the reason, never guessed at.

use serde_json::Value;
use time::{Date, OffsetDateTime, format_description::well_known::Rfc3339, macros::format_description};

/// The made-up extension saying a patient asked not to be emailed.
pub const NO_EMAIL: &str = "http://example.test/fhir/StructureDefinition/no-email";
/// The made-up extension saying a patient opted out of one kind of reminder.
pub const OPT_OUT: &str = "http://example.test/fhir/StructureDefinition/reminder-opt-out";

/// A patient's record: who they are and the care the rules look at.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PatientRecord {
    /// The patient.
    pub patient: Patient,
    /// Their lab results.
    pub results: Vec<LabResult>,
    /// Their hospital stays.
    pub stays: Vec<HospitalStay>,
    /// The screenings they had done.
    pub screenings: Vec<Screening>,
    /// Their appointments, in any status.
    pub appointments: Vec<Appointment>,
}

/// A patient.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Patient {
    /// Their FHIR id.
    pub fhir_id: String,
    /// The name a reminder greets them by.
    pub given_name: String,
    /// Their whole name, as staff see it.
    pub name: String,
    /// Their sex, as recorded.
    pub sex: Sex,
    /// Their date of birth.
    pub birth_date: Date,
    /// Their email address, if the record has one.
    pub email: Option<String>,
    /// They asked not to be emailed (E5).
    pub no_email: bool,
    /// The kinds of reminder they opted out of (F3).
    pub opted_out: Vec<Kind>,
}

/// A patient's recorded sex, which screening rules may use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Sex {
    /// Female.
    Female,
    /// Male.
    Male,
    /// Other or unknown.
    Other,
}

/// The three kinds of follow-up, and so of reminder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// A follow-up visit after an abnormal result.
    AfterResult,
    /// A screening recall.
    ScreeningRecall,
    /// A visit after a hospital discharge.
    AfterDischarge,
}

impl Kind {
    /// Its code, as in the opt-out extension and the database.
    pub fn code(self) -> &'static str {
        match self {
            Self::AfterResult => "after-result",
            Self::ScreeningRecall => "screening-recall",
            Self::AfterDischarge => "after-discharge",
        }
    }

    /// The kind with this code.
    pub fn from_code(code: &str) -> Option<Self> {
        [Self::AfterResult, Self::ScreeningRecall, Self::AfterDischarge]
            .into_iter()
            .find(|kind| kind.code() == code)
    }
}

/// One lab result.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LabResult {
    /// The Observation's FHIR id.
    pub fhir_id: String,
    /// The test's code (LOINC).
    pub code: String,
    /// The test's name.
    pub name: String,
    /// Whether the lab flagged it as abnormal.
    pub abnormal: bool,
    /// When the sample was taken.
    pub effective: Date,
}

/// One hospital stay.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct HospitalStay {
    /// The Encounter's FHIR id.
    pub fhir_id: String,
    /// When they left hospital, once they have.
    pub discharged: Option<Date>,
}

/// One screening done.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Screening {
    /// The Procedure's FHIR id.
    pub fhir_id: String,
    /// Its code (SNOMED CT).
    pub code: String,
    /// When it was done.
    pub performed: Date,
}

/// One appointment.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Appointment {
    /// Its FHIR id.
    pub fhir_id: String,
    /// Where it stands.
    pub status: AppointmentStatus,
    /// Its type's code, such as `FOLLOWUP` or `ROUTINE`.
    pub kind: String,
    /// When it starts.
    pub start: OffsetDateTime,
}

/// Where an appointment stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum AppointmentStatus {
    /// Booked, or about to happen.
    Booked,
    /// It happened.
    Fulfilled,
    /// Cancelled, or the patient didn't come.
    Cancelled,
}

/// Why a bundle was refused.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FhirError {
    /// It isn't JSON.
    #[error("not JSON: {0}")]
    NotJson(String),
    /// It isn't a bundle.
    #[error("not a FHIR Bundle")]
    NotABundle,
    /// It has no patient, or more than one.
    #[error("the bundle holds {0}")]
    PatientCount(&'static str),
    /// Something about the patient.
    #[error("Patient: {0}")]
    Patient(String),
    /// Something about one resource: its type and id, and the problem.
    #[error("{0}: {1}")]
    Resource(String, String),
}

impl PatientRecord {
    /// The record in `json`, a FHIR R4 bundle.
    pub fn read(json: &str) -> Result<Self, FhirError> {
        let bundle: Value = serde_json::from_str(json).map_err(|err| FhirError::NotJson(err.to_string()))?;
        if bundle["resourceType"] != "Bundle" {
            return Err(FhirError::NotABundle);
        }
        let resources: Vec<&Value> = bundle["entry"]
            .as_array()
            .map(|entries| entries.iter().map(|entry| &entry["resource"]).collect())
            .unwrap_or_default();
        Self::from_resources(&resources)
    }

    /// The record made of `resources`: one patient and their care.
    pub fn from_resources(resources: &[&Value]) -> Result<Self, FhirError> {
        let patients: Vec<&&Value> = resources.iter().filter(|r| r["resourceType"] == "Patient").collect();
        let patient = match patients.as_slice() {
            [] => return Err(FhirError::PatientCount("no patient")),
            [one] => read_patient(one)?,
            _ => return Err(FhirError::PatientCount("more than one patient")),
        };
        let own = format!("Patient/{}", patient.fhir_id);
        let mut record = Self {
            patient,
            results: Vec::new(),
            stays: Vec::new(),
            screenings: Vec::new(),
            appointments: Vec::new(),
        };
        for resource in resources {
            let kind = resource["resourceType"].as_str().unwrap_or_default();
            let label = format!("{kind}/{}", resource["id"].as_str().unwrap_or("?"));
            let refuse = |problem: &str| FhirError::Resource(label.clone(), problem.to_owned());
            let about = |reference: &Value| reference.as_str() == Some(own.as_str());
            match kind {
                "Observation" => {
                    if !about(&resource["subject"]["reference"]) {
                        return Err(refuse("is about another patient"));
                    }
                    // Vital signs, surveys and results the lab hasn't
                    // finalised, or withdrew, aren't results the rules use.
                    if is_lab(resource)
                        && matches!(resource["status"].as_str(), Some("final" | "amended" | "corrected"))
                    {
                        record.results.push(read_result(resource).map_err(refuse)?);
                    }
                }
                "Encounter" => {
                    if !about(&resource["subject"]["reference"]) {
                        return Err(refuse("is about another patient"));
                    }
                    if resource["class"]["code"] == "IMP" {
                        let discharged = match &resource["period"]["end"] {
                            Value::Null => None,
                            end => Some(date(end).ok_or_else(|| refuse("no valid date"))?),
                        };
                        record.stays.push(HospitalStay {
                            fhir_id: id(resource).map_err(refuse)?,
                            discharged,
                        });
                    }
                }
                "Procedure" => {
                    if !about(&resource["subject"]["reference"]) {
                        return Err(refuse("is about another patient"));
                    }
                    if resource["status"] == "completed" {
                        record.screenings.push(Screening {
                            fhir_id: id(resource).map_err(refuse)?,
                            code: first_code(&resource["code"]).ok_or_else(|| refuse("no code"))?,
                            performed: date(&resource["performedDateTime"]).ok_or_else(|| refuse("no valid date"))?,
                        });
                    }
                }
                "Appointment" => {
                    let participants = resource["participant"]
                        .as_array()
                        .map(Vec::as_slice)
                        .unwrap_or_default();
                    if !participants.iter().any(|p| about(&p["actor"]["reference"])) {
                        return Err(refuse("is about another patient"));
                    }
                    record.appointments.push(read_appointment(resource).map_err(refuse)?);
                }
                _ => {}
            }
        }
        Ok(record)
    }
}

fn read_patient(resource: &Value) -> Result<Patient, FhirError> {
    let refuse = |problem: &str| FhirError::Patient(problem.to_owned());
    let fhir_id = id(resource).map_err(refuse)?;
    let name = &resource["name"][0];
    let given_name = name["given"][0]
        .as_str()
        .filter(|given| !given.trim().is_empty())
        .ok_or_else(|| refuse("no given name"))?
        .to_owned();
    let family = name["family"].as_str().unwrap_or_default();
    let sex = match resource["gender"].as_str() {
        Some("female") => Sex::Female,
        Some("male") => Sex::Male,
        _ => Sex::Other,
    };
    let birth_date = date(&resource["birthDate"]).ok_or_else(|| refuse("no valid birth date"))?;
    let email = resource["telecom"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|telecom| telecom["system"] == "email")
        .and_then(|telecom| telecom["value"].as_str())
        .map(str::to_owned);
    let extensions = resource["extension"].as_array().map(Vec::as_slice).unwrap_or_default();
    let no_email = extensions
        .iter()
        .any(|e| e["url"] == NO_EMAIL && e["valueBoolean"] == true);
    let mut opted_out = Vec::new();
    for extension in extensions.iter().filter(|e| e["url"] == OPT_OUT) {
        let code = extension["valueCode"].as_str().unwrap_or_default();
        opted_out.push(Kind::from_code(code).ok_or_else(|| refuse("an opt-out of an unknown kind of reminder"))?);
    }
    Ok(Patient {
        fhir_id,
        name: format!("{given_name} {family}").trim().to_owned(),
        given_name,
        sex,
        birth_date,
        email,
        no_email,
        opted_out,
    })
}

fn read_result(resource: &Value) -> Result<LabResult, &'static str> {
    let code = first_code(&resource["code"]).ok_or("no code")?;
    let name = resource["code"]["coding"][0]["display"]
        .as_str()
        .or_else(|| resource["code"]["text"].as_str())
        .unwrap_or(&code)
        .to_owned();
    let interpretation = resource["interpretation"][0]["coding"][0]["code"]
        .as_str()
        .ok_or("no interpretation (normal or abnormal)")?;
    let abnormal = match interpretation {
        "N" | "NEG" => false,
        "H" | "HH" | "L" | "LL" | "A" | "AA" | "POS" => true,
        _ => return Err("an interpretation that isn't normal or abnormal"),
    };
    Ok(LabResult {
        fhir_id: id(resource)?,
        code,
        name,
        abnormal,
        effective: date(&resource["effectiveDateTime"])
            .or_else(|| date(&resource["effectivePeriod"]["start"]))
            .or_else(|| date(&resource["issued"]))
            .ok_or("no valid date")?,
    })
}

fn read_appointment(resource: &Value) -> Result<Appointment, &'static str> {
    let status = match resource["status"].as_str() {
        Some("proposed" | "pending" | "booked" | "arrived" | "checked-in" | "waitlist") => AppointmentStatus::Booked,
        Some("fulfilled") => AppointmentStatus::Fulfilled,
        Some("cancelled" | "noshow" | "entered-in-error") => AppointmentStatus::Cancelled,
        _ => return Err("no valid status"),
    };
    let start = resource["start"]
        .as_str()
        .and_then(|start| OffsetDateTime::parse(start, &Rfc3339).ok())
        .ok_or("no valid start")?;
    Ok(Appointment {
        fhir_id: id(resource)?,
        status,
        kind: first_code(&resource["appointmentType"]).ok_or("no appointment type")?,
        start,
    })
}

/// Whether an Observation is a laboratory result.
fn is_lab(resource: &Value) -> bool {
    resource["category"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|category| category["coding"].as_array().into_iter().flatten())
        .any(|coding| coding["code"] == "laboratory")
}

fn id(resource: &Value) -> Result<String, &'static str> {
    resource["id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
        .ok_or("no id")
}

fn first_code(concept: &Value) -> Option<String> {
    concept["coding"][0]["code"].as_str().map(str::to_owned)
}

/// The date at the start of a FHIR `date` or `dateTime`.
fn date(value: &Value) -> Option<Date> {
    let text = value.as_str()?;
    Date::parse(text.get(..10)?, format_description!("[year]-[month]-[day]")).ok()
}
