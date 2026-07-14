//! Policy builder (product-depth task, Part 1): lets a `compliance_admin`
//! adjust their own organisation's confidence threshold, within safe
//! hard-coded bounds, and add organisation-specific structured-identifier
//! patterns — both scoped strictly to the caller's own organisation
//! (`claims.organisation_id`, no exceptions, same pattern as every other
//! multi-tenant query in this codebase).
//!
//! What is deliberately NOT configurable, anywhere in this file: the
//! near-zero fail-closed name floor (`NAME_LOW_CONFIDENCE_FLOOR`) and the
//! special_category_health leniency hard rule (`is_leniency_eligible` in
//! `detection::scan`). Neither is read from nor written to any table this
//! module touches — see `detection::scan::MIN_ORG_CONFIDENCE_THRESHOLD`'s
//! own doc comment for why the floor here (0.50) sits comfortably above
//! that non-configurable floor (0.30) rather than merely avoiding it.
use axum::{
    extract::{Path, State},
    Json,
};
use regex::RegexBuilder;
use uuid::Uuid;

use crate::{
    auth::{require_role, AuthUser},
    detection::scan,
    error::{AppError, AppResult},
    models::{
        CreateCustomPatternRequest, CustomPatternResponse, PolicySettingsResponse,
        UpdateThresholdRequest,
    },
    state::AppState,
};

/// Built-in entity type strings — an organisation cannot name a custom
/// pattern one of these, so a custom pattern can never be confused with (or
/// silently reclassify) a built-in detector's output, and — since
/// `special_category_health` classification is a fixed mapping keyed
/// exactly on these five names (see `health_rules::sensitivity_class`) — a
/// custom pattern can never land in `special_category_health` and inherit
/// its leniency exclusion, keeping that hard rule genuinely un-configurable
/// rather than merely "not exposed in the UI".
const RESERVED_ENTITY_LABELS: &[&str] = &[
    "national_id",
    "bank_account",
    "phone_number",
    "credit_card",
    "api_key",
    "medical_record_no",
    "full_name",
    "diagnosis_code",
    "medication_name",
    "medical_aid_number",
    "lab_result_value",
    "next_of_kin",
    "probable_identifier",
];

fn validate_entity_label(label: &str) -> AppResult<()> {
    if label.len() < 3 || label.len() > 40 {
        return Err(AppError::BadRequest(
            "entity_label must be 3-40 characters long.".to_string(),
        ));
    }
    let mut chars = label.chars();
    let first_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase());
    let rest_ok = chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !first_ok || !rest_ok {
        return Err(AppError::BadRequest(
            "entity_label must start with a lowercase letter and contain only lowercase \
             letters, digits, and underscores (e.g. 'acme_bank_account_format')."
                .to_string(),
        ));
    }
    if RESERVED_ENTITY_LABELS.contains(&label) {
        return Err(AppError::BadRequest(format!(
            "'{}' is a built-in entity type and cannot be used as a custom pattern label.",
            label
        )));
    }
    Ok(())
}

async fn load_settings(
    state: &AppState,
    organisation_id: Uuid,
) -> AppResult<PolicySettingsResponse> {
    let confidence_threshold: f32 = sqlx::query_scalar(
        "SELECT confidence_threshold FROM organisation_detection_settings WHERE organisation_id = $1",
    )
    .bind(organisation_id)
    .fetch_optional(&state.db)
    .await?
    .unwrap_or(scan::CONFIDENCE_THRESHOLD);

    let custom_patterns: Vec<CustomPatternResponse> = sqlx::query_as(
        r#"
        SELECT id, entity_label, pattern, confidence, active, created_at
        FROM organisation_custom_patterns
        WHERE organisation_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(organisation_id)
    .fetch_all(&state.db)
    .await?;

    Ok(PolicySettingsResponse {
        confidence_threshold,
        min_confidence_threshold: scan::MIN_ORG_CONFIDENCE_THRESHOLD,
        max_confidence_threshold: scan::MAX_ORG_CONFIDENCE_THRESHOLD,
        custom_patterns,
    })
}

pub async fn get_settings(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<PolicySettingsResponse>> {
    require_role(&claims, &["compliance_admin"])?;
    Ok(Json(load_settings(&state, claims.organisation_id).await?))
}

pub async fn update_threshold(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<UpdateThresholdRequest>,
) -> AppResult<Json<PolicySettingsResponse>> {
    require_role(&claims, &["compliance_admin"])?;

    // Server-side bound enforcement — the actual guarantee, not just a UI
    // affordance. Checked here (returning a clean 400) in addition to the
    // DB-level CHECK constraint in migration 0013, so an out-of-range value
    // never even reaches a raw constraint-violation error.
    if !(scan::MIN_ORG_CONFIDENCE_THRESHOLD..=scan::MAX_ORG_CONFIDENCE_THRESHOLD)
        .contains(&payload.confidence_threshold)
    {
        return Err(AppError::BadRequest(format!(
            "confidence_threshold must be between {:.2} and {:.2} (received {:.2}). This bound \
             keeps the fail-closed guarantee real — it is not configurable past these limits.",
            scan::MIN_ORG_CONFIDENCE_THRESHOLD,
            scan::MAX_ORG_CONFIDENCE_THRESHOLD,
            payload.confidence_threshold
        )));
    }

    sqlx::query(
        r#"
        INSERT INTO organisation_detection_settings (organisation_id, confidence_threshold, updated_by)
        VALUES ($1, $2, $3)
        ON CONFLICT (organisation_id)
        DO UPDATE SET confidence_threshold = $2, updated_at = now(), updated_by = $3
        "#,
    )
    .bind(claims.organisation_id)
    .bind(payload.confidence_threshold)
    .bind(claims.sub)
    .execute(&state.db)
    .await?;

    Ok(Json(load_settings(&state, claims.organisation_id).await?))
}

pub async fn create_custom_pattern(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<CreateCustomPatternRequest>,
) -> AppResult<Json<PolicySettingsResponse>> {
    require_role(&claims, &["compliance_admin"])?;

    let entity_label = payload.entity_label.trim().to_string();
    validate_entity_label(&entity_label)?;

    let pattern = payload.pattern.trim().to_string();
    if pattern.is_empty() {
        return Err(AppError::BadRequest("pattern must not be empty.".to_string()));
    }
    if pattern.len() > scan::MAX_CUSTOM_PATTERN_LENGTH {
        return Err(AppError::BadRequest(format!(
            "pattern exceeds the maximum accepted length ({} chars).",
            scan::MAX_CUSTOM_PATTERN_LENGTH
        )));
    }
    // Rust's `regex` crate cannot catastrophically backtrack (see
    // detection/mod.rs's audit note) — `size_limit` here guards against a
    // different failure mode: a pathological pattern whose compiled
    // NFA/DFA program is simply too large, regardless of matching speed.
    if RegexBuilder::new(&pattern).size_limit(1_000_000).build().is_err() {
        return Err(AppError::BadRequest(
            "pattern is not a valid regular expression, or compiles to a program that's too \
             large. Try a simpler or more specific pattern."
                .to_string(),
        ));
    }

    let confidence = payload.confidence.unwrap_or(0.80);
    if !(scan::MIN_ORG_CONFIDENCE_THRESHOLD..=scan::MAX_ORG_CONFIDENCE_THRESHOLD).contains(&confidence) {
        return Err(AppError::BadRequest(format!(
            "confidence must be between {:.2} and {:.2} (received {:.2}).",
            scan::MIN_ORG_CONFIDENCE_THRESHOLD,
            scan::MAX_ORG_CONFIDENCE_THRESHOLD,
            confidence
        )));
    }

    sqlx::query(
        r#"
        INSERT INTO organisation_custom_patterns (organisation_id, entity_label, pattern, confidence, created_by)
        VALUES ($1, $2, $3, $4, $5)
        "#,
    )
    .bind(claims.organisation_id)
    .bind(&entity_label)
    .bind(&pattern)
    .bind(confidence)
    .bind(claims.sub)
    .execute(&state.db)
    .await?;

    Ok(Json(load_settings(&state, claims.organisation_id).await?))
}

pub async fn delete_custom_pattern(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<PolicySettingsResponse>> {
    require_role(&claims, &["compliance_admin"])?;

    // Org-scoped delete — an admin can only ever remove a pattern that
    // belongs to their own organisation, not merely one they can guess the
    // id of (see multi_tenant_isolation.rs's pattern for why this is a real
    // WHERE clause, not just a role check).
    let result = sqlx::query(
        "DELETE FROM organisation_custom_patterns WHERE id = $1 AND organisation_id = $2",
    )
    .bind(id)
    .bind(claims.organisation_id)
    .execute(&state.db)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(
            "Custom pattern not found in your organisation.".to_string(),
        ));
    }

    Ok(Json(load_settings(&state, claims.organisation_id).await?))
}
