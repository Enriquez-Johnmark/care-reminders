//! The clinic's follow-up rules (B1, B3, B5): who is due, by when and why.
//!
//! Each rule type is a plain function from a patient's record, the rule's
//! settings and the day to the follow-ups it finds. It makes no model call
//! and no database call: the model never decides who is contacted.

use serde::{Deserialize, Serialize};
use time::{Date, Duration};

use crate::fhir::{AppointmentStatus, Kind, PatientRecord, Sex};

/// A rule's settings: one of the fixed rule types.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RuleSettings {
    /// An abnormal result of a listed test → a follow-up visit.
    AfterResult {
        /// The tests' LOINC codes.
        tests: Vec<String>,
        /// Days after the result the visit is due by.
        within_days: i64,
        /// How far back a result still counts.
        look_back_days: i64,
    },
    /// A screening not done for a while, at certain ages.
    ScreeningRecall {
        /// The screening's SNOMED CT code.
        screening: String,
        /// What staff call it, in the reason.
        screening_name: String,
        /// How often it is due.
        every_months: i32,
        /// The youngest age it applies to.
        min_age: i32,
        /// The oldest age it applies to.
        max_age: i32,
        /// Only for this sex, if set.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sex: Option<Sex>,
        /// Days from today it is due by.
        within_days: i64,
    },
    /// A hospital discharge → a visit soon after.
    AfterDischarge {
        /// Days after the discharge the visit is due by.
        within_days: i64,
        /// How far back a discharge still counts.
        look_back_days: i64,
    },
}

impl RuleSettings {
    /// The kind of follow-up this rule finds.
    pub fn kind(&self) -> Kind {
        match self {
            Self::AfterResult { .. } => Kind::AfterResult,
            Self::ScreeningRecall { .. } => Kind::ScreeningRecall,
            Self::AfterDischarge { .. } => Kind::AfterDischarge,
        }
    }
}

/// One follow-up a rule found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Due {
    /// What in the record it comes from, so the same thing never makes two
    /// follow-ups: e.g. `Observation/p01-result-1` or `screening/73761001`.
    pub source: String,
    /// Why, for staff (never for the patient).
    pub reason: String,
    /// When the care is due by.
    pub due_by: Date,
}

/// A new clinic's rules: their kind, name and settings.
pub fn default_rules() -> Vec<(Kind, &'static str, RuleSettings)> {
    let screening = |code: &str, name: &str, every_months, (min_age, max_age), sex| RuleSettings::ScreeningRecall {
        screening: code.to_owned(),
        screening_name: name.to_owned(),
        every_months,
        min_age,
        max_age,
        sex,
        within_days: 30,
    };
    vec![
        (
            Kind::AfterResult,
            "Follow-up after an abnormal result",
            RuleSettings::AfterResult {
                // HbA1c, LDL cholesterol, TSH, potassium, chlamydia.
                tests: ["4548-4", "13457-7", "3016-3", "2823-3", "43304-5"]
                    .map(str::to_owned)
                    .to_vec(),
                within_days: 30,
                look_back_days: 90,
            },
        ),
        (
            Kind::ScreeningRecall,
            "Colorectal screening recall",
            screening("73761001", "Colonoscopy", 120, (45, 75), None),
        ),
        (
            Kind::ScreeningRecall,
            "Breast screening recall",
            screening("71651007", "Mammography", 24, (50, 74), Some(Sex::Female)),
        ),
        (
            Kind::AfterDischarge,
            "Visit after a hospital discharge",
            RuleSettings::AfterDischarge {
                within_days: 7,
                look_back_days: 14,
            },
        ),
    ]
}

/// The follow-ups `settings` finds in `record` on `today`. A patient who
/// opted out of the kind, or already booked the visit, has none.
pub fn due(settings: &RuleSettings, record: &PatientRecord, today: Date) -> Vec<Due> {
    if record.patient.opted_out.contains(&settings.kind()) {
        return Vec::new();
    }
    match settings {
        RuleSettings::AfterResult {
            tests,
            within_days,
            look_back_days,
        } => record
            .results
            .iter()
            .filter(|result| result.abnormal && tests.contains(&result.code))
            .filter(|result| result.effective <= today && today - result.effective <= Duration::days(*look_back_days))
            .filter(|result| !booked(record, "FOLLOWUP", result.effective))
            .map(|result| Due {
                source: format!("Observation/{}", result.fhir_id),
                reason: format!("Abnormal {} result on {}", result.name, result.effective),
                due_by: result.effective + Duration::days(*within_days),
            })
            .collect(),
        RuleSettings::ScreeningRecall {
            screening,
            screening_name,
            every_months,
            min_age,
            max_age,
            sex,
            within_days,
        } => {
            let age = age_on(record.patient.birth_date, today);
            let sex_matches = sex.is_none_or(|sex| sex == record.patient.sex);
            let last = record
                .screenings
                .iter()
                .filter(|done| &done.code == screening && done.performed <= today)
                .map(|done| done.performed)
                .max();
            let overdue = last.is_none_or(|last| months_between(last, today) >= *every_months);
            if !(sex_matches && (*min_age..=*max_age).contains(&age) && overdue) || booked(record, "ROUTINE", today) {
                return Vec::new();
            }
            let reason = match last {
                Some(last) => format!("{screening_name} last recorded on {last}, over {every_months} months ago"),
                None => format!("No {screening_name} on record"),
            };
            vec![Due {
                source: format!("screening/{screening}"),
                reason,
                due_by: today + Duration::days(*within_days),
            }]
        }
        RuleSettings::AfterDischarge {
            within_days,
            look_back_days,
        } => record
            .stays
            .iter()
            .filter_map(|stay| stay.discharged.map(|day| (stay, day)))
            .filter(|(_, day)| *day <= today && today - *day <= Duration::days(*look_back_days))
            .filter(|(_, day)| !booked(record, "FOLLOWUP", *day))
            .map(|(stay, day)| Due {
                source: format!("Encounter/{}", stay.fhir_id),
                reason: format!("Discharged from hospital on {day}"),
                due_by: day + Duration::days(*within_days),
            })
            .collect(),
    }
}

/// Whether the patient has an appointment of type `kind`, booked or already
/// fulfilled, on or after `since`.
pub fn booked(record: &PatientRecord, kind: &str, since: Date) -> bool {
    record.appointments.iter().any(|appointment| {
        appointment.kind == kind
            && appointment.status != AppointmentStatus::Cancelled
            && appointment.start.date() >= since
    })
}

/// Whole years from `birth` to `day`.
fn age_on(birth: Date, day: Date) -> i32 {
    let had_birthday = (day.month() as u8, day.day()) >= (birth.month() as u8, birth.day());
    day.year() - birth.year() - i32::from(!had_birthday)
}

/// Whole months from `from` to `to`.
fn months_between(from: Date, to: Date) -> i32 {
    let months = (to.year() - from.year()) * 12 + (to.month() as i32 - from.month() as i32);
    if to.day() < from.day() { months - 1 } else { months }
}
