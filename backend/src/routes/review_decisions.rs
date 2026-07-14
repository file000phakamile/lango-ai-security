//! Active learning loop (product-depth task, Part 3): when a
//! `compliance_admin` or `department_reviewer` confirms or overturns a
//! flagged low-confidence audit_log row, this handler records that human
//! judgment as a labelled example — not just a status change on the
//! audit_log row itself (nothing on the `audit_log` table is mutated here
//! at all; `review_decisions` is a separate, append-only table — see
//! migration 0014). `routes::labelled_dataset` is the export side of this
//! same feature.
//!
//! Explicitly out of scope, per the task: nothing here retrains or
//! fine-tunes anything automatically. This module only ever captures
//! signal.
use axum::{
    extract::{Path, State},
    Json,
};
use uuid::Uuid;

use crate::{
    auth::{require_role, AuthUser},
    error::{AppError, AppResult},
    models::{RecordReviewDecisionResponse, RecordReviewDecisionRequest, REVIEWABLE_DECISIONS, VALID_REVIEW_DECISIONS},
    state::AppState,
};

#[derive(sqlx::FromRow)]
struct TargetRow {
    decision: String,
    department: String,
    entities_detected: sqlx::types::Json<Vec<String>>,
    risk_score: f32,
    reason_string: String,
    sensitivity_class: String,
}

pub async fn record_review_decision(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(audit_log_id): Path<Uuid>,
    Json(payload): Json<RecordReviewDecisionRequest>,
) -> AppResult<Json<RecordReviewDecisionResponse>> {
    // 'staff' cannot review anything — same two dashboard-facing roles as
    // every other reviewing/reading endpoint in this codebase.
    require_role(&claims, &["compliance_admin", "department_reviewer"])?;

    if !VALID_REVIEW_DECISIONS.contains(&payload.decision.as_str()) {
        return Err(AppError::BadRequest(format!(
            "decision must be one of: {}",
            VALID_REVIEW_DECISIONS.join(", ")
        )));
    }
    if let Some(reasoning) = &payload.reasoning {
        if reasoning.len() > 2_000 {
            return Err(AppError::BadRequest(
                "reasoning exceeds the maximum accepted length (2,000 chars).".to_string(),
            ));
        }
    }

    // Org-scoped fetch — the same query-level tenant boundary as every
    // other endpoint, not just a role check. A row belonging to a
    // different organisation looks identical to a nonexistent one from the
    // caller's perspective, which is the correct behavior (not a 403 that
    // would confirm the id exists elsewhere).
    let target: Option<TargetRow> = sqlx::query_as(
        r#"
        SELECT decision, department, entities_detected, risk_score, reason_string, sensitivity_class
        FROM audit_log
        WHERE id = $1 AND organisation_id = $2
        "#,
    )
    .bind(audit_log_id)
    .bind(claims.organisation_id)
    .fetch_optional(&state.db)
    .await?;

    let target = target.ok_or_else(|| {
        AppError::NotFound("Audit log row not found in your organisation.".to_string())
    })?;

    // department_reviewer is further scoped to their own department, same
    // as their read access to the audit log (routes::audit_log) — they can
    // only record a judgment on a row they'd actually be able to see.
    if claims.role == "department_reviewer" && target.department != claims.department {
        return Err(AppError::Forbidden(
            "You may only review audit log rows in your own department.".to_string(),
        ));
    }

    if !REVIEWABLE_DECISIONS.contains(&target.decision.as_str()) {
        return Err(AppError::BadRequest(format!(
            "Only rows with decision {} are eligible for a review judgment (this row's decision is '{}').",
            REVIEWABLE_DECISIONS.join(" or "),
            target.decision
        )));
    }

    let insert_result = sqlx::query(
        r#"
        INSERT INTO review_decisions (
            audit_log_id, organisation_id, reviewer_user_id, reviewer_role, decision, reasoning,
            original_decision, original_entities_detected, original_risk_score, original_reason_string,
            original_sensitivity_class, original_department
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
        "#,
    )
    .bind(audit_log_id)
    .bind(claims.organisation_id)
    .bind(claims.sub)
    .bind(&claims.role)
    .bind(&payload.decision)
    .bind(&payload.reasoning)
    .bind(&target.decision)
    .bind(&target.entities_detected)
    .bind(target.risk_score)
    .bind(&target.reason_string)
    .bind(&target.sensitivity_class)
    .bind(&target.department)
    .execute(&state.db)
    .await;

    match insert_result {
        Ok(_) => Ok(Json(RecordReviewDecisionResponse { recorded: true })),
        // audit_log_id UNIQUE — a second attempt to review the same row is
        // rejected rather than silently overwriting the first reviewer's
        // judgment (see migration 0014's own comment on why).
        Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => Err(AppError::BadRequest(
            "This audit log row already has a recorded review decision.".to_string(),
        )),
        Err(e) => Err(AppError::from(e)),
    }
}
