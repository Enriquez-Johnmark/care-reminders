//! The activity log as staff read it (G1–G3): each follow-up's timeline and
//! the clinic-wide log. The entries are the engine's append-only audit log,
//! which the database refuses to change or delete (G2).

use serde_json::Value;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::follow_ups::FollowUpId;
use crate::records::ClinicId;

/// One entry, as the pages show it.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// When.
    pub at: OffsetDateTime,
    /// Who: a person's address, `model`, `system`, `operator` or `hourly fetch`.
    pub actor: String,
    /// What, e.g. `reminder.approved`.
    pub action: String,
    /// What kind of thing it is about: `follow_up`, `reminder`, `rule` or
    /// `fhir_server`.
    pub subject_type: String,
    /// Which one.
    pub subject_id: uuid::Uuid,
    /// Extra facts.
    pub details: Value,
}

/// How many entries the clinic-wide log shows at most.
pub const LIMIT: i64 = 500;

/// The clinics' activity.
#[derive(Clone)]
pub struct Activity {
    pool: PgPool,
}

impl Activity {
    /// The activity in `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// `follow_up`'s entries and its reminders', oldest first (G3).
    pub async fn timeline(&self, clinic: ClinicId, follow_up: FollowUpId) -> Result<Vec<Entry>, sqlx::Error> {
        let rows = sqlx::query!(
            "select at, actor, action, subject_type, subject_id, details from audit_events
             where tenant_id = $1
               and ((subject_type = 'follow_up' and subject_id = $2)
                 or (subject_type = 'reminder' and subject_id in
                       (select id from reminders where tenant_id = $1 and follow_up_id = $2)))
             order by at, id",
            clinic.as_uuid(),
            follow_up.as_uuid(),
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| Entry {
                at: r.at,
                actor: r.actor,
                action: r.action,
                subject_type: r.subject_type,
                subject_id: r.subject_id,
                details: r.details,
            })
            .collect())
    }

    /// `clinic`'s latest entries, newest first, optionally only one person's
    /// or one step's (G3).
    pub async fn clinic_log(
        &self,
        clinic: ClinicId,
        actor: Option<&str>,
        action: Option<&str>,
    ) -> Result<Vec<Entry>, sqlx::Error> {
        let rows = sqlx::query!(
            "select at, actor, action, subject_type, subject_id, details from audit_events
             where tenant_id = $1 and ($2::text is null or actor = $2) and ($3::text is null or action = $3)
             order by at desc, id desc limit $4",
            clinic.as_uuid(),
            actor,
            action,
            LIMIT,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| Entry {
                at: r.at,
                actor: r.actor,
                action: r.action,
                subject_type: r.subject_type,
                subject_id: r.subject_id,
                details: r.details,
            })
            .collect())
    }
}
