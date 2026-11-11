//! Machine endpoints under `/hooks/`, called by the email provider rather
//! than a browser. Each request proves where it came from with a signature
//! over its body, so the same-origin check for forms doesn't apply.
//!
//! - `POST /hooks/inbound-email`: one raw inbound email (F2), signed with
//!   `X-Care-Reminders-Signature: <hex HMAC-SHA256 of the body>` under
//!   `INBOUND_SECRET`. Without that setting the endpoint doesn't exist.

use axum::Router;
use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;

use super::AppState;
use crate::engine::inbound::{self, InboundError, MAX_RAW_BYTES};

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/hooks/inbound-email", post(inbound_email))
        .layer(DefaultBodyLimit::max(MAX_RAW_BYTES))
}

async fn inbound_email(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(secret) = &state.inbound_secret else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let signature = headers
        .get("x-care-reminders-signature")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    if !inbound::signed(secret.as_bytes(), &body, signature) {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let email = match inbound::parse(&body) {
        Ok(email) => email,
        Err(InboundError::TooLarge) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        Err(InboundError::Unreadable) => return StatusCode::BAD_REQUEST.into_response(),
    };
    match crate::sending::receive(&state.pool, state.clock.clone(), &state.sending, &email).await {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => {
            tracing::error!(%err, "taking in an inbound email failed");
            // The provider retries a failed delivery.
            StatusCode::SERVICE_UNAVAILABLE.into_response()
        }
    }
}
