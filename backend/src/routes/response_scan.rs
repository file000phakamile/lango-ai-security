//! Response scanning ("response scanning + observability + hardening"
//! task, Part 1) — the second half of the pipeline. `routes::scan` handles
//! the prompt side (before it's sent); this handles the response side
//! (after the AI provider's reply has rendered in the browser). See
//! `detection::scan::scan_response`'s doc comment for the full design
//! reasoning on why a flagged response is never modified, only flagged.
use axum::{extract::State, Json};
use chrono::{DateTime, Utc};

use crate::{
    auth::AuthUser,
    detection::scan::{hash_prompt, scan_response},
    error::{AppError, AppResult},
    models::{ScanResponseCheckRequest, ScanResponseCheckResponse},
    routes::scan::{check_consent, load_scan_config},
    state::AppState,
};

#[derive(sqlx::FromRow)]
struct TargetRow {
    decision: String,
    response_scanned_at: Option<DateTime<Utc>>,
}

pub async fn scan_response_handler(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<ScanResponseCheckRequest>,
) -> AppResult<Json<ScanResponseCheckResponse>> {
    if payload.response_text.trim().is_empty() {
        return Err(AppError::BadRequest("response_text must not be empty.".to_string()));
    }
    // Same cap as the prompt side (routes::scan) — a response this long is
    // implausible for a chat UI turn and more likely a caller bug than a
    // real reply.
    if payload.response_text.len() > 20_000 {
        return Err(AppError::BadRequest(
            "response_text exceeds the maximum accepted length (20,000 chars).".to_string(),
        ));
    }

    // Performance pass, Step 2/3: the consent check and the ownership
    // lookup below are independent (different tables, neither result
    // depends on the other) — fired concurrently rather than sequentially,
    // same reasoning as routes/scan.rs. `load_scan_config` deliberately
    // stays sequential, AFTER the ownership check below, not joined in here
    // — if ownership fails, this request should fail fast without spending
    // a third round trip loading a policy config it will never use.
    //
    // Ownership check: the audit_log row must belong to THIS caller, in
    // THIS organisation — a real query-level boundary, not just a role
    // check, so one user can never attach response text to another user's
    // audit trail (which would let them fabricate or pollute someone
    // else's compliance record) — same "org, optionally department, real
    // WHERE clause" discipline as every other multi-tenant query in this
    // codebase.
    // `try_join!` requires every future to share one error type; wrapped in
    // an async block so this query's `sqlx::Error` maps to `AppError` the
    // same way the `?` operator already does everywhere else in this file,
    // rather than the two futures disagreeing on error type.
    let ownership_fut = async {
        sqlx::query_as::<_, TargetRow>(
            "SELECT decision, response_scanned_at FROM audit_log WHERE id = $1 AND user_id = $2 AND organisation_id = $3",
        )
        .bind(payload.audit_log_id)
        .bind(claims.sub)
        .bind(claims.organisation_id)
        .fetch_optional(&state.db)
        .await
        .map_err(AppError::from)
    };

    let (_, target) = tokio::try_join!(check_consent(&state.db, claims.sub), ownership_fut)?;

    let target = target.ok_or_else(|| {
        AppError::NotFound("Audit log row not found for this user in this organisation.".to_string())
    })?;

    if target.response_scanned_at.is_some() {
        return Err(AppError::BadRequest(
            "A response scan has already been recorded for this row.".to_string(),
        ));
    }
    if target.decision == "blocked_low_confidence" {
        return Err(AppError::BadRequest(
            "This row's prompt was blocked pre-gateway - nothing was sent, so there is no response to scan."
                .to_string(),
        ));
    }

    let scan_config = load_scan_config(&state.db, claims.organisation_id).await?;
    let outcome = scan_response(&payload.response_text, &scan_config);
    let response_text_hash = hash_prompt(&payload.response_text);
    let entities_json = serde_json::to_value(&outcome.entities_detected)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        UPDATE audit_log
        SET response_entities_detected = $1,
            response_risk_score = $2,
            response_flagged = $3,
            response_text_hash = $4,
            response_scanned_at = now(),
            response_scan_result = $5
        WHERE id = $6
        "#,
    )
    .bind(&entities_json)
    .bind(outcome.risk_score)
    .bind(outcome.flagged)
    .bind(&response_text_hash)
    .bind(&outcome.user_message)
    .bind(payload.audit_log_id)
    .execute(&state.db)
    .await?;

    Ok(Json(ScanResponseCheckResponse {
        flagged: outcome.flagged,
        user_message: outcome.user_message,
        entities_detected: outcome.entities_detected,
    }))
}

