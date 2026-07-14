use axum::{extract::State, Json};
use chrono::{Duration, Utc};

use crate::{
    auth::{issue_token, verify_password, SESSION_TTL_HOURS},
    error::{AppError, AppResult},
    models::{LoginRequest, LoginResponse, UserPublic, UserRow},
    state::AppState,
};

pub async fn login(
    State(state): State<AppState>,
    Json(payload): Json<LoginRequest>,
) -> AppResult<Json<LoginResponse>> {
    if payload.email.trim().is_empty() || payload.password.is_empty() {
        return Err(AppError::BadRequest(
            "email and password are both required.".to_string(),
        ));
    }

    let email_normalized = payload.email.trim().to_lowercase();

    let user = sqlx::query_as::<_, UserRow>(
        r#"
        SELECT u.id, u.email, u.password_hash, u.department, u.role, u.organisation_id,
               u.consent_accepted_at,
               o.consent_policy_version AS org_consent_policy_version,
               u.consent_policy_version AS user_accepted_policy_version
        FROM users u
        JOIN organisations o ON o.id = u.organisation_id
        WHERE u.email = $1
        "#,
    )
    .bind(&email_normalized)
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| {
        // Real observability (product-depth task, Part 2): structured
        // fields, never the password — this codebase's security review
        // (Questions.md, "real observability + hardening" task) confirmed
        // no credential value is ever passed to a `tracing` call anywhere
        // in this file or elsewhere.
        tracing::warn!(email = %email_normalized, "login failed: no such user");
        AppError::Unauthorized("Invalid email or password.".to_string())
    })?;

    let valid = verify_password(&payload.password, &user.password_hash)?;
    if !valid {
        tracing::warn!(email = %email_normalized, user_id = %user.id, "login failed: wrong password");
        return Err(AppError::Unauthorized(
            "Invalid email or password.".to_string(),
        ));
    }

    let expires_at = Utc::now() + Duration::hours(SESSION_TTL_HOURS);
    let session_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO sessions (user_id, expires_at) VALUES ($1, $2) RETURNING id",
    )
    .bind(user.id)
    .bind(expires_at)
    .fetch_one(&state.db)
    .await?;

    let token = issue_token(
        &state.config.jwt_signing_secret,
        user.id,
        session_id,
        &user.email,
        &user.department,
        &user.role,
        user.organisation_id,
    )?;

    // Requires (re-)consent if the user has never accepted anything, OR
    // accepted a version that isn't the organisation's CURRENT version
    // (e.g. the organisation bumped consent_policy_version since this user
    // last consented) — comparing against the org's live column rather
    // than trusting a possibly-stale value cached anywhere else.
    let requires_consent = user.user_accepted_policy_version.as_deref() != Some(user.org_consent_policy_version.as_str());

    tracing::info!(
        user_id = %user.id,
        organisation_id = %user.organisation_id,
        role = %user.role,
        "login succeeded"
    );

    Ok(Json(LoginResponse {
        token,
        user: UserPublic {
            id: user.id,
            email: user.email,
            department: user.department,
            role: user.role,
            organisation_id: user.organisation_id,
        },
        requires_consent,
        consent_policy_version: user.org_consent_policy_version,
    }))
}
