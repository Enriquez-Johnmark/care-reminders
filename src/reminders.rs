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
struct NewDraft {
    source: &'static str,
    model: Option<String>,
    text: String,
    checked: Checked,
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
               where f.stopped_at is null
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
            let planned_on = follow_up.selected_at.date() + Duration::days(*days);
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
            let draft = self.write(clinic, kind, number as u8, tone).await?;
            self.store(clinic, follow_up.id, number, planned_on, &draft).await?;
            drafted += 1;
        }
        Ok(drafted)
    }

    /// Reminder `number`'s draft, from the model or the template, checked.
    async fn write(&self, clinic: ClinicId, kind: Kind, number: u8, tone: Tone) -> Result<NewDraft, sqlx::Error> {
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
            Ok(text) => {
                let checked = check(&text);
                NewDraft {
                    source: "model",
                    model: answered_by,
                    text,
                    checked,
                }
            }
            // An answer that can't be read, or none, blocks the draft (C1);
            // the coordinator can use the template or ask again.
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
    ) -> Result<(), sqlx::Error> {
        let now = self.clock.now();
        let (reminder, draft_id) = (ReminderId::new(), DraftId::new());
        let encode = |err: serde_json::Error| sqlx::Error::Encode(err.into());
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
            return Ok(());
        }
        sqlx::query!(
            "insert into drafts (id, tenant_id, reminder_id, attempt, source, model, text, problems, warnings, grade,
                                 created_at)
             values ($1, $2, $3, 1, $4, $5, $6, $7, $8, $9, $10)",
            draft_id.as_uuid(),
            clinic.as_uuid(),
            reminder.as_uuid(),
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
        let event = AuditEvent {
            tenant: clinic,
            at: now,
            actor: if draft.source == "model" { "model" } else { "system" },
            action: "reminder.drafted",
            subject_type: "reminder",
            subject_id: reminder.as_uuid(),
            details: json!({
                "follow_up": follow_up, "number": number, "source": draft.source, "model": draft.model,
                "blocked": !draft.checked.problems.is_empty(), "problems": draft.checked.problems,
                "grade": draft.checked.grade,
            }),
        };
        audit::record(&mut *tx, &event).await?;
        tx.commit().await
    }
}
