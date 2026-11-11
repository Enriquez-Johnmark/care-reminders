//! Each clinic's patients and their records, as last fetched (A4, H2).
//!
//! Every function takes the clinic and every query filters on it.

use std::sync::Arc;

use sqlx::PgPool;

use crate::engine::clock::Clock;
use crate::engine::id_type;
use crate::fhir::PatientRecord;

/// A clinic: the engine's tenant.
pub type ClinicId = crate::engine::ids::TenantId;

id_type!(
    /// A patient of one clinic.
    PatientId
);

/// One patient in a clinic's list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatientSummary {
    /// Their ID here.
    pub id: PatientId,
    /// Their FHIR id on the clinic's server.
    pub fhir_id: String,
    /// Their name.
    pub name: String,
    /// Why their stored record may be out of date; `None` while current.
    pub not_current: Option<String>,
}

/// What saving a record did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Saved {
    /// A patient not seen before.
    New,
    /// A known patient whose record changed.
    Updated,
    /// A known patient whose record is the same.
    Unchanged,
}

/// The clinics' patient records.
#[derive(Clone)]
pub struct Records {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

impl Records {
    /// The records in `pool`.
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    /// Saves `record` for `clinic`: a new patient, or the latest record of a
    /// known one (matched by FHIR id), never a second copy.
    pub async fn save(&self, clinic: ClinicId, record: &PatientRecord) -> Result<Saved, sqlx::Error> {
        let now = self.clock.now();
        let patient = &record.patient;
        let json = serde_json::to_value(record).map_err(|err| sqlx::Error::Encode(err.into()))?;
        let mut tx = self.pool.begin().await?;
        let before = sqlx::query_scalar!(
            "select record from patients where tenant_id = $1 and fhir_id = $2 for update",
            clinic.as_uuid(),
            patient.fhir_id,
        )
        .fetch_optional(&mut *tx)
        .await?;
        // Whether this insert made the row decides "new", so two fetches at
        // once can't both count the same patient as new.
        let inserted = sqlx::query_scalar!(
            "insert into patients (id, tenant_id, fhir_id, name, given_name, email, no_email, record, fetched_at, updated_at)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
             on conflict (tenant_id, fhir_id) do update set
                 name = excluded.name, given_name = excluded.given_name, email = excluded.email,
                 no_email = excluded.no_email, record = excluded.record, fetched_at = excluded.fetched_at,
                 not_current = null,
                 updated_at = case when patients.record = excluded.record then patients.updated_at
                                   else excluded.updated_at end
             returning (xmax = 0) as \"inserted!\"",
            PatientId::new().as_uuid(),
            clinic.as_uuid(),
            patient.fhir_id,
            patient.name,
            patient.given_name,
            patient.email,
            patient.no_email,
            json,
            now,
        )
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(match before {
            _ if inserted => Saved::New,
            Some(before) if before == json => Saved::Unchanged,
            _ => Saved::Updated,
        })
    }

    /// `clinic`'s patients, by name.
    pub async fn list(&self, clinic: ClinicId) -> Result<Vec<PatientSummary>, sqlx::Error> {
        let rows = sqlx::query!(
            "select id, fhir_id, name, not_current from patients where tenant_id = $1 order by name, id",
            clinic.as_uuid()
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| PatientSummary {
                id: row.id.into(),
                fhir_id: row.fhir_id,
                name: row.name,
                not_current: row.not_current,
            })
            .collect())
    }

    /// Marks `clinic`'s patient `fhir_id`, if stored, as not current, for
    /// `reason`.
    pub async fn mark_not_current(&self, clinic: ClinicId, fhir_id: &str, reason: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            "update patients set not_current = $3 where tenant_id = $1 and fhir_id = $2",
            clinic.as_uuid(),
            fhir_id,
            reason
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Marks every one of `clinic`'s patients not in `seen` as not current,
    /// for `reason`; returns how many.
    pub async fn mark_unseen(&self, clinic: ClinicId, seen: &[String], reason: &str) -> Result<u64, sqlx::Error> {
        let done = sqlx::query!(
            "update patients set not_current = $3
             where tenant_id = $1 and not (fhir_id = any($2)) and not_current is distinct from $3",
            clinic.as_uuid(),
            seen,
            reason
        )
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected())
    }

    /// One of `clinic`'s patients' records; `None` for another clinic's.
    pub async fn get(&self, clinic: ClinicId, id: PatientId) -> Result<Option<PatientRecord>, sqlx::Error> {
        let record = sqlx::query_scalar!(
            "select record from patients where tenant_id = $1 and id = $2",
            clinic.as_uuid(),
            id.as_uuid()
        )
        .fetch_optional(&self.pool)
        .await?;
        record
            .map(|json| serde_json::from_value(json).map_err(|err| sqlx::Error::Decode(err.into())))
            .transpose()
    }
}
