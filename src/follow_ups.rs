//! Follow-ups (B2–B5): the patients the clinic's rules choose, each with the
//! reason and the rule version, never twice, and stopped once the patient
//! books. Only rules choose; nothing here asks the model anything.

use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use time::Date;

use crate::engine::audit::{self, AuditEvent};
use crate::engine::clock::Clock;
use crate::engine::id_type;
use crate::fhir::{Kind, PatientRecord};
use crate::records::ClinicId;
use crate::rules::{Rules, booked, due};

id_type!(
    /// One follow-up.
    FollowUpId
);

/// Why a follow-up's reminders stopped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stop {
    /// The patient booked the visit.
    Booked,
    /// The patient replied.
    Replied,
    /// The patient opted out of this kind of reminder.
    OptedOut,
}

impl Stop {
    fn code(self) -> &'static str {
        match self {
            Self::Booked => "booked",
            Self::Replied => "replied",
            Self::OptedOut => "opted-out",
        }
    }

    fn from_code(code: &str) -> Option<Self> {
        [Self::Booked, Self::Replied, Self::OptedOut]
            .into_iter()
            .find(|stop| stop.code() == code)
    }
}

/// One follow-up in a clinic's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FollowUpRow {
    /// Its ID.
    pub id: FollowUpId,
    /// The patient's name.
    pub patient_name: String,
    /// Its kind.
    pub kind: Kind,
    /// Why, for staff.
    pub reason: String,
    /// When the care is due by.
    pub due_by: Date,
    /// The rule that chose it.
    pub rule_name: String,
    /// The version of that rule that chose it.
    pub rule_version: i32,
    /// Why its reminders stopped, if they have.
    pub stopped: Option<Stop>,
}

/// What choosing did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Refreshed {
    /// New follow-ups chosen.
    pub selected: usize,
    /// Open follow-ups stopped because the patient booked.
    pub booked: usize,
}

/// The clinics' follow-ups.
#[derive(Clone)]
pub struct FollowUps {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

impl FollowUps {
    /// The follow-ups in `pool`.
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// Applies `clinic`'s current rules to its patients whose records are
    /// current: chooses the new follow-ups, and stops open ones whose
    /// patient has since booked. Each step is logged, as done by `actor`.
    pub async fn refresh(&self, clinic: ClinicId, actor: &str) -> Result<Refreshed, sqlx::Error> {
        let now = self.clock.now();
        let today = now.date();
        let rules = Rules::new(self.pool.clone(), self.clock.clone())
            .current(clinic)
            .await?;
        let patients = sqlx::query!(
            "select id, record from patients where tenant_id = $1 and not_current is null order by id",
            clinic.as_uuid()
        )
        .fetch_all(&self.pool)
        .await?;
        let mut done = Refreshed::default();
        let mut tx = self.pool.begin().await?;
        for patient in patients {
            // A stored record that can't be read marks that patient alone as
            // not current; the others are still chosen.
            let record: PatientRecord = match serde_json::from_value(patient.record) {
                Ok(record) => record,
                Err(err) => {
                    sqlx::query!(
                        "update patients set not_current = $3 where tenant_id = $1 and id = $2",
                        clinic.as_uuid(),
                        patient.id,
                        format!("their stored record can't be read: {err}")
                    )
                    .execute(&mut *tx)
                    .await?;
                    continue;
                }
            };
            for rule in &rules {
                for found in due(&rule.settings, &record, today) {
                    let id = FollowUpId::new();
                    let added = sqlx::query!(
                        "insert into follow_ups (id, tenant_id, patient_id, rule_id, rule_version, kind, source,
                                                 reason, since, due_by, selected_at)
                         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                         on conflict (tenant_id, patient_id, rule_id, source) do nothing",
                        id.as_uuid(),
                        clinic.as_uuid(),
                        patient.id,
                        rule.id.as_uuid(),
                        rule.version,
                        rule.settings.kind().code(),
                        found.source,
                        found.reason,
                        found.since,
                        found.due_by,
                        now,
                    )
                    .execute(&mut *tx)
                    .await?;
                    if added.rows_affected() == 1 {
                        done.selected += 1;
                        let details = json!({
                            "rule": rule.name, "rule_version": rule.version,
                            "reason": found.reason, "due_by": found.due_by.to_string(),
                        });
                        log(&mut tx, clinic, now, actor, "follow_up.selected", id, details).await?;
                    }
                }
            }
            let open = sqlx::query!(
                "select id, kind, since from follow_ups
                 where tenant_id = $1 and patient_id = $2 and stopped_at is null",
                clinic.as_uuid(),
                patient.id
            )
            .fetch_all(&mut *tx)
            .await?;
            for follow_up in open {
                let kind = Kind::from_code(&follow_up.kind).unwrap_or(Kind::AfterResult);
                let stop = if record.patient.opted_out.contains(&kind) {
                    Stop::OptedOut
                } else if booked(&record, booking_type(kind), follow_up.since) {
                    Stop::Booked
                } else {
                    continue;
                };
                sqlx::query!(
                    "update follow_ups set stopped_at = $3, stop = $4 where tenant_id = $1 and id = $2",
                    clinic.as_uuid(),
                    follow_up.id,
                    now,
                    stop.code()
                )
                .execute(&mut *tx)
                .await?;
                let (action, details) = match stop {
                    Stop::Booked => {
                        done.booked += 1;
                        ("follow_up.booked", json!({}))
                    }
                    _ => ("follow_up.opted_out", json!({ "from": "the patient's record" })),
                };
                log(
                    &mut tx,
                    clinic,
                    now,
                    actor,
                    action,
                    FollowUpId::from(follow_up.id),
                    details,
                )
                .await?;
            }
        }
        tx.commit().await?;
        Ok(done)
    }

    /// `clinic`'s follow-ups, soonest due first.
    pub async fn list(&self, clinic: ClinicId) -> Result<Vec<FollowUpRow>, sqlx::Error> {
        let rows = sqlx::query!(
            "select f.id, p.name as patient_name, f.kind, f.reason, f.due_by, r.name as rule_name, f.rule_version, f.stop
             from follow_ups f
             join patients p on p.tenant_id = f.tenant_id and p.id = f.patient_id
             join rules r on r.tenant_id = f.tenant_id and r.id = f.rule_id
             where f.tenant_id = $1
             order by f.due_by, p.name, f.id",
            clinic.as_uuid()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| FollowUpRow {
                id: row.id.into(),
                patient_name: row.patient_name,
                kind: Kind::from_code(&row.kind).unwrap_or(Kind::AfterResult),
                reason: row.reason,
                due_by: row.due_by,
                rule_name: row.rule_name,
                rule_version: row.rule_version,
                stopped: row.stop.as_deref().and_then(Stop::from_code),
            })
            .collect())
    }
}

/// The appointment type that answers a kind of follow-up.
fn booking_type(kind: Kind) -> &'static str {
    match kind {
        Kind::ScreeningRecall => "ROUTINE",
        Kind::AfterResult | Kind::AfterDischarge => "FOLLOWUP",
    }
}

async fn log(
    tx: &mut sqlx::PgConnection,
    clinic: ClinicId,
    at: time::OffsetDateTime,
    actor: &str,
    action: &str,
    id: FollowUpId,
    details: serde_json::Value,
) -> Result<(), sqlx::Error> {
    let event = AuditEvent {
        tenant: clinic,
        at,
        actor,
        action,
        subject_type: "follow_up",
        subject_id: id.as_uuid(),
        details,
    };
    audit::record(&mut *tx, &event).await
}
