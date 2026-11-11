//! The opt-out page (F3, F4), server-rendered from a reminder's signed link,
//! with no login and no JavaScript, in the clinic's letter style.
//!
//! Opening the link only shows a button; pressing it opts the patient out of
//! that kind of reminder. Email scanners and link previews, which only fetch
//! links, change nothing. The page names the clinic and nothing about the
//! patient's care.

use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;

use super::AppState;
use crate::html::escape;
use crate::sending::{OptOutLink, opt_out, opt_out_link};

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/o/{token}", get(gate).post(confirm))
}

async fn gate(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    match opt_out_link(&state.pool, &state.sending, &token).await {
        Ok(Some(link)) if link.done => done_page(&link),
        Ok(Some(link)) => {
            let body = format!(
                "<header class=\"letterhead\"><p class=\"clinic\">{clinic}</p></header>\n\
                 <h1>Stop these reminders?</h1>\n\
                 <p>{clinic} sends reminders to book care that is due. You can stop these reminders here. \
                 Your care at {clinic} doesn't change, and you can still call the clinic at any time.</p>\n\
                 <form method=\"post\"><button type=\"submit\">Stop these reminders</button></form>",
                clinic = escape(&link.clinic_name)
            );
            document("Stop these reminders", &body).into_response()
        }
        Ok(None) => not_found(),
        Err(err) => failed(&err.to_string()),
    }
}

async fn confirm(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    match opt_out(&state.pool, state.clock.clone(), &state.sending, &token).await {
        Ok(Some(link)) => done_page(&link),
        Ok(None) => not_found(),
        Err(err) => failed(&err.to_string()),
    }
}

fn done_page(link: &OptOutLink) -> Response {
    let body = format!(
        "<header class=\"letterhead\"><p class=\"clinic\">{clinic}</p></header>\n\
         <h1>These reminders are stopped</h1>\n\
         <p>You won't get these reminders from {clinic} again. If you change your mind, call the clinic.</p>",
        clinic = escape(&link.clinic_name)
    );
    document("These reminders are stopped", &body).into_response()
}

fn not_found() -> Response {
    let body = "<h1>This link doesn't work</h1>\n\
                <p>It may have been copied only in part. Please call your clinic to stop its reminders.</p>";
    (StatusCode::NOT_FOUND, document("This link doesn't work", body)).into_response()
}

fn failed(err: &str) -> Response {
    tracing::error!(%err, "the opt-out page failed");
    let body = "<h1>Something went wrong</h1>\n<p>Nothing was changed. Please try again in a few minutes.</p>";
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        document("Something went wrong", body),
    )
        .into_response()
}

const STYLE: &str = "body{margin:0;background:#f6f4f0;color:#24211d;font:1.0625rem/1.6 'IBM Plex Sans',system-ui,sans-serif}\
     main{max-width:36rem;margin:3rem auto;padding:2rem;background:#fff;border:1px solid #e4dfd6;border-radius:8px}\
     .letterhead{border-bottom:1px solid #ebe7e0;margin:0 0 1.5rem;padding-bottom:1rem}\
     .clinic{margin:0;font-weight:600;color:#0f5a5c}\
     h1{font-size:1.5rem;line-height:1.25;margin:0 0 .75rem}\
     button{font:inherit;font-weight:600;padding:.65rem 1.25rem;border-radius:6px;border:0;background:#0f5a5c;color:#fff;cursor:pointer;margin-top:.5rem}\
     button:hover{background:#0c4a4c}button:focus-visible{outline:3px solid #7fb9bb;outline-offset:2px}\
     @media (max-width:40rem){main{margin:0;border:0;border-radius:0;padding:1.5rem 1rem}}";

/// A whole page, never cached and never indexed.
fn document(title: &str, body: &str) -> impl IntoResponse {
    let html = Html(format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
         <meta name=\"robots\" content=\"noindex\"><title>{}</title><style>{STYLE}</style></head>\
         <body>\n<main>\n{body}\n</main>\n</body></html>\n",
        escape(title)
    ));
    ([(axum::http::header::CACHE_CONTROL, "no-store")], html)
}
