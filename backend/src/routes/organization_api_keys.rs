//! Organisation OpenAI API key management (native chat feature, Phase 3):
//! `compliance_admin`-only provisioning/rotation of the organisation's
//! single shared OpenAI key, plus basic usage visibility. Same access-
//! control pattern as `routes::policy` — `require_role` checked inside
//! every handler, every query additionally scoped to `claims.
//! organisation_id` (defense in depth, matching this codebase's existing
//! discipline throughout).
//!
//! The real key is NEVER returned once saved — only `last_four` (see
//! migration 0017's own comment on why that fragment alone is safe to store
//! and display in the clear) and timestamps. Provisioning and rotation are
//! the same operation (`PUT`, an upsert on `organization_api_keys`'
//! `UNIQUE(organisation_id, provider)` constraint) — a second `PUT` for an
//! organisation that already has a key simply replaces it and stamps
//! `rotated_at`, leaving `created_at` untouched.
use axum::{extract::State, Json};
use chrono::{DateTime, Utc};

use crate::{
    auth::{require_role, AuthUser},
    crypto,
    error::{AppError, AppResult},
    models::{OpenAiKeyStatusResponse, OpenAiKeyUsageResponse, SetOpenAiKeyRequest},
    state::AppState,
};

/// A real but deliberately permissive format check — OpenAI's own key
/// formats have changed shape over time (`sk-...` vs. the newer
/// `sk-proj-...`), so this only rejects what's structurally implausible
/// (wrong prefix, wrong length, characters OpenAI keys never contain)
/// rather than pattern-matching one exact historical shape that might
/// reject a legitimately-formatted future key.
fn validate_openai_key_format(key: &str) -> AppResult<()> {
    if !key.starts_with("sk-") {
        return Err(AppError::BadRequest(
            "An OpenAI API key must start with 'sk-'.".to_string(),
        ));
    }
    if key.len() < 20 || key.len() > 200 {
        return Err(AppError::BadRequest(
            "That doesn't look like a real OpenAI API key (unexpected length).".to_string(),
        ));
    }
    if !key.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(AppError::BadRequest(
            "An OpenAI API key should only contain letters, digits, hyphens, and underscores."
                .to_string(),
        ));
    }
    Ok(())
}

async fn load_status(state: &AppState, organisation_id: uuid::Uuid) -> AppResult<OpenAiKeyStatusResponse> {
    let row: Option<(String, DateTime<Utc>, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT last_four, created_at, rotated_at FROM organization_api_keys \
         WHERE organisation_id = $1 AND provider = 'openai'",
    )
    .bind(organisation_id)
    .fetch_optional(&state.db)
    .await?;

    Ok(match row {
        Some((last_four, created_at, rotated_at)) => OpenAiKeyStatusResponse {
            configured: true,
            last_four: Some(last_four),
            created_at: Some(created_at),
            rotated_at,
        },
        None => OpenAiKeyStatusResponse {
            configured: false,
            last_four: None,
            created_at: None,
            rotated_at: None,
        },
    })
}

pub async fn get_status(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<OpenAiKeyStatusResponse>> {
    require_role(&claims, &["compliance_admin"])?;
    Ok(Json(load_status(&state, claims.organisation_id).await?))
}

/// Provisions a new key, or rotates an existing one — the same operation,
/// distinguished only by whether a row already existed (see this module's
/// own doc comment).
pub async fn set_key(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<SetOpenAiKeyRequest>,
) -> AppResult<Json<OpenAiKeyStatusResponse>> {
    require_role(&claims, &["compliance_admin"])?;

    let api_key = payload.api_key.trim();
    validate_openai_key_format(api_key)?;

    let encrypted_key = crypto::encrypt_secret(api_key, &state.config.api_key_encryption_key)?;
    let last_four = crypto::last_four(api_key);

    sqlx::query(
        r#"
        INSERT INTO organization_api_keys (organisation_id, provider, encrypted_key, last_four, created_by)
        VALUES ($1, 'openai', $2, $3, $4)
        ON CONFLICT (organisation_id, provider)
        DO UPDATE SET encrypted_key = $2, last_four = $3, rotated_at = now()
        "#,
    )
    .bind(claims.organisation_id)
    .bind(&encrypted_key)
    .bind(&last_four)
    .bind(claims.sub)
    .execute(&state.db)
    .await?;

    // Real observability, same convention as routes/policy.rs's own
    // threshold/pattern changes — never logs the key itself, only that a
    // change happened and who made it.
    tracing::info!(
        organisation_id = %claims.organisation_id,
        updated_by = %claims.sub,
        "organisation OpenAI API key provisioned or rotated"
    );

    Ok(Json(load_status(&state, claims.organisation_id).await?))
}

const ALLOWED_USAGE_WINDOW_DAYS: [i64; 3] = [7, 30, 90];

/// Basic internal usage count (chat feature, Phase 3): counts `audit_log`
/// rows in the requested window where `ai_model_used` shows a real OpenAI
/// call was made — i.e. every forwarded chat turn, excluding blocked ones
/// (which never reached OpenAI at all; see `routes::chat`'s
/// `BLOCKED_AI_MODEL_LABEL`). Pulled from this codebase's own data, not
/// OpenAI's billing API — that integration was judged not trivial (a
/// separate authenticated call to a different OpenAI endpoint, its own
/// error handling and rate limits, and a cost/usage object shape this
/// codebase would need to normalise into something worth displaying) for
/// what this task asked for as a "basic usage visibility" minimum. Stated
/// plainly rather than silently skipped — see Questions.md.
pub async fn get_usage(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> AppResult<Json<OpenAiKeyUsageResponse>> {
    require_role(&claims, &["compliance_admin"])?;

    let days: i64 = params
        .get("days")
        .and_then(|v| v.parse::<i64>().ok())
        .unwrap_or(30);
    if !ALLOWED_USAGE_WINDOW_DAYS.contains(&days) {
        return Err(AppError::BadRequest(format!(
            "days must be one of {:?} (received {}).",
            ALLOWED_USAGE_WINDOW_DAYS, days
        )));
    }

    let request_count: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*) FROM audit_log
        WHERE organisation_id = $1
          AND ai_model_used LIKE 'openai:%'
          AND created_at >= now() - ($2 || ' days')::interval
        "#,
    )
    .bind(claims.organisation_id)
    .bind(days.to_string())
    .fetch_one(&state.db)
    .await?;

    Ok(Json(OpenAiKeyUsageResponse { days, request_count }))
}
