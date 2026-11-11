//! The JSON API under `/api/` that the React app calls.
//!
//! - Every endpoint but logging in takes an [`ApiMember`] from the session
//!   cookie, and passes their clinic to the repositories, so another
//!   clinic's IDs are "not found" (A4). The rules (who may approve, what
//!   blocks a draft) are in `review` and `checks`; this layer only
//!   translates.
//! - Errors are JSON ([`ErrorBody`]) without internal details.

use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::extract::{FromRequest, FromRequestParts, Query, Request, State};
use axum::http::request::Parts;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;

use super::{AppState, SESSION_COOKIE, session_cookie};
use crate::activity::{Activity, Entry};
use crate::drafting::{FillIns, fill, template};
use crate::engine::accounts::{Accounts, LoginLink, LoginOutcome, Role, SESSION_LIFETIME, Session};
use crate::engine::mail::Email;
use crate::engine::outbox::NoExtraChecks;
use crate::fhir_server::{FetchError, FetchOutcome, FhirServers};
use crate::follow_ups::FollowUpId;
use crate::model::AnyModel;
use crate::reminders::{DraftId, ReminderId};
use crate::review::{MAX_NEW_DRAFTS, ReviewError, content_hash};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/api/login", post(request_link))
        .route("/api/login/{token}", post(use_link))
        .route("/api/logout", post(log_out))
        .route("/api/me", get(me))
        .route("/api/queue", get(queue))
        .route("/api/reminders/{id}", get(reminder))
        .route("/api/reminders/{id}/approve", post(approve))
        .route("/api/reminders/{id}/edit", post(edit))
        .route("/api/reminders/{id}/template", post(use_template))
        .route("/api/reminders/{id}/redraft", post(redraft))
        .route("/api/reminders/{id}/skip", post(skip))
        .route("/api/follow-ups/{id}/timeline", get(timeline))
        .route("/api/replies", get(replies))
        .route("/api/activity", get(activity))
        .route("/api/fhir-server/fetch", post(fetch_from_fhir_server))
        .route("/api/{*rest}", any(not_found))
}

async fn not_found() -> ApiError {
    ApiError::NotFound
}

/// What every API error answers.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ErrorBody {
    /// One sentence for the person.
    pub message: String,
    /// Each problem found; may be empty.
    pub problems: Vec<String>,
}

/// An API failure.
#[derive(Debug)]
pub enum ApiError {
    /// Missing, or another clinic's (A4).
    NotFound,
    /// No live session.
    NotLoggedIn,
    /// Understood but refused, with the reason.
    Refused(StatusCode, String),
    /// Logged; the person sees a generic error.
    Internal,
}

impl ApiError {
    /// Logs `err` and answers with a generic error.
    fn internal(err: impl std::fmt::Display) -> Self {
        tracing::error!(%err, "API request failed");
        Self::Internal
    }

    fn administrator_only() -> Self {
        Self::Refused(
            StatusCode::FORBIDDEN,
            "Only the clinic's administrator can do this.".into(),
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "Not found.".to_owned()),
            Self::NotLoggedIn => (StatusCode::UNAUTHORIZED, "Log in first.".to_owned()),
            Self::Refused(status, message) => (status, message),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Something went wrong. Please try again.".to_owned(),
            ),
        };
        (
            status,
            Json(ErrorBody {
                message,
                problems: Vec::new(),
            }),
        )
            .into_response()
    }
}

impl From<ReviewError> for ApiError {
    fn from(err: ReviewError) -> Self {
        let conflict = |message: &str| Self::Refused(StatusCode::CONFLICT, message.to_owned());
        match err {
            ReviewError::NotFound => Self::NotFound,
            ReviewError::NotACoordinator => {
                Self::Refused(StatusCode::FORBIDDEN, "Only a care coordinator can do this.".into())
            }
            ReviewError::Blocked => {
                conflict("This draft failed a check, so it can't be approved. Edit it or use the template.")
            }
            ReviewError::Changed => {
                conflict("The reminder changed since you opened it. Read the latest text before approving.")
            }
            ReviewError::Finished => conflict("This reminder was already sent or skipped."),
            ReviewError::NoMoreDrafts => conflict("No more new drafts for this reminder. Edit it or use the template."),
            ReviewError::NoReason => {
                Self::Refused(StatusCode::UNPROCESSABLE_ENTITY, "Say why you're skipping it.".into())
            }
            ReviewError::Stopped => conflict("This follow-up has stopped: the patient booked, replied or opted out."),
            ReviewError::NoModel => {
                conflict("No model is set up for new drafts. Edit the reminder or use the template.")
            }
            ReviewError::CantFill(reason) => conflict(&format!("This reminder can't be sent as it stands: {reason}.")),
            other => Self::internal(other),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(err: sqlx::Error) -> Self {
        Self::internal(err)
    }
}

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        Self::Refused(rejection.status(), "That wasn't understood; try again.".into())
    }
}

impl From<PathRejection> for ApiError {
    /// A malformed ID in the address names nothing: "not found" (A4).
    fn from(_: PathRejection) -> Self {
        Self::NotFound
    }
}

/// A JSON body; one that can't be read is refused as an [`ErrorBody`].
struct ApiJson<T>(T);

impl<T: DeserializeOwned, S: Send + Sync> FromRequest<S> for ApiJson<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state).await?;
        Ok(Self(value))
    }
}

/// An ID from the path; a malformed one is "not found".
struct IdPath(uuid::Uuid);

impl<S: Send + Sync> FromRequestParts<S> for IdPath {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let axum::extract::Path(id) = axum::extract::Path::<uuid::Uuid>::from_request_parts(parts, state).await?;
        Ok(Self(id))
    }
}

/// The logged-in person; 401 without a live session (H1).
struct ApiMember(Session);

impl FromRequestParts<AppState> for ApiMember {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let session = session_cookie(parts).ok_or(ApiError::NotLoggedIn)?;
        match state.logins.session(&session).await {
            Ok(Some(member)) => Ok(Self(member)),
            Ok(None) => Err(ApiError::NotLoggedIn),
            Err(err) => Err(ApiError::internal(err)),
        }
    }
}

fn time_text(at: time::OffsetDateTime) -> String {
    at.format(&Rfc3339).unwrap_or_default()
}

/// Asking for a login link.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct LoginRequest {
    /// The address to send it to.
    pub email: String,
}

/// The login email: a link to the app's login page, which uses it only when
/// the person presses "Log in", so email link scanners can't use it up.
fn login_email(base_url: &str, link: &LoginLink<'_>) -> Email {
    Email {
        to_address: link.to_address.to_owned(),
        from_name: link.account_name.to_owned(),
        reply_to: None,
        subject: "Your login link".to_owned(),
        body_text: format!(
            "Hi,\n\nOpen this link to log in to {}:\n\n{base_url}/login/{}\n\nIt works once, for the next 15 \
             minutes. If you didn't ask for it, ignore this email.\n",
            link.account_name, link.token
        ),
    }
}

async fn request_link(State(state): State<AppState>, ApiJson(request): ApiJson<LoginRequest>) -> StatusCode {
    // The lookup and the email happen after the answer, so neither the
    // answer nor how long it takes reveals who has an account.
    tokio::spawn(async move {
        let base_url = state.base_url.clone();
        let compose = |link: &LoginLink<'_>| login_email(&base_url, link);
        match state.logins.send_link(&request.email, compose, &NoExtraChecks).await {
            Ok(LoginOutcome::Sent) => state.wake_sender.notify_one(),
            Ok(LoginOutcome::NotSent) => {}
            Err(err) => tracing::error!(%err, "sending a login link failed"),
        }
    });
    StatusCode::ACCEPTED
}

async fn use_link(
    State(state): State<AppState>,
    axum::extract::Path(token): axum::extract::Path<String>,
) -> Result<Response, ApiError> {
    let Some(session) = state.logins.log_in(&token).await.map_err(ApiError::internal)? else {
        return Err(ApiError::Refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "This login link has expired or was already used. Ask for a new one.".into(),
        ));
    };
    let secure = if state.secure_cookies { "; Secure" } else { "" };
    let cookie = format!(
        "{SESSION_COOKIE}={session}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax{secure}",
        SESSION_LIFETIME.whole_seconds()
    );
    Ok(([(header::SET_COOKIE, cookie)], Json(serde_json::json!({}))).into_response())
}

async fn log_out(State(state): State<AppState>, request: Request) -> Result<Response, ApiError> {
    let (parts, _) = request.into_parts();
    if let Some(session) = session_cookie(&parts) {
        state.logins.log_out(&session).await.map_err(ApiError::internal)?;
    }
    let cookie = format!("{SESSION_COOKIE}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax");
    Ok(([(header::SET_COOKIE, cookie)], StatusCode::NO_CONTENT).into_response())
}

/// Who is logged in.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Me {
    /// The clinic's name.
    pub clinic_name: String,
    /// Their address.
    pub email: String,
    /// `coordinator`, `administrator` or `staff`.
    pub role: &'static str,
    /// A coordinator's name.
    pub name: Option<String>,
    /// The clinic's FHIR server, if the operator has set one.
    pub fhir_server: Option<String>,
}

async fn me(State(state): State<AppState>, ApiMember(member): ApiMember) -> Result<Json<Me>, ApiError> {
    let account = Accounts::new(state.pool.clone(), state.clock.clone())
        .get(member.tenant)
        .await
        .map_err(ApiError::internal)?;
    let name = state.reviews.coordinator_name(&member).await?;
    let role = match (&name, member.role) {
        (Some(_), _) => "coordinator",
        (None, Role::Owner) => "administrator",
        (None, Role::Staff) => "staff",
    };
    let fhir_server = FhirServers::new(state.pool.clone(), state.clock.clone())
        .get(member.tenant)
        .await?;
    Ok(Json(Me {
        clinic_name: account.name,
        email: member.email,
        role,
        name,
        fhir_server: fhir_server.map(|address| address.to_string()),
    }))
}

/// One reminder waiting for the coordinator.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct QueueRow {
    /// The reminder.
    pub id: String,
    /// Its follow-up.
    pub follow_up: String,
    /// The patient's name.
    pub patient_name: String,
    /// `after-result`, `screening-recall` or `after-discharge`.
    pub kind: String,
    /// Why, for staff.
    pub reason: String,
    /// When the care is due by.
    pub due_by: String,
    /// Which reminder, 1 to 3.
    pub number: i16,
    /// `waiting` (passed its checks), `blocked` or `approved`.
    pub status: &'static str,
    /// The patient asked not to be emailed: contact them another way (E5).
    pub contact_another_way: bool,
}

/// The reminders waiting for the coordinator: drafts that passed every
/// check first, then blocked ones, then approved ones; soonest due first.
async fn queue(State(state): State<AppState>, ApiMember(member): ApiMember) -> Result<Json<Vec<QueueRow>>, ApiError> {
    let rows = sqlx::query!(
        r#"select distinct on (r.id) r.id, r.number, r.follow_up_id, f.kind, f.reason, f.due_by, p.name,
                  p.no_email, jsonb_array_length(d.problems) > 0 as "blocked!",
                  exists (select 1 from approvals a where a.tenant_id = r.tenant_id and a.draft_id = d.id) as "approved!"
           from reminders r
           join follow_ups f on f.tenant_id = r.tenant_id and f.id = r.follow_up_id
           join patients p on p.tenant_id = f.tenant_id and p.id = f.patient_id
           join drafts d on d.tenant_id = r.tenant_id and d.reminder_id = r.id
           where r.tenant_id = $1 and r.finished_at is null and f.stopped_at is null
           order by r.id, d.attempt desc"#,
        member.tenant.as_uuid()
    )
    .fetch_all(&state.pool)
    .await?;
    let mut queue: Vec<QueueRow> = rows
        .into_iter()
        .map(|row| QueueRow {
            id: row.id.to_string(),
            follow_up: row.follow_up_id.to_string(),
            patient_name: row.name,
            kind: row.kind,
            reason: row.reason,
            due_by: row.due_by.to_string(),
            number: row.number,
            status: match (row.approved, row.blocked) {
                (true, _) => "approved",
                (false, true) => "blocked",
                (false, false) => "waiting",
            },
            contact_another_way: row.no_email,
        })
        .collect();
    let order = |status: &str| ["waiting", "blocked", "approved"].iter().position(|s| *s == status);
    queue.sort_by(|a, b| {
        (order(a.status), &a.due_by, &a.patient_name).cmp(&(order(b.status), &b.due_by, &b.patient_name))
    });
    Ok(Json(queue))
}

/// One problem a check found.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ProblemView {
    /// The check, e.g. `clinical-words`.
    pub check: String,
    /// What it found.
    pub detail: String,
}

/// A reminder's latest draft.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct DraftView {
    /// Its ID, which an approval names.
    pub id: String,
    /// `model`, `template` or `coordinator`.
    pub source: String,
    /// Its text, with its fill-ins.
    pub text: String,
    /// The fingerprint of the exact text, which an approval names (D3).
    pub content_hash: String,
    /// What the checks found; any one blocks it.
    pub problems: Vec<ProblemView>,
    /// Warnings, which don't block it.
    pub warnings: Vec<String>,
    /// Its reading grade.
    pub grade: f64,
}

/// One reminder, as the review page shows it.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ReminderView {
    /// The reminder.
    pub id: String,
    /// Its follow-up.
    pub follow_up: String,
    /// The patient's name.
    pub patient_name: String,
    /// The kind of follow-up.
    pub kind: String,
    /// Why, for staff.
    pub reason: String,
    /// The rule that chose the patient, and its version.
    pub rule: String,
    /// The version of that rule.
    pub rule_version: i32,
    /// When the care is due by.
    pub due_by: String,
    /// Which reminder, 1 to 3.
    pub number: i16,
    /// The latest draft.
    pub draft: DraftView,
    /// The draft as the patient would read it, its fill-ins filled; `None`
    /// when something is missing, such as the clinic's phone.
    pub preview: Option<String>,
    /// Why there is no preview.
    pub preview_problem: Option<String>,
    /// The clinic's fixed template for this reminder, to compare.
    pub template: String,
    /// When the latest draft was approved, if it was.
    pub approved_at: Option<String>,
    /// How many more new drafts the coordinator may ask for (D5).
    pub new_drafts_left: i64,
    /// The patient asked not to be emailed (E5).
    pub contact_another_way: bool,
    /// `sent`, `skipped` or `contact-another-way` once finished.
    pub finished: Option<String>,
    /// Its email, once approved.
    pub delivery: Option<DeliveryView>,
}

/// Where a reminder's email stands.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct DeliveryView {
    /// `queued`, `sent`, `withdrawn` or `not_sent`.
    pub status: &'static str,
    /// Why it waits or wasn't sent.
    pub detail: Option<String>,
    /// When it went.
    pub sent_at: Option<String>,
}

async fn reminder(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    IdPath(id): IdPath,
) -> Result<Json<ReminderView>, ApiError> {
    let clinic = member.tenant;
    let row = sqlx::query!(
        r#"select r.id, r.number, r.follow_up_id, r.outcome, f.kind, f.reason, f.due_by, f.rule_version,
                  ru.name as rule, p.name as patient_name, p.given_name, p.no_email,
                  d.id as draft, d.source, d.text, d.problems, d.warnings, d.grade,
                  (select count(*) from drafts m where m.tenant_id = r.tenant_id and m.reminder_id = r.id
                     and m.source = 'model' and m.attempt > 1) as "new_drafts!"
           from reminders r
           join follow_ups f on f.tenant_id = r.tenant_id and f.id = r.follow_up_id
           join rules ru on ru.tenant_id = f.tenant_id and ru.id = f.rule_id
           join patients p on p.tenant_id = f.tenant_id and p.id = f.patient_id
           join drafts d on d.tenant_id = r.tenant_id and d.reminder_id = r.id
           where r.tenant_id = $1 and r.id = $2
           order by d.attempt desc limit 1"#,
        clinic.as_uuid(),
        id,
    )
    .fetch_optional(&state.pool)
    .await?
    .ok_or(ApiError::NotFound)?;
    let problems: Vec<crate::checks::Problem> = serde_json::from_value(row.problems).map_err(ApiError::internal)?;
    let warnings: Vec<String> = serde_json::from_value(row.warnings).map_err(ApiError::internal)?;
    let account = Accounts::new(state.pool.clone(), state.clock.clone())
        .get(clinic)
        .await
        .map_err(ApiError::internal)?;
    let settings = crate::settings::get(&state.pool, clinic).await?;
    let values = FillIns {
        first_name: row.given_name,
        clinic_name: account.name,
        clinic_phone: settings.phone,
        due_by: row.due_by,
        today: crate::sending_hours::local_day(state.clock.now(), &settings.time_zone).map_err(ApiError::internal)?,
        opt_out_link: "(the link to stop these reminders)".to_owned(),
    };
    let (preview, preview_problem) = match fill(&row.text, &values) {
        Ok(filled) => (Some(filled), None),
        Err(err) => (None, Some(err.to_string())),
    };
    let approval = state.reviews.current_approval(clinic, ReminderId::from(row.id)).await?;
    let delivery = crate::sending::delivery(&state.pool, clinic, ReminderId::from(row.id))
        .await?
        .map(|delivery| DeliveryView {
            status: delivery.status,
            detail: delivery.detail,
            sent_at: delivery.sent_at.map(time_text),
        });
    Ok(Json(ReminderView {
        id: row.id.to_string(),
        follow_up: row.follow_up_id.to_string(),
        patient_name: row.patient_name,
        kind: row.kind,
        reason: row.reason,
        rule: row.rule,
        rule_version: row.rule_version,
        due_by: row.due_by.to_string(),
        number: row.number,
        draft: DraftView {
            id: row.draft.to_string(),
            source: row.source,
            content_hash: content_hash(&row.text),
            text: row.text,
            problems: problems
                .into_iter()
                .map(|p| ProblemView {
                    check: serde_json::to_value(p.check)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    detail: p.detail,
                })
                .collect(),
            warnings,
            grade: row.grade,
        },
        preview,
        preview_problem,
        template: template(row.number as u8),
        approved_at: approval.map(|a| time_text(a.approved_at)),
        // Without a model there are no new drafts to ask for.
        new_drafts_left: if state.model.is_some() {
            (MAX_NEW_DRAFTS - row.new_drafts).max(0)
        } else {
            0
        },
        contact_another_way: row.no_email,
        finished: row.outcome,
        delivery,
    }))
}

/// Approving the text the coordinator read.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ApproveRequest {
    /// The draft read.
    pub draft_id: String,
    /// The fingerprint of the exact text read (D3).
    pub content_hash: String,
}

async fn approve(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    IdPath(id): IdPath,
    ApiJson(request): ApiJson<ApproveRequest>,
) -> Result<StatusCode, ApiError> {
    let draft: uuid::Uuid = request.draft_id.parse().map_err(|_| ApiError::NotFound)?;
    state
        .reviews
        .approve(
            &member,
            ReminderId::from(id),
            DraftId::from(draft),
            &request.content_hash,
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The coordinator's own text.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct EditRequest {
    /// The text, with its fill-ins.
    pub text: String,
}

/// What an edit, the template or a new draft gave.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct EditedView {
    /// Whether a check blocked it.
    pub blocked: bool,
    /// What the checks found.
    pub problems: Vec<ProblemView>,
}

fn edited_view(edited: crate::review::Edited) -> EditedView {
    EditedView {
        blocked: edited.blocked,
        problems: edited
            .problems
            .into_iter()
            .map(|p| ProblemView {
                check: serde_json::to_value(p.check)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_owned))
                    .unwrap_or_default(),
                detail: p.detail,
            })
            .collect(),
    }
}

async fn edit(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    IdPath(id): IdPath,
    ApiJson(request): ApiJson<EditRequest>,
) -> Result<Json<EditedView>, ApiError> {
    let edited = state.reviews.edit(&member, ReminderId::from(id), &request.text).await?;
    Ok(Json(edited_view(edited)))
}

async fn use_template(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    IdPath(id): IdPath,
) -> Result<Json<EditedView>, ApiError> {
    let edited = state.reviews.use_template(&member, ReminderId::from(id)).await?;
    Ok(Json(edited_view(edited)))
}

async fn redraft(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    IdPath(id): IdPath,
) -> Result<Json<EditedView>, ApiError> {
    let model: Option<&AnyModel> = state.model.as_deref();
    let edited = state.reviews.new_draft(&member, ReminderId::from(id), model).await?;
    Ok(Json(edited_view(edited)))
}

/// Skipping a reminder.
#[derive(Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SkipRequest {
    /// Why (D6).
    pub reason: String,
}

async fn skip(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    IdPath(id): IdPath,
    ApiJson(request): ApiJson<SkipRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .reviews
        .skip(&member, ReminderId::from(id), &request.reason)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// One activity entry.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct EntryView {
    /// When.
    pub at: String,
    /// Who.
    pub actor: String,
    /// What, e.g. `reminder.approved`.
    pub action: String,
    /// What it is about: `follow_up`, `reminder`, `rule` or `fhir_server`.
    pub subject_type: String,
    /// Which one.
    pub subject_id: String,
    /// Extra facts.
    #[cfg_attr(test, ts(type = "Record<string, unknown>"))]
    pub details: serde_json::Value,
}

fn entry_view(entry: Entry) -> EntryView {
    EntryView {
        at: time_text(entry.at),
        actor: entry.actor,
        action: entry.action,
        subject_type: entry.subject_type,
        subject_id: entry.subject_id.to_string(),
        details: entry.details,
    }
}

async fn timeline(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    IdPath(id): IdPath,
) -> Result<Json<Vec<EntryView>>, ApiError> {
    let entries = Activity::new(state.pool.clone())
        .timeline(member.tenant, FollowUpId::from(id))
        .await?;
    if entries.is_empty() {
        return Err(ApiError::NotFound);
    }
    Ok(Json(entries.into_iter().map(entry_view).collect()))
}

/// Filters for the clinic-wide log.
#[derive(Debug, Deserialize)]
pub struct ActivityQuery {
    actor: Option<String>,
    action: Option<String>,
}

/// The clinic-wide activity, for the administrator (G3).
async fn activity(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
    Query(query): Query<ActivityQuery>,
) -> Result<Json<Vec<EntryView>>, ApiError> {
    if member.role != Role::Owner {
        return Err(ApiError::administrator_only());
    }
    let blank = |value: Option<String>| value.filter(|v| !v.trim().is_empty());
    let (actor, action) = (blank(query.actor), blank(query.action));
    let entries = Activity::new(state.pool.clone())
        .clinic_log(member.tenant, actor.as_deref(), action.as_deref())
        .await?;
    Ok(Json(entries.into_iter().map(entry_view).collect()))
}

/// What a fetch did, for the administrator.
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct FetchView {
    /// The server.
    pub server: String,
    /// Patients not seen before.
    pub new: usize,
    /// Patients whose record changed.
    pub updated: usize,
    /// Patients the reader refused, each with the reason.
    pub refused: Vec<String>,
    /// Follow-ups the rules chose.
    pub follow_ups_chosen: usize,
    /// Follow-ups stopped because the patient booked.
    pub follow_ups_booked: usize,
}

/// "Fetch from the FHIR server", for the administrator (H2).
async fn fetch_from_fhir_server(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
) -> Result<Json<FetchView>, ApiError> {
    if member.role != Role::Owner {
        return Err(ApiError::administrator_only());
    }
    let outcome: FetchOutcome = FhirServers::new(state.pool.clone(), state.clock.clone())
        .fetch(member.tenant, &member.email)
        .await
        .map_err(|err| match err {
            FetchError::NoServer => ApiError::Refused(StatusCode::CONFLICT, err.to_string()),
            FetchError::Database(err) => ApiError::internal(err),
            other => ApiError::Refused(StatusCode::BAD_GATEWAY, other.to_string()),
        })?;
    Ok(Json(FetchView {
        server: outcome.server,
        new: outcome.new,
        updated: outcome.updated,
        refused: outcome
            .refused
            .into_iter()
            .map(|r| format!("{}: {}", r.patient, r.reason))
            .collect(),
        follow_ups_chosen: outcome.follow_ups_chosen,
        follow_ups_booked: outcome.follow_ups_booked,
    }))
}

/// One patient's reply (F2).
#[derive(Debug, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ReplyView {
    /// The reply.
    pub id: String,
    /// Its follow-up, whose reminders it stopped.
    pub follow_up: String,
    /// The patient.
    pub patient_name: String,
    /// The address it came from.
    pub from_address: String,
    /// What they wrote: untrusted text, shown as text only.
    pub text: String,
    /// When it arrived.
    pub received_at: String,
}

/// The clinic's replies, newest first.
async fn replies(
    State(state): State<AppState>,
    ApiMember(member): ApiMember,
) -> Result<Json<Vec<ReplyView>>, ApiError> {
    let replies = crate::sending::replies(&state.pool, member.tenant).await?;
    Ok(Json(
        replies
            .into_iter()
            .map(|reply| ReplyView {
                id: reply.id.to_string(),
                follow_up: reply.follow_up.to_string(),
                patient_name: reply.patient_name,
                from_address: reply.from_address,
                text: reply.text,
                received_at: time_text(reply.received_at),
            })
            .collect(),
    ))
}
