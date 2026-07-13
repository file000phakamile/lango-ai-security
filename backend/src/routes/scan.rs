use axum::{extract::State, Json};

use crate::{
    auth::AuthUser,
    detection::scan::{hash_prompt, response_scan_result_for, scan_prompt, NO_PROVIDER_MODEL_LABEL},
    error::{AppError, AppResult},
    models::{ScanRequest, ScanResponse},
    state::AppState,
};

pub async fn scan(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<ScanRequest>,
) -> AppResult<Json<ScanResponse>> {
    if payload.prompt.trim().is_empty() {
        return Err(AppError::BadRequest("prompt must not be empty.".to_string()));
    }
    if payload.prompt.len() > 20_000 {
        return Err(AppError::BadRequest(
            "prompt exceeds the maximum accepted length (20,000 chars).".to_string(),
        ));
    }

    let outcome = scan_prompt(&payload.prompt);
    let original_prompt_hash = hash_prompt(&payload.prompt);
    let response_scan_result = response_scan_result_for(outcome.decision);

    // Both decisions that actually forward a prompt store the redacted
    // version — a low-confidence-but-forwarded name match still needs its
    // redacted text on record for the compliance review this decision
    // exists to flag.
    let redacted_prompt_for_storage =
        if outcome.decision == "redacted_and_forwarded" || outcome.decision == "redacted_low_confidence_review" {
            Some(outcome.redacted_prompt.clone())
        } else {
            None
        };

    let entities_json = serde_json::to_value(&outcome.entities_detected)
        .map_err(|e| AppError::Internal(e.to_string()))?;

    sqlx::query(
        r#"
        INSERT INTO audit_log (
            session_id, user_id, department, language, "timestamp",
            entities_detected, risk_score, decision, reason_string,
            ai_model_used, response_scan_result, original_prompt_hash, redacted_prompt,
            sensitivity_class, facility_type
        )
        VALUES ($1, $2, $3, $4, now(), $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
        "#,
    )
    .bind(claims.session_id)
    .bind(claims.sub)
    .bind(&claims.department)
    .bind(&payload.language)
    .bind(&entities_json)
    .bind(outcome.risk_score)
    .bind(outcome.decision)
    .bind(&outcome.reason_string)
    .bind(NO_PROVIDER_MODEL_LABEL)
    .bind(&response_scan_result)
    .bind(&original_prompt_hash)
    .bind(&redacted_prompt_for_storage)
    .bind(outcome.sensitivity_class)
    .bind(&payload.facility_type)
    .execute(&state.db)
    .await?;

    Ok(Json(ScanResponse {
        entities_detected: outcome.entities_detected,
        risk_score: outcome.risk_score,
        redacted_prompt: outcome.redacted_prompt,
        decision: outcome.decision.to_string(),
        reason_string: outcome.reason_string,
        user_message: outcome.user_message,
        sensitivity_class: outcome.sensitivity_class.to_string(),
    }))
}
