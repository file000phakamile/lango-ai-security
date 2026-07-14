use axum::{extract::State, Json};
use regex::Regex;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::AuthUser,
    detection::scan::{
        hash_prompt, response_scan_result_for, scan_prompt_with_config, CustomPattern,
        ScanConfig, NO_PROVIDER_MODEL_LABEL,
    },
    error::{AppError, AppResult},
    models::{ScanRequest, ScanResponse},
    state::AppState,
};

/// Consent gate (Part 4 of the multi-tenancy task): "before any scanning is
/// permitted" is a hard requirement, enforced here server-side — checked
/// fresh against the database on every call, NOT read from the JWT, since a
/// token issued at login time can't reflect a consent acceptance (or an
/// organisation's policy-version bump) that happened after that token was
/// minted. `pub(crate)` so `routes::response_scan` can apply the exact same
/// gate to a response scan, rather than re-implementing (and risking
/// drifting from) this query.
pub(crate) async fn check_consent(pool: &PgPool, user_id: Uuid) -> AppResult<()> {
    let (user_consent_version, org_consent_version): (Option<String>, String) = sqlx::query_as(
        r#"
        SELECT u.consent_policy_version, o.consent_policy_version
        FROM users u
        JOIN organisations o ON o.id = u.organisation_id
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await?;
    if user_consent_version.as_deref() != Some(org_consent_version.as_str()) {
        return Err(AppError::ConsentRequired(
            "Data-use consent is required before scanning. Please open the Lango extension \
             popup and accept the consent screen."
                .to_string(),
        ));
    }
    Ok(())
}

/// Loads this organisation's configured confidence threshold (falling back
/// to the system default if they've never customized it — policy builder,
/// product-depth task Part 1) and active custom patterns, fresh on every
/// call — same "read live from the DB, never from the JWT" reasoning as
/// `check_consent` above, since an org's policy can change between token
/// issuance and this request. A custom pattern whose stored regex somehow
/// fails to recompile (it was validated at creation time in
/// routes/policy.rs, so this should never happen) is skipped rather than
/// failing the whole scan. `pub(crate)` so `routes::response_scan` builds
/// its `ScanConfig` identically to the prompt path — a response scan must
/// use the SAME org policy (threshold, custom patterns) a prompt scan
/// would, not a second, potentially-drifted copy of it.
pub(crate) async fn load_scan_config(pool: &PgPool, organisation_id: Uuid) -> AppResult<ScanConfig> {
    let org_confidence_threshold: f32 = sqlx::query_scalar(
        "SELECT confidence_threshold FROM organisation_detection_settings WHERE organisation_id = $1",
    )
    .bind(organisation_id)
    .fetch_optional(pool)
    .await?
    .unwrap_or(crate::detection::scan::CONFIDENCE_THRESHOLD);

    let custom_pattern_rows: Vec<(String, String, f32)> = sqlx::query_as(
        r#"
        SELECT entity_label, pattern, confidence
        FROM organisation_custom_patterns
        WHERE organisation_id = $1 AND active = true
        "#,
    )
    .bind(organisation_id)
    .fetch_all(pool)
    .await?;

    let custom_patterns: Vec<CustomPattern> = custom_pattern_rows
        .into_iter()
        .filter_map(|(entity_label, pattern, confidence)| {
            Regex::new(&pattern).ok().map(|regex| CustomPattern {
                entity_label,
                regex,
                confidence,
            })
        })
        .collect();

    Ok(ScanConfig { confidence_threshold: org_confidence_threshold, custom_patterns })
}

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

    check_consent(&state.db, claims.sub).await?;
    let scan_config = load_scan_config(&state.db, claims.organisation_id).await?;

    let outcome = scan_prompt_with_config(&payload.prompt, &scan_config);
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

    // RETURNING id: response scanning (product-depth task Part 1) needs a
    // way to correlate the AI's reply, once it stabilises, back to this
    // exact audit_log row — see ScanResponse::id's own doc comment.
    let audit_log_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO audit_log (
            session_id, user_id, department, language, "timestamp",
            entities_detected, risk_score, decision, reason_string,
            ai_model_used, response_scan_result, original_prompt_hash, redacted_prompt,
            sensitivity_class, facility_type, organisation_id
        )
        VALUES ($1, $2, $3, $4, now(), $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        RETURNING id
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
    .bind(claims.organisation_id)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(ScanResponse {
        id: audit_log_id,
        entities_detected: outcome.entities_detected,
        risk_score: outcome.risk_score,
        redacted_prompt: outcome.redacted_prompt,
        decision: outcome.decision.to_string(),
        reason_string: outcome.reason_string,
        user_message: outcome.user_message,
        sensitivity_class: outcome.sensitivity_class.to_string(),
    }))
}
