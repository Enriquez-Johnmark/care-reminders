//! Sending (E1, E3–E5, E7): the email an approved reminder becomes, the rules
//! checked at approval and again just before it goes, and marking it sent.

use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::checks::{Check, check};
use crate::engine::audit::{self, AuditEvent};
use crate::engine::clock::Clock;
use crate::engine::files::MasterKey;
use crate::engine::ids::{MessageId, TenantId};
use crate::engine::mail::Email;
use crate::engine::outbox::{BoxFuture, CheckFailed, Outbox, OutboxError, PreSendCheck};
use crate::engine::signed::Signer;
use crate::follow_ups::FollowUpId;
use crate::review::content_hash;
use crate::sending_hours::{HoursError, next_day, next_window};

/// Most reminders a patient gets in one clinic day (E3).
pub const PER_DAY: usize = 1;
/// Most reminders a patient gets in any seven days (E3).
pub const PER_WEEK: usize = 3;
/// How long a reminder to a patient whose record isn't current waits before
/// it is checked again.
pub const NOT_CURRENT_WAIT: Duration = Duration::hours(1);

/// Where a reminder's links point, and how they are signed.
#[derive(Clone)]
pub struct Sending {
    base_url: String,
    reply_domain: String,
    opt_out: Arc<Signer>,
    reply: Arc<Signer>,
}

impl Sending {
    /// Opt-out links under `base_url` and reply addresses at `reply_domain`,
    /// each signed with its own key derived from `master`.
    pub fn new(master: &MasterKey, base_url: &str, reply_domain: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            reply_domain: reply_domain.to_owned(),
            opt_out: Arc::new(Signer::new(master, "care-reminders-opt-out")),
            reply: Arc::new(Signer::new(master, "care-reminders-reply")),
        }
    }

    /// The opt-out link for `follow_up` (F3).
    pub fn opt_out_link(&self, follow_up: FollowUpId) -> String {
        format!("{}/o/{}", self.base_url, self.opt_out.token(follow_up.as_uuid()))
    }

    /// The follow-up an opt-out token names, if its signature is right (F4).
    pub fn opt_out_of(&self, token: &str) -> Option<FollowUpId> {
        self.opt_out.verify(token).map(FollowUpId::from)
    }

    /// The reply-to address for `follow_up` (F2).
    pub fn reply_to(&self, follow_up: FollowUpId) -> String {
        format!("reply+{}@{}", self.reply.token(follow_up.as_uuid()), self.reply_domain)
    }

    /// The follow-up a reply address names, if its signature is right.
    pub fn reply_of(&self, address: &str) -> Option<FollowUpId> {
        let (local, domain) = address.trim().rsplit_once('@')?;
        if !domain.eq_ignore_ascii_case(&self.reply_domain) {
            return None;
        }
        self.reply.verify(local.strip_prefix("reply+")?).map(FollowUpId::from)
    }
}

/// When the next reminder to a patient may go, given when the earlier ones
/// started to go (`sent`): `None` if the caps allow one now (E3). A time it
/// waits for is moved into the sending window (E4).
pub fn cap_until(
    sent: &[OffsetDateTime],
    now: OffsetDateTime,
    time_zone: &str,
) -> Result<Option<OffsetDateTime>, HoursError> {
    let tomorrow = next_day(now, time_zone)?;
    let today = tomorrow - Duration::days(1);
    let mut until: Option<OffsetDateTime> = None;
    // The clinic's midnight a day back may be 23 or 25 hours away; the
    // window below moves the answer to the next morning anyway.
    if sent.iter().filter(|at| **at >= today && **at < tomorrow).count() >= PER_DAY {
        until = Some(tomorrow);
    }
    let mut week: Vec<OffsetDateTime> = sent
        .iter()
        .copied()
        .filter(|at| *at > now - Duration::days(7))
        .collect();
    week.sort_unstable();
    if week.len() >= PER_WEEK {
        // Once enough have aged past seven days, one more may go.
        let frees = week[week.len() - PER_WEEK] + Duration::days(7);
        until = Some(until.map_or(frees, |day| day.max(frees)));
    }
    until.map(|at| next_window(at, time_zone)).transpose()
}

/// The sending rules, checked when a reminder is approved and again just
/// before it goes. Other messages, such as login links, pass.
#[derive(Clone)]
pub struct SendingRules {
    pool: PgPool,
    clock: Arc<dyn Clock>,
}

impl SendingRules {
    /// Rules on `pool`, read at `clock`'s time.
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>) -> Self {
        Self { pool, clock }
    }

    async fn judge(&self, tenant: TenantId, id: MessageId, email: &Email) -> Result<(), CheckFailed> {
        let unavailable = |err: sqlx::Error| CheckFailed::Unavailable(err.to_string());
        let row = sqlx::query!(
            "select f.stopped_at is not null as \"stopped!\", p.id as patient, p.no_email, p.email, p.not_current,
                    d.text, a.content_hash as \"approved_hash?\",
                    d.attempt = (select max(attempt) from drafts
                                 where tenant_id = d.tenant_id and reminder_id = d.reminder_id) as \"latest!\",
                    coalesce(s.time_zone, 'America/New_York') as \"time_zone!\"
             from reminder_messages m
             join reminders r on r.tenant_id = m.tenant_id and r.id = m.reminder_id
             join follow_ups f on f.tenant_id = r.tenant_id and f.id = r.follow_up_id
             join patients p on p.tenant_id = f.tenant_id and p.id = f.patient_id
             join drafts d on d.tenant_id = m.tenant_id and d.id = m.draft_id
             left join approvals a on a.tenant_id = m.tenant_id and a.draft_id = m.draft_id
             left join clinic_settings s on s.tenant_id = m.tenant_id
             where m.tenant_id = $1 and m.message_id = $2",
            tenant.as_uuid(),
            id.as_uuid(),
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(unavailable)?;
        let Some(row) = row else { return Ok(()) };
        let refuse = |why: &str| Err(CheckFailed::Refused(why.to_owned()));
        if row.stopped {
            return refuse("the follow-up has stopped");
        }
        if row.no_email || row.email.as_deref() != Some(email.to_address.as_str()) {
            return refuse("the patient asked not to be emailed, or their address changed");
        }
        if !row.latest || row.approved_hash.as_deref() != Some(content_hash(&row.text).as_str()) {
            return refuse("the reminder changed after it was approved");
        }
        // E7, on the exact text that would go.
        if let Some(problem) = check(&email.body_text)
            .problems
            .into_iter()
            .find(|problem| problem.check == Check::ClinicalWords)
        {
            return Err(CheckFailed::Refused(format!(
                "the email names the patient's care: {}",
                problem.detail
            )));
        }
        let now = self.clock.now();
        if let Some(why) = row.not_current {
            return Err(CheckFailed::NotUntil {
                until: now + NOT_CURRENT_WAIT,
                reason: format!("the patient's record isn't current: {why}"),
            });
        }
        let sent = sqlx::query_scalar!(
            "select o.first_send_at as \"first_send_at!\"
             from reminder_messages m
             join outbox_messages o on o.tenant_id = m.tenant_id and o.id = m.message_id
             join reminders r on r.tenant_id = m.tenant_id and r.id = m.reminder_id
             join follow_ups f on f.tenant_id = r.tenant_id and f.id = r.follow_up_id
             where m.tenant_id = $1 and f.patient_id = $2 and m.message_id <> $3
               and o.first_send_at is not null and o.first_send_at > $4",
            tenant.as_uuid(),
            row.patient,
            id.as_uuid(),
            now - Duration::days(8),
        )
        .fetch_all(&self.pool)
        .await
        .map_err(unavailable)?;
        let hours = |err: HoursError| CheckFailed::Unavailable(err.to_string());
        if let Some(until) = cap_until(&sent, now, &row.time_zone).map_err(hours)? {
            return Err(CheckFailed::NotUntil {
                until,
                reason: "the patient had a reminder today, or three this week".to_owned(),
            });
        }
        let window = next_window(now, &row.time_zone).map_err(hours)?;
        if window > now {
            return Err(CheckFailed::NotUntil {
                until: window,
                reason: "reminders go out 8 am to 6 pm on weekdays, clinic time".to_owned(),
            });
        }
        Ok(())
    }
}

impl PreSendCheck for SendingRules {
    fn check<'a>(
        &'a self,
        tenant: TenantId,
        id: MessageId,
        email: &'a Email,
    ) -> BoxFuture<'a, Result<(), CheckFailed>> {
        Box::pin(self.judge(tenant, id, email))
    }
}

/// Approves an approved reminder's email in the outbox, after the approval's
/// own transaction (ADR 0001); one the rules refuse is cancelled, so it
/// never lingers. `true` if approved.
pub(crate) async fn release(
    outbox: &Outbox,
    rules: &SendingRules,
    tenant: TenantId,
    id: MessageId,
    approver: &str,
) -> Result<bool, OutboxError> {
    let message = outbox.get(tenant, id).await?;
    match outbox
        .approve(tenant, id, approver, &message.content_hash(), rules)
        .await
    {
        Ok(()) => Ok(true),
        Err(OutboxError::CheckFailed(reason)) => {
            tracing::info!(%reason, message = %id, "a reminder's email was refused");
            outbox.cancel(tenant, id, audit::SYSTEM).await?;
            Ok(false)
        }
        Err(err) => Err(err),
    }
}

/// The worker's sweep, run before and after each send: approves every
/// reminder email left waiting because an approval was interrupted between
/// its transaction and the outbox's, and finishes each reminder whose email
/// went out, logging it sent (G1), so the next one can follow. System-wide;
/// every update names the row's clinic.
pub async fn finish(pool: &PgPool, clock: Arc<dyn Clock>) -> Result<usize, OutboxError> {
    let waiting = sqlx::query!(
        "select m.tenant_id, m.message_id from reminder_messages m
         join outbox_messages o on o.tenant_id = m.tenant_id and o.id = m.message_id
         where o.status = 'draft'"
    )
    .fetch_all(pool)
    .await?;
    let outbox = Outbox::new(pool.clone(), clock.clone());
    let rules = SendingRules::new(pool.clone(), clock.clone());
    for row in waiting {
        let (tenant, id) = (TenantId::from(row.tenant_id), MessageId::from(row.message_id));
        // One that fails never holds up the rest; it's tried next round.
        if let Err(err) = release(&outbox, &rules, tenant, id, audit::SYSTEM).await {
            tracing::warn!(%err, message = %id, "couldn't release a waiting reminder");
        }
    }
    let now = clock.now();
    let mut tx = pool.begin().await?;
    let sent = sqlx::query!(
        "update reminders r set finished_at = o.sent_at, outcome = 'sent'
         from reminder_messages m join outbox_messages o on o.tenant_id = m.tenant_id and o.id = m.message_id
         where m.tenant_id = r.tenant_id and m.reminder_id = r.id
           and r.finished_at is null and o.status = 'sent' and o.sent_at is not null
         returning r.tenant_id, r.id, m.message_id"
    )
    .fetch_all(&mut *tx)
    .await?;
    for row in &sent {
        let event = AuditEvent {
            tenant: row.tenant_id.into(),
            at: now,
            actor: audit::SYSTEM,
            action: "reminder.sent",
            subject_type: "reminder",
            subject_id: row.id,
            details: json!({ "message": row.message_id }),
        };
        audit::record(&mut *tx, &event).await?;
    }
    tx.commit().await?;
    Ok(sent.len())
}

/// Withdraws every email of `clinic`'s stopped follow-ups that hasn't started
/// to go (F1): a booking, reply or opt-out means no more reminders. Each is
/// logged, as done by `actor`; returns how many.
pub async fn withdraw_stopped(
    pool: &PgPool,
    clock: Arc<dyn Clock>,
    clinic: TenantId,
    actor: &str,
) -> Result<usize, OutboxError> {
    let waiting = sqlx::query!(
        "select m.message_id, m.reminder_id, f.stop as \"stop!\"
         from reminder_messages m
         join outbox_messages o on o.tenant_id = m.tenant_id and o.id = m.message_id
         join reminders r on r.tenant_id = m.tenant_id and r.id = m.reminder_id
         join follow_ups f on f.tenant_id = r.tenant_id and f.id = r.follow_up_id
         where m.tenant_id = $1 and f.stopped_at is not null and o.status in ('draft', 'approved')",
        clinic.as_uuid(),
    )
    .fetch_all(pool)
    .await?;
    let outbox = Outbox::new(pool.clone(), clock.clone());
    for row in &waiting {
        outbox.cancel(clinic, row.message_id.into(), actor).await?;
        let event = AuditEvent {
            tenant: clinic,
            at: clock.now(),
            actor,
            action: "reminder.withdrawn",
            subject_type: "reminder",
            subject_id: row.reminder_id,
            details: json!({ "message": row.message_id, "stop": row.stop }),
        };
        audit::record(pool, &event).await?;
    }
    Ok(waiting.len())
}

/// What an opt-out link names, as its page shows it: only the clinic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptOutLink {
    /// The clinic the reminders come from.
    pub clinic_name: String,
    /// Whether the patient already opted out of this kind.
    pub done: bool,
}

/// The follow-up an opt-out link names, found from its signed code before any
/// clinic is known (the one lookup not scoped by a clinic), with its clinic,
/// patient and kind.
async fn linked(
    pool: &PgPool,
    sending: &Sending,
    token: &str,
) -> Result<Option<(TenantId, uuid::Uuid, String, String)>, sqlx::Error> {
    let Some(follow_up) = sending.opt_out_of(token) else {
        return Ok(None);
    };
    let row = sqlx::query!(
        "select f.tenant_id, f.patient_id, f.kind, a.name as clinic_name
         from follow_ups f join accounts a on a.tenant_id = f.tenant_id
         where f.id = $1",
        follow_up.as_uuid(),
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|row| (row.tenant_id.into(), row.patient_id, row.kind, row.clinic_name)))
}

/// The opt-out link's page, if `token` is a real link (F4). Reading it
/// changes nothing (F3).
pub async fn opt_out_link(pool: &PgPool, sending: &Sending, token: &str) -> Result<Option<OptOutLink>, sqlx::Error> {
    let Some((clinic, patient, kind, clinic_name)) = linked(pool, sending, token).await? else {
        return Ok(None);
    };
    let done = sqlx::query_scalar!(
        "select exists (select 1 from opt_outs where tenant_id = $1 and patient_id = $2 and kind = $3) as \"done!\"",
        clinic.as_uuid(),
        patient,
        kind,
    )
    .fetch_one(pool)
    .await?;
    Ok(Some(OptOutLink { clinic_name, done }))
}

/// Opts the patient a link names out of that kind of reminder only (F3):
/// its open follow-ups of that kind stop, logged, and their unsent emails
/// are withdrawn. `None` if `token` isn't a real link (F4).
pub async fn opt_out(
    pool: &PgPool,
    clock: Arc<dyn Clock>,
    sending: &Sending,
    token: &str,
) -> Result<Option<OptOutLink>, OutboxError> {
    let Some((clinic, patient, kind, clinic_name)) = linked(pool, sending, token).await? else {
        return Ok(None);
    };
    let now = clock.now();
    let mut tx = pool.begin().await?;
    sqlx::query!(
        "insert into opt_outs (tenant_id, patient_id, kind, opted_out_at) values ($1, $2, $3, $4)
         on conflict do nothing",
        clinic.as_uuid(),
        patient,
        kind,
        now,
    )
    .execute(&mut *tx)
    .await?;
    let stopped = sqlx::query_scalar!(
        "update follow_ups set stopped_at = $4, stop = 'opted-out'
         where tenant_id = $1 and patient_id = $2 and kind = $3 and stopped_at is null
         returning id",
        clinic.as_uuid(),
        patient,
        kind,
        now,
    )
    .fetch_all(&mut *tx)
    .await?;
    for id in stopped {
        let event = AuditEvent {
            tenant: clinic,
            at: now,
            actor: "patient",
            action: "follow_up.opted_out",
            subject_type: "follow_up",
            subject_id: id,
            details: json!({ "from": "the opt-out link" }),
        };
        audit::record(&mut *tx, &event).await?;
    }
    tx.commit().await?;
    withdraw_stopped(pool, clock, clinic, "patient").await?;
    Ok(Some(OptOutLink {
        clinic_name,
        done: true,
    }))
}
