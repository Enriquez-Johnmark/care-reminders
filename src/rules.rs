//! The clinic's follow-up rules (B1, B3, B5): who is due, by when and why.
//!
//! Each rule type is a plain function from a patient's record, the rule's
//! settings and the day to the follow-ups it finds. It makes no model call
//! and no database call: the model never decides who is contacted.

use serde::{Deserialize, Serialize};
use time::{Date, Duration};

use crate::fhir::{AppointmentStatus, Kind, PatientRecord, Sex};
use crate::records::ClinicId;

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
    /// follow-ups: e.g. `Observation/p01-result-1` or `screening/73761001/never`.
    pub source: String,
    /// Why, for staff (never for the patient).
    pub reason: String,
    /// When the care is due by.
    pub due_by: Date,
    /// From when a booking answers it: the result or discharge it follows,
    /// or for a screening the day it was found due.
    pub since: Date,
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
                since: result.effective,
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
                // Named by the last screening it follows, so the next cycle
                // is a new recall and this one never repeats (B4).
                source: format!(
                    "screening/{screening}/{}",
                    last.map_or_else(|| "never".to_owned(), |last| last.to_string())
                ),
                reason,
                due_by: today + Duration::days(*within_days),
                since: today,
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
                since: day,
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

/// A stored rule, at its current version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rule {
    /// Its ID.
    pub id: RuleId,
    /// What staff call it.
    pub name: String,
    /// Its current version, from 1.
    pub version: i32,
    /// Its current settings.
    pub settings: RuleSettings,
}

crate::engine::id_type!(
    /// One of a clinic's rules.
    RuleId
);

/// Why a rule wasn't changed.
#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    /// No such rule in this clinic.
    #[error("no such rule")]
    NotFound,
    /// The new settings are of another rule type.
    #[error("a rule keeps its type")]
    OtherType,
    /// The database failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Each clinic's rules and their versions.
#[derive(Clone)]
pub struct Rules {
    pool: sqlx::PgPool,
    clock: std::sync::Arc<dyn crate::engine::clock::Clock>,
}

impl Rules {
    /// The rules in `pool`.
    pub fn new(pool: sqlx::PgPool, clock: std::sync::Arc<dyn crate::engine::clock::Clock>) -> Self {
        Self { pool, clock }
    }

    /// Gives `clinic` the default rules it doesn't have yet, at version 1.
    pub async fn install_defaults(&self, clinic: ClinicId, actor: &str) -> Result<(), sqlx::Error> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        for (kind, name, settings) in default_rules() {
            let id = RuleId::new();
            let added = sqlx::query!(
                "insert into rules (id, tenant_id, kind, name, version) values ($1, $2, $3, $4, 1)
                 on conflict (tenant_id, name) do nothing",
                id.as_uuid(),
                clinic.as_uuid(),
                kind.code(),
                name,
            )
            .execute(&mut *tx)
            .await?;
            if added.rows_affected() == 1 {
                add_version(&mut tx, clinic, id, 1, &settings, now, actor).await?;
            }
        }
        tx.commit().await
    }

    /// `clinic`'s rules at their current versions, by name.
    pub async fn current(&self, clinic: ClinicId) -> Result<Vec<Rule>, sqlx::Error> {
        let rows = sqlx::query!(
            "select r.id, r.name, r.version, v.settings
             from rules r join rule_versions v on v.tenant_id = r.tenant_id and v.rule_id = r.id and v.version = r.version
             where r.tenant_id = $1 order by r.name",
            clinic.as_uuid()
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(Rule {
                    id: row.id.into(),
                    name: row.name,
                    version: row.version,
                    settings: serde_json::from_value(row.settings).map_err(|err| sqlx::Error::Decode(err.into()))?,
                })
            })
            .collect()
    }

    /// The settings of version `version` of `clinic`'s rule `rule`.
    pub async fn version(
        &self,
        clinic: ClinicId,
        rule: RuleId,
        version: i32,
    ) -> Result<Option<RuleSettings>, sqlx::Error> {
        let settings = sqlx::query_scalar!(
            "select settings from rule_versions where tenant_id = $1 and rule_id = $2 and version = $3",
            clinic.as_uuid(),
            rule.as_uuid(),
            version
        )
        .fetch_optional(&self.pool)
        .await?;
        settings
            .map(|json| serde_json::from_value(json).map_err(|err| sqlx::Error::Decode(err.into())))
            .transpose()
    }

    /// Changes `clinic`'s rule `rule` to `settings`, of the same type, as a
    /// new version; returns its number. Logged as done by `actor`.
    pub async fn change(
        &self,
        clinic: ClinicId,
        rule: RuleId,
        settings: &RuleSettings,
        actor: &str,
    ) -> Result<i32, RuleError> {
        let now = self.clock.now();
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query!(
            "select kind, version from rules where tenant_id = $1 and id = $2 for update",
            clinic.as_uuid(),
            rule.as_uuid()
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(RuleError::NotFound)?;
        if row.kind != settings.kind().code() {
            return Err(RuleError::OtherType);
        }
        let version = row.version + 1;
        sqlx::query!(
            "update rules set version = $3 where tenant_id = $1 and id = $2",
            clinic.as_uuid(),
            rule.as_uuid(),
            version
        )
        .execute(&mut *tx)
        .await?;
        add_version(&mut tx, clinic, rule, version, settings, now, actor).await?;
        tx.commit().await?;
        Ok(version)
    }
}

async fn add_version(
    tx: &mut sqlx::PgConnection,
    clinic: ClinicId,
    rule: RuleId,
    version: i32,
    settings: &RuleSettings,
    now: time::OffsetDateTime,
    actor: &str,
) -> Result<(), sqlx::Error> {
    let json = serde_json::to_value(settings).map_err(|err| sqlx::Error::Encode(err.into()))?;
    sqlx::query!(
        "insert into rule_versions (tenant_id, rule_id, version, settings, created_at, created_by)
         values ($1, $2, $3, $4, $5, $6)",
        clinic.as_uuid(),
        rule.as_uuid(),
        version,
        json.clone(),
        now,
        actor,
    )
    .execute(&mut *tx)
    .await?;
    let event = crate::engine::audit::AuditEvent {
        tenant: clinic,
        at: now,
        actor,
        action: if version == 1 { "rule.added" } else { "rule.changed" },
        subject_type: "rule",
        subject_id: rule.as_uuid(),
        details: serde_json::json!({ "version": version, "settings": json }),
    };
    crate::engine::audit::record(&mut *tx, &event).await
}
