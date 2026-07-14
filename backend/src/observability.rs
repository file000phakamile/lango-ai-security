//! Real observability ("response scanning + observability + hardening"
//! task, Part 2) — the internal error log's write path. See migration
//! `0016_create_backend_errors.sql` for the schema and the reasoning
//! behind building this instead of (or alongside) a third-party error
//! tracking service.
use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

/// Wraps every route (see `main.rs`'s `.layer(axum::middleware::from_fn_with_state(...))`)
/// — a single choke point for capturing every 5xx response, rather than
/// something each handler has to remember to call individually. Mirrors
/// `error.rs`'s existing `tracing::error!` call for the same class of
/// failure (a genuine server-side error, not a client mistake like a bad
/// request or an expired token) — this middleware only acts on responses
/// whose status is already `>= 500`, which `error.rs` is the sole producer
/// of.
///
/// The DB write happens on a spawned task, not awaited inline: a failure to
/// log an error must never itself turn into a slower response (or a second
/// error) for the caller who already hit a real problem. If the write
/// itself fails (e.g. the database is down — plausibly the exact reason a
/// request just 500'd), it's silently dropped; `tracing::error!` in
/// `error.rs` is still the durable, always-available record of every
/// server error via the deployment's own log output, which this table
/// supplements for in-dashboard visibility, not replaces.
pub async fn error_log_middleware(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();

    let response = next.run(req).await;

    if !response.status().is_server_error() {
        return response;
    }

    let status_code = response.status().as_u16() as i16;
    let (parts, body) = response.into_parts();
    let bytes = match axum::body::to_bytes(body, 64 * 1024).await {
        Ok(b) => b,
        // The body itself failed to read — log what we know (method, path,
        // status) with no message rather than dropping the whole record.
        Err(_) => {
            spawn_insert(state, method, path, status_code, None);
            return Response::from_parts(parts, Body::empty());
        }
    };

    // Every AppError-produced body has this exact shape (see error.rs) —
    // parsed defensively, not assumed, since a 5xx from somewhere outside
    // AppError's own IntoResponse impl (there isn't one today, but this
    // shouldn't panic if that ever changes) would have a different shape.
    let message: Option<String> = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|v| v.get("error").and_then(|e| e.get("message")).and_then(|m| m.as_str()).map(String::from));

    spawn_insert(state, method, path, status_code, message);

    Response::from_parts(parts, Body::from(bytes))
}

fn spawn_insert(state: AppState, method: String, path: String, status_code: i16, message: Option<String>) {
    tokio::spawn(async move {
        let _ = sqlx::query(
            "INSERT INTO backend_errors (method, path, status_code, message) VALUES ($1, $2, $3, $4)",
        )
        .bind(method)
        .bind(path)
        .bind(status_code)
        .bind(message)
        .execute(&state.db)
        .await;
    });
}

