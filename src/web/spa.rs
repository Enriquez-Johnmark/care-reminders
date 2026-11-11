//! Serves the built React app (`web/dist`): its files under `/assets/`,
//! cached for good because their names change with their content, and its
//! page for every other address the app shows, never cached, so a new
//! deploy is picked up at once.

use std::path::Path;

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use tower_http::services::{ServeDir, ServeFile};

use super::AppState;

/// Every GET that no other route answers.
pub(super) async fn app(State(state): State<AppState>, request: Request) -> Response {
    if !matches!(*request.method(), Method::GET | Method::HEAD) {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let Some(dir) = state.web_dir.as_deref() else {
        return not_built();
    };
    let index = dir.join("index.html");
    if !index.is_file() {
        return not_built();
    }
    let assets = request.uri().path().starts_with("/assets/");
    let served = if assets {
        ServeDir::new(dir).try_call(request).await
    } else {
        ServeDir::new(dir)
            .fallback(ServeFile::new(&index))
            .try_call(request)
            .await
    };
    let mut response = match served {
        Ok(response) => response.map(Body::new),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return StatusCode::NOT_FOUND.into_response();
        }
        Err(err) => {
            tracing::error!(%err, "serving the React app failed");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };
    let cache = if assets && response.status().is_success() {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    response
}

fn not_built() -> Response {
    (
        StatusCode::NOT_FOUND,
        "The app isn't built here. Run `pnpm --dir web build`, or open the Vite server in development.",
    )
        .into_response()
}

/// Whether `dir` holds a built app, for a warning at start-up.
pub fn is_built(dir: &Path) -> bool {
    dir.join("index.html").is_file()
}
