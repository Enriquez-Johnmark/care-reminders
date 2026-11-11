//! The care coordinator's review (D1–D6): approve the exact text, edit it,
//! use the clinic's template, ask for a new draft, or skip the reminder.
//! Every step is logged (G1).

use std::sync::Arc;

use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::checks::{Problem, check};
use crate::drafting::{Tone, template};
use crate::engine::accounts::{MemberError, MemberId, Members, Session};
use crate::engine::audit::{self, AuditEvent};
use crate::engine::clock::Clock;
use crate::engine::llm::LanguageModel;
use crate::engine::mail::Email;
use crate::engine::outbox::Outbox;
use crate::fhir::Kind;
use crate::records::ClinicId;
use crate::reminders::{DraftId, NewDraft, ReminderId, add_draft, write_draft};
use crate::sending::{Sending, SendingRules, release};

/// Most new drafts a coordinator may ask the model for, after the first
/// draft (D5).
pub const MAX_NEW_DRAFTS: i64 = 2;

/// Why a review step was refused.
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    /// Only a care coordinator may do this (D1).
    #[error("only a care coordinator can do this")]
    NotACoordinator,
    /// The draft failed a check (D2, D4).
    #[error("this draft failed a check, so it can't be approved")]
    Blocked,
    /// The text isn't the one approved, or a newer draft exists (D3).
    #[error("the reminder changed since you read it; review the latest text")]
    Changed,
    /// The reminder was already sent or skipped.
    #[error("this reminder is already finished")]
    Finished,
    /// The model has written its last draft (D5).
    #[error("no more new drafts for this reminder; edit it or use the template")]
    NoMoreDrafts,
    /// The follow-up stopped: the patient booked, replied or opted out.
    #[error("this follow-up has stopped, so its reminders can't be changed or approved")]
    Stopped,
    /// The text can't be filled from the clinic's data, so it could never be
    /// sent (D3): e.g. the clinic's phone isn't set.
    #[error("this reminder can't be sent as it stands: {0}")]
    CantFill(String),
    /// No model is set up to ask for a new draft (D5).
    #[error("no model is set up for new drafts; edit the reminder or use the template")]
    NoModel,
    /// A coordinator's name must be 1 to 100 characters.
    #[error("a coordinator needs a name of 1 to 100 characters")]
    BadName,
    /// A skip needs a reason (D6).
    #[error("say why the reminder is skipped")]
    NoReason,
    /// This server can't send reminders: it has no signing key.
    #[error("sending isn't set up on this server")]
    NoSending,
    /// The outbox failed.
    #[error(transparent)]
    Outbox(#[from] crate::engine::outbox::OutboxError),
    /// No such reminder or draft in this clinic.
    #[error("not found")]
    NotFound,
    /// Adding the coordinator failed.
    #[error(transparent)]
    Member(#[from] MemberError),
    /// The database failed.
    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// A coordinator's text, stored and checked.
#[derive(Debug, Clone, PartialEq)]
pub struct Edited {
    /// The new draft.
    pub draft: DraftId,
    /// Whether a check blocked it (D4).
    pub blocked: bool,
    /// What the checks found.
    pub problems: Vec<Problem>,
    /// Warnings.
    pub warnings: Vec<String>,
}

/// One approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    /// The draft approved.
    pub draft: DraftId,
    /// The coordinator.
    pub approved_by: MemberId,
    /// When.
    pub approved_at: OffsetDateTime,
}

/// The fingerprint of a draft's exact text, which an approval names (D3).
pub fn content_hash(text: &str) -> String {
    hex::encode(Sha256::digest(text.as_bytes()))
}

/// What a review step needs to know about a reminder.
struct Current {
    follow_up: uuid::Uuid,
    follow_up_kind: String,
    email: Option<String>,
    no_email: bool,
    stopped: bool,
    given_name: String,
    due_by: time::Date,
    number: i16,
    finished: bool,
    draft: uuid::Uuid,
    attempt: i32,
    text: String,
    blocked: bool,
}

/// The review of a clinic's reminders.
#[derive(Clone)]
pub struct Reviews {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    sending: Option<Sending>,
}

impl Reviews {
    /// Reviews in `pool`.
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self {
            pool,
            clock,
            sending: None,
        }
    }

    /// Sends approved reminders with `sending`'s signed links.
    #[must_use]
    pub fn with_sending(mut self, sending: Sending) -> Self {
        self.sending = Some(sending);
        self
    }

    /// Adds a care coordinator to the clinic, as a staff member who may
    /// approve.
    pub async fn add_coordinator(
        &self,
        clinic: ClinicId,
        email: &str,
        name: &str,
        by: &str,
    ) -> Result<MemberId, ReviewError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 100 {
            return Err(ReviewError::BadName);
        }
        let member = Members::new(self.pool.clone(), self.clock.clone())
            .add_staff(clinic, email, by)
            .await?;
        sqlx::query!(
            "insert into coordinators (member_id, tenant_id, name, created_at) values ($1, $2, $3, $4)",
            member.id.as_uuid(),
            clinic.as_uuid(),
            name.trim(),
            self.clock.now(),
        )
        .execute(&self.pool)
        .await?;
        Ok(member.id)
    }

    /// The coordinator's name, if `who` is one of their clinic's.
    pub async fn coordinator_name(&self, who: &Session) -> Result<Option<String>, ReviewError> {
        Ok(sqlx::query_scalar!(
            "select name from coordinators where tenant_id = $1 and member_id = $2",
            who.tenant.as_uuid(),
            who.member.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    async fn require_coordinator(&self, who: &Session) -> Result<(), ReviewError> {
        match self.coordinator_name(who).await? {
            Some(_) => Ok(()),
            None => Err(ReviewError::NotACoordinator),
        }
    }

    /// The reminder and its latest draft, in `who`'s clinic.
    async fn current(
        &self,
        tx: &mut sqlx::PgConnection,
        who: &Session,
        reminder: ReminderId,
    ) -> Result<Current, ReviewError> {
        let row = sqlx::query!(
            "select f.id as follow_up, p.email, p.no_email, f.kind, f.stopped_at is not null as \"stopped!\", f.due_by, p.given_name,
                    r.number, r.finished_at is not null as \"finished!\", d.id as draft, d.attempt, d.text,
                    jsonb_array_length(d.problems) > 0 as \"blocked!\"
             from reminders r
             join follow_ups f on f.tenant_id = r.tenant_id and f.id = r.follow_up_id
             join patients p on p.tenant_id = f.tenant_id and p.id = f.patient_id
             join drafts d on d.tenant_id = r.tenant_id and d.reminder_id = r.id
             where r.tenant_id = $1 and r.id = $2
             order by d.attempt desc limit 1
             for update of r",
            who.tenant.as_uuid(),
            reminder.as_uuid(),
        )
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ReviewError::NotFound)?;
        let current = Current {
            follow_up: row.follow_up,
            follow_up_kind: row.kind,
            email: row.email,
            no_email: row.no_email,
            stopped: row.stopped,
            given_name: row.given_name,
            due_by: row.due_by,
            number: row.number,
            finished: row.finished,
            draft: row.draft,
            attempt: row.attempt,
            text: row.text,
            blocked: row.blocked,
        };
        if current.stopped {
            return Err(ReviewError::Stopped);
        }
        Ok(current)
    }

    /// Approves `draft` of `reminder`, if it is the latest, passed its checks
    /// and its text still hashes to `hash`, the text the coordinator read.
    pub async fn approve(
        &self,
        who: &Session,
        reminder: ReminderId,
        draft: DraftId,
        hash: &str,
    ) -> Result<(), ReviewError> {
        let mut tx = self.pool.begin().await?;
        let current = self.current(&mut tx, who, reminder).await?;
        self.require_coordinator(who).await?;
        if current.finished {
            return Err(ReviewError::Finished);
        }
        if current.draft != draft.as_uuid() || content_hash(&current.text) != hash {
            return Err(ReviewError::Changed);
        }
        if current.blocked {
            return Err(ReviewError::Blocked);
        }
        let sending = self.sending.as_ref().ok_or(ReviewError::NoSending)?;
        let follow_up = crate::follow_ups::FollowUpId::from(current.follow_up);
        // Approving only what could be sent: every fill-in has a value.
        let account = crate::engine::accounts::Accounts::new(self.pool.clone(), self.clock.clone())
            .get(who.tenant)
            .await
            .map_err(|err| ReviewError::CantFill(err.to_string()))?;
        let settings = crate::settings::get(&self.pool, who.tenant).await?;
        let values = crate::drafting::FillIns {
            first_name: current.given_name.clone(),
            clinic_name: account.name,
            clinic_phone: settings.phone,
            due_by: current.due_by,
            opt_out_link: sending.opt_out_link(follow_up),
        };
        let body =
            crate::drafting::fill(&current.text, &values).map_err(|err| ReviewError::CantFill(err.to_string()))?;
        let clinic_name = values.clinic_name;
        let now = self.clock.now();
        sqlx::query!(
            "insert into approvals (draft_id, tenant_id, reminder_id, content_hash, approved_by, approved_at)
             values ($1, $2, $3, $4, $5, $6) on conflict (draft_id) do nothing",
            draft.as_uuid(),
            who.tenant.as_uuid(),
            reminder.as_uuid(),
            hash,
            who.member.as_uuid(),
            now,
        )
        .execute(&mut *tx)
        .await?;
        log(
            &mut tx,
            who,
            now,
            "reminder.approved",
            reminder,
            json!({ "draft": draft, "content_hash": hash }),
        )
        .await?;
        let address = current.email.filter(|_| !current.no_email);
        let Some(address) = address else {
            // E5: never emailed; the coordinator contacts the patient another way.
            sqlx::query!(
                "update reminders set finished_at = $3, outcome = 'contact-another-way' where tenant_id = $1 and id = $2",
                who.tenant.as_uuid(),
                reminder.as_uuid(),
                now,
            )
            .execute(&mut *tx)
            .await?;
            log(
                &mut tx,
                who,
                now,
                "reminder.not_emailed",
                reminder,
                json!({ "draft": draft }),
            )
            .await?;
            tx.commit().await?;
            return Ok(());
        };
        let already = sqlx::query_scalar!(
            "select message_id from reminder_messages where tenant_id = $1 and draft_id = $2",
            who.tenant.as_uuid(),
            draft.as_uuid(),
        )
        .fetch_optional(&mut *tx)
        .await?;
        if already.is_some() {
            return Ok(());
        }
        // ADR 0001: the email is stored with the approval, and released to
        // the outbox once both are committed (or by the worker's sweep).
        let outbox = Outbox::new(self.pool.clone(), self.clock.clone());
        let email = Email {
            to_address: address,
            from_name: clinic_name.clone(),
            reply_to: Some(sending.reply_to(follow_up)),
            subject: format!("A message from {clinic_name}"),
            body_text: body,
        };
        let message = outbox.create_draft_in(&mut tx, who.tenant, &email, None).await?;
        sqlx::query!(
            "insert into reminder_messages (message_id, tenant_id, reminder_id, draft_id, created_at)
             values ($1, $2, $3, $4, $5)",
            message.as_uuid(),
            who.tenant.as_uuid(),
            reminder.as_uuid(),
            draft.as_uuid(),
            now,
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let rules = SendingRules::new(self.pool.clone(), self.clock.clone());
        if let Err(err) = release(&outbox, &rules, who.tenant, message, &who.email).await {
            tracing::warn!(%err, %message, "an approved reminder waits for the next sweep");
        }
        Ok(())
    }

    /// The approval of `reminder`'s latest draft, if its text still hashes to
    /// what was approved (D3); `None` once a newer draft exists.
    pub async fn current_approval(
        &self,
        clinic: ClinicId,
        reminder: ReminderId,
    ) -> Result<Option<Approval>, ReviewError> {
        let row = sqlx::query!(
            "select a.draft_id, a.content_hash, a.approved_by, a.approved_at, d.text
             from approvals a join drafts d on d.tenant_id = a.tenant_id and d.id = a.draft_id
             where a.tenant_id = $1 and a.reminder_id = $2
               and d.attempt = (select max(attempt) from drafts where tenant_id = $1 and reminder_id = $2)",
            clinic.as_uuid(),
            reminder.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row
            .filter(|row| content_hash(&row.text) == row.content_hash)
            .map(|row| Approval {
                draft: row.draft_id.into(),
                approved_by: row.approved_by.into(),
                approved_at: row.approved_at,
            }))
    }

    /// Stores the coordinator's own text as `reminder`'s latest draft,
    /// checked like any other (D4).
    pub async fn edit(&self, who: &Session, reminder: ReminderId, text: &str) -> Result<Edited, ReviewError> {
        let text = text.trim().to_owned();
        let checked = check(&text);
        let draft = NewDraft {
            source: "coordinator",
            model: None,
            text,
            checked,
        };
        self.replace(who, reminder, &draft, "reminder.edited").await
    }

    /// Makes the clinic's template `reminder`'s latest draft (D5).
    pub async fn use_template(&self, who: &Session, reminder: ReminderId) -> Result<Edited, ReviewError> {
        let number = {
            let mut tx = self.pool.begin().await?;
            self.current(&mut tx, who, reminder).await?.number
        };
        let text = template(number as u8);
        let checked = check(&text);
        let draft = NewDraft {
            source: "template",
            model: None,
            text,
            checked,
        };
        self.replace(who, reminder, &draft, "reminder.templated").await
    }

    /// Asks `model` for a new draft of `reminder`: at most two after the
    /// first (D5). With no model, the template.
    pub async fn new_draft<M: LanguageModel>(
        &self,
        who: &Session,
        reminder: ReminderId,
        model: Option<&M>,
    ) -> Result<Edited, ReviewError> {
        self.require_coordinator(who).await?;
        let Some(model) = model else {
            return Err(ReviewError::NoModel);
        };
        let (kind, number) = {
            let mut tx = self.pool.begin().await?;
            let current = self.current(&mut tx, who, reminder).await?;
            if current.finished {
                return Err(ReviewError::Finished);
            }
            (current.follow_up_kind, current.number)
        };
        let new_drafts = sqlx::query_scalar!(
            "select count(*) as \"count!\" from drafts
             where tenant_id = $1 and reminder_id = $2 and source = 'model' and attempt > 1",
            who.tenant.as_uuid(),
            reminder.as_uuid(),
        )
        .fetch_one(&self.pool)
        .await?;
        if new_drafts >= MAX_NEW_DRAFTS {
            return Err(ReviewError::NoMoreDrafts);
        }
        let tone = sqlx::query_scalar!(
            "select tone from clinic_settings where tenant_id = $1",
            who.tenant.as_uuid()
        )
        .fetch_optional(&self.pool)
        .await?;
        let tone = if tone.as_deref() == Some("plain") {
            Tone::Plain
        } else {
            Tone::Warm
        };
        let kind = Kind::from_code(&kind).unwrap_or(Kind::AfterResult);
        let draft = write_draft(
            &self.pool,
            self.clock.as_ref(),
            Some(model),
            who.tenant,
            kind,
            number as u8,
            tone,
        )
        .await?;
        self.replace(who, reminder, &draft, "reminder.redrafted").await
    }

    /// Skips `reminder`, saying why (D6): it is finished, never sent, and the
    /// next one can follow.
    pub async fn skip(&self, who: &Session, reminder: ReminderId, reason: &str) -> Result<(), ReviewError> {
        let reason = reason.trim();
        let mut tx = self.pool.begin().await?;
        let current = self.current(&mut tx, who, reminder).await?;
        self.require_coordinator(who).await?;
        if reason.is_empty() {
            return Err(ReviewError::NoReason);
        }
        if current.finished {
            return Err(ReviewError::Finished);
        }
        let now = self.clock.now();
        sqlx::query!(
            "update reminders set finished_at = $3, outcome = 'skipped', skip_reason = $4
             where tenant_id = $1 and id = $2",
            who.tenant.as_uuid(),
            reminder.as_uuid(),
            now,
            reason,
        )
        .execute(&mut *tx)
        .await?;
        log(
            &mut tx,
            who,
            now,
            "reminder.skipped",
            reminder,
            json!({ "reason": reason }),
        )
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Stores `draft` as `reminder`'s newest, logged as `action`.
    async fn replace(
        &self,
        who: &Session,
        reminder: ReminderId,
        draft: &NewDraft,
        action: &str,
    ) -> Result<Edited, ReviewError> {
        let mut tx = self.pool.begin().await?;
        let current = self.current(&mut tx, who, reminder).await?;
        self.require_coordinator(who).await?;
        if current.finished {
            return Err(ReviewError::Finished);
        }
        let now = self.clock.now();
        let actor = if draft.source == "model" {
            "model"
        } else {
            who.email.as_str()
        };
        let id = add_draft(&mut tx, who.tenant, reminder, current.attempt + 1, draft, now, actor).await?;
        log(
            &mut tx,
            who,
            now,
            action,
            reminder,
            json!({ "draft": id, "source": draft.source }),
        )
        .await?;
        tx.commit().await?;
        Ok(Edited {
            draft: id,
            blocked: !draft.checked.problems.is_empty(),
            problems: draft.checked.problems.clone(),
            warnings: draft.checked.warnings.clone(),
        })
    }
}

async fn log(
    tx: &mut sqlx::PgConnection,
    who: &Session,
    at: OffsetDateTime,
    action: &str,
    reminder: ReminderId,
    details: serde_json::Value,
) -> Result<(), sqlx::Error> {
    let event = AuditEvent {
        tenant: who.tenant,
        at,
        actor: &who.email,
        action,
        subject_type: "reminder",
        subject_id: reminder.as_uuid(),
        details,
    };
    audit::record(&mut *tx, &event).await
}
