//! Reminders and their drafts (E6, C1, C7): each follow-up's reminders are
//! drafted on their planned days, checked, and stored with what the checks
//! found.

use std::sync::Arc;

use serde_json::json;
use sqlx::PgPool;
use time::Duration;

use crate::checks::{Check, Checked, Problem, check};
use crate::drafting::{Tone, parse_answer, request, template};
use crate::engine::audit::{self, AuditEvent};
use crate::engine::clock::Clock;
use crate::engine::cost_log::{self, LlmCall, Outcome};
use crate::engine::id_type;
use crate::engine::llm::{LanguageModel, LlmError, Usage};
use crate::fhir::Kind;
use crate::records::ClinicId;

id_type!(
    /// One reminder of a follow-up.
    ReminderId
);
id_type!(
    /// One draft of a reminder.
    DraftId
);

/// Days after a follow-up is chosen that each reminder is planned for.
pub const PLANNED_DAYS: [i64; 3] = [0, 7, 21];

/// A reminder's latest draft.
#[derive(Debug, Clone, PartialEq)]
pub struct LatestDraft {
    /// The reminder.
    pub reminder: ReminderId,
    /// Which reminder of its follow-up, from 1.
    pub number: i16,
    /// The draft.
    pub draft: DraftId,
    /// Who wrote it: `model`, `template` or `coordinator`.
    pub source: String,
    /// Its text, with its fill-ins not yet filled.
    pub text: String,
    /// What the checks found; any one blocks it.
    pub problems: Vec<Problem>,
    /// Warnings, which don't block it.
    pub warnings: Vec<String>,
}

impl LatestDraft {
    /// Whether a check blocked it.
    pub fn blocked(&self) -> bool {
        !self.problems.is_empty()
    }
}

/// The clinics' reminders.
#[derive(Clone)]
pub struct Reminders {
    pool: PgPool,
}

impl Reminders {
    /// The reminders in `pool`.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The latest draft of each of `clinic`'s reminders.
    pub async fn latest_drafts(&self, clinic: ClinicId) -> Result<Vec<LatestDraft>, sqlx::Error> {
        let rows = sqlx::query!(
            "select distinct on (r.id) r.id as reminder, r.number, d.id as draft, d.source, d.text, d.problems, d.warnings
             from reminders r join drafts d on d.tenant_id = r.tenant_id and d.reminder_id = r.id
             where r.tenant_id = $1
             order by r.id, d.attempt desc",
            clinic.as_uuid()
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|row| {
                let decode = |err: serde_json::Error| sqlx::Error::Decode(err.into());
                Ok(LatestDraft {
                    reminder: row.reminder.into(),
                    number: row.number,
                    draft: row.draft.into(),
                    source: row.source,
                    text: row.text,
                    problems: serde_json::from_value(row.problems).map_err(decode)?,
                    warnings: serde_json::from_value(row.warnings).map_err(decode)?,
                })
            })
            .collect()
    }
}

/// Drafts each reminder that is due: the model words it, or the clinic's
/// template when there is no model.
pub struct Drafter<M> {
    pool: PgPool,
    clock: Arc<dyn Clock>,
    model: Option<M>,
}

/// A draft to store.
pub(crate) struct NewDraft {
    /// Who wrote it: `model`, `template` or `coordinator`.
    pub(crate) source: &'static str,
    /// The model that answered, if one did.
    pub(crate) model: Option<String>,
    /// Its text, with its fill-ins.
    pub(crate) text: String,
    /// What the checks found.
    pub(crate) checked: Checked,
}

/// Stores `draft` as attempt `attempt` of `reminder`, logging that it was
/// drafted, by `actor`, and what the checks found (G1).
pub(crate) async fn add_draft(
    tx: &mut sqlx::PgConnection,
    clinic: ClinicId,
    reminder: ReminderId,
    attempt: i32,
    draft: &NewDraft,
    now: time::OffsetDateTime,
    actor: &str,
) -> Result<DraftId, sqlx::Error> {
    let id = DraftId::new();
    let encode = |err: serde_json::Error| sqlx::Error::Encode(err.into());
    sqlx::query!(
        "insert into drafts (id, tenant_id, reminder_id, attempt, source, model, text, problems, warnings, grade,
                             created_at)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        id.as_uuid(),
        clinic.as_uuid(),
        reminder.as_uuid(),
        attempt,
        draft.source,
        draft.model,
        draft.text,
        serde_json::to_value(&draft.checked.problems).map_err(encode)?,
        serde_json::to_value(&draft.checked.warnings).map_err(encode)?,
        draft.checked.grade,
        now,
    )
    .execute(&mut *tx)
    .await?;
    for (action, details) in [
        (
            "reminder.drafted",
            json!({ "draft": id, "attempt": attempt, "source": draft.source, "model": draft.model }),
        ),
        (
            "reminder.checked",
            json!({
                "draft": id, "blocked": !draft.checked.problems.is_empty(), "problems": draft.checked.problems,
                "warnings": draft.checked.warnings, "grade": draft.checked.grade,
            }),
        ),
    ] {
        let event = AuditEvent {
            tenant: clinic,
            at: now,
            actor: if action == "reminder.checked" { "system" } else { actor },
            action,
            subject_type: "reminder",
            subject_id: reminder.as_uuid(),
            details,
        };
        audit::record(&mut *tx, &event).await?;
    }
    Ok(id)
}

impl<M: LanguageModel> Drafter<M> {
    /// Drafts with `model`, or the template when `None`.
    pub fn new(pool: PgPool, clock: Arc<dyn Clock>, model: Option<M>) -> Self {
        Self { pool, clock, model }
    }

    /// Drafts every reminder that is due (E6): a follow-up's first on the day
    /// it is chosen, its second and third on days 7 and 21, each only once
    /// the one before is finished, and none once the follow-up has stopped.
    /// A system-wide worker: it reads across clinics, and every write names
    /// the row's clinic. Returns how many it drafted.
    pub async fn draft_due(&self) -> Result<usize, sqlx::Error> {
        let today = self.clock.now().date();
        let due = sqlx::query!(
            r#"select f.tenant_id, f.id, f.kind, f.selected_at, coalesce(s.tone, 'warm') as "tone!",
                      last.number as "last_number?", last.finished_at as "last_finished?"
               from follow_ups f
               left join clinic_settings s on s.tenant_id = f.tenant_id
               left join lateral (
                   select number, finished_at from reminders r
                   where r.tenant_id = f.tenant_id and r.follow_up_id = f.id
                   order by number desc limit 1
               ) last on true
               where f.stopped_at is null and coalesce(last.number, 0) < 3
               order by f.selected_at, f.id"#
        )
        .fetch_all(&self.pool)
        .await?;
        let mut drafted = 0;
        for follow_up in due {
            let number = follow_up.last_number.map_or(1, |last| last + 1);
            let previous_finished = follow_up.last_number.is_none() || follow_up.last_finished.is_some();
            let Some(days) = usize::try_from(number - 1).ok().and_then(|i| PLANNED_DAYS.get(i)) else {
                continue;
            };
            // Planned from when the follow-up was chosen, but never sooner than
            // the same gap after the reminder before was finished, so a late
            // reminder doesn't bring the next one forward.
            let mut planned_on = follow_up.selected_at.date() + Duration::days(*days);
            if let (Some(finished), Some(before)) = (
                follow_up.last_finished,
                usize::try_from(number - 2).ok().and_then(|i| PLANNED_DAYS.get(i)),
            ) {
                planned_on = planned_on.max(finished.date() + Duration::days(days - before));
            }
            if !previous_finished || planned_on > today {
                continue;
            }
            let clinic = ClinicId::from(follow_up.tenant_id);
            let kind = Kind::from_code(&follow_up.kind).unwrap_or(Kind::AfterResult);
            let tone = if follow_up.tone == "plain" {
                Tone::Plain
            } else {
                Tone::Warm
            };
            // One follow-up's failure doesn't stop the others; it is tried
            // again on the next pass.
            let result = match self.write(clinic, kind, number as u8, tone).await {
                Ok(draft) => self.store(clinic, follow_up.id, number, planned_on, &draft).await,
                Err(err) => Err(err),
            };
            match result {
                Ok(true) => drafted += 1,
                Ok(false) => {}
                Err(err) => eprintln!("care-reminders: drafting a reminder for {} failed: {err}", follow_up.id),
            }
        }
        Ok(drafted)
    }

    /// Reminder `number`'s draft, from the model or the template, checked.
    pub(crate) async fn write(
        &self,
        clinic: ClinicId,
        kind: Kind,
        number: u8,
        tone: Tone,
    ) -> Result<NewDraft, sqlx::Error> {
        let Some(model) = &self.model else {
            let text = template(number);
            let checked = check(&text);
            return Ok(NewDraft {
                source: "template",
                model: None,
                text,
                checked,
            });
        };
        let reply = model.complete(&request(kind, number, tone)).await;
        let parsed = reply
            .as_ref()
            .map_err(LlmError::to_string)
            .and_then(|reply| parse_answer(&reply.text).map_err(|err| err.to_string()));
        let (usage, request_id) = match &reply {
            Ok(reply) => (
                Usage {
                    input_tokens: reply.input_tokens,
                    output_tokens: reply.output_tokens,
                },
                reply.request_id.clone(),
            ),
            Err(LlmError::Incomplete { usage, request_id, .. }) => (*usage, request_id.clone()),
            Err(_) => (Usage::default(), None),
        };
        let outcome = match (&reply, &parsed) {
            (Err(_), _) => Outcome::Failed,
            (Ok(_), Err(_)) => Outcome::Invalid,
            (Ok(_), Ok(_)) => Outcome::Ok,
        };
        let call = LlmCall {
            tenant: clinic,
            at: self.clock.now(),
            model: model.model().to_owned(),
            usage,
            request_id,
            outcome,
        };
        cost_log::record(&self.pool, &call).await?;
        let answered_by = reply.as_ref().map(|reply| reply.model.clone()).ok();
        Ok(match parsed {
            // No answer at all: the clinic's template stands in, saying so.
            Err(_) if reply.is_err() => {
                let text = template(number);
                let mut checked = check(&text);
                let reason = reply.as_ref().err().map(ToString::to_string).unwrap_or_default();
                checked.warnings.insert(
                    0,
                    format!("The model didn't answer ({reason}), so this is the clinic's template."),
                );
                NewDraft {
                    source: "template",
                    model: None,
                    text,
                    checked,
                }
            }
            Ok(text) => {
                let checked = check(&text);
                NewDraft {
                    source: "model",
                    model: answered_by,
                    text,
                    checked,
                }
            }
            // An answer that can't be read blocks the draft (C1); the
            // coordinator can use the template or ask again.
            Err(reason) => NewDraft {
                source: "model",
                model: answered_by.or_else(|| Some(model.model().to_owned())),
                text: String::new(),
                checked: Checked {
                    problems: vec![Problem {
                        check: Check::Answer,
                        detail: reason,
                    }],
                    warnings: Vec::new(),
                    grade: 0.0,
                },
            },
        })
    }

    async fn store(
        &self,
        clinic: ClinicId,
        follow_up: uuid::Uuid,
        number: i16,
        planned_on: time::Date,
        draft: &NewDraft,
    ) -> Result<bool, sqlx::Error> {
        let now = self.clock.now();
        let reminder = ReminderId::new();
        let mut tx = self.pool.begin().await?;
        let added = sqlx::query!(
            "insert into reminders (id, tenant_id, follow_up_id, number, planned_on, created_at)
             values ($1, $2, $3, $4, $5, $6) on conflict (tenant_id, follow_up_id, number) do nothing",
            reminder.as_uuid(),
            clinic.as_uuid(),
            follow_up,
            number,
            planned_on,
            now,
        )
        .execute(&mut *tx)
        .await?;
        if added.rows_affected() == 0 {
            // Another worker drafted it first.
            return Ok(false);
        }
        add_draft(
            &mut tx,
            clinic,
            reminder,
            1,
            draft,
            now,
            if draft.source == "model" { "model" } else { "system" },
        )
        .await?;
        tx.commit().await?;
        Ok(true)
    }
}
