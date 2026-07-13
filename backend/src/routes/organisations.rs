use axum::{extract::State, Json};
use chrono::{Duration, Utc};

use crate::{
    auth::{hash_password, issue_token, SESSION_TTL_HOURS},
    error::{AppError, AppResult},
    models::{LoginResponse, OrganisationSignupRequest, UserPublic},
    state::AppState,
};

/// Self-service organisation signup — Part 5 of the multi-tenancy task.
/// Creates a brand-new organisation and its first user (always
/// `compliance_admin`, since there's no one else in the organisation yet to
/// review anything), then logs that user in immediately (same token/session
/// shape as `POST /api/auth/login`).
///
/// Deliberately minimal, and honestly so: no email verification, no
/// invitation flow for additional users, no way to add a second
/// organisation admin except by sharing this same account's credentials.
/// This is a real, working first version — see docs/ARCHITECTURE.md and
/// Questions.md for what's explicitly deferred.
pub async fn signup(
    State(state): State<AppState>,
    Json(payload): Json<OrganisationSignupRequest>,
) -> AppResult<Json<LoginResponse>> {
    let organisation_name = payload.organisation_name.trim();
    let email = payload.email.trim().to_lowercase();

    if organisation_name.is_empty() {
        return Err(AppError::BadRequest("organisation_name must not be empty.".to_string()));
    }
    if email.is_empty() {
        return Err(AppError::BadRequest("email must not be empty.".to_string()));
    }
    // A real minimum, not a decorative one — Argon2 hashing happens
    // regardless, but a trivially short password defeats the point of it.
    if payload.password.len() < 8 {
        return Err(AppError::BadRequest("password must be at least 8 characters.".to_string()));
    }

    let password_hash = hash_password(&payload.password)?;

    // Both inserts happen in one transaction: if the user insert fails
    // (e.g. the email is already taken), the organisation row must not be
    // left behind as an orphaned, user-less row.
    let mut tx = state.db.begin().await?;

    let org_id: uuid::Uuid = sqlx::query_scalar("INSERT INTO organisations (name) VALUES ($1) RETURNING id")
        .bind(organisation_name)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| map_unique_violation(e, "organisation_name", "That organisation name is already registered."))?;

    // No department scoping makes sense for the very first user of a brand
    // new organisation — they're compliance_admin, which already sees
    // every department in the org, not just one. "Administration" is a
    // placeholder value, not a real department in the department_reviewer
    // sense; it's never used to scope anything for this role.
    const FIRST_ADMIN_DEPARTMENT: &str = "Administration";

    let user_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, department, role, organisation_id) \
         VALUES ($1, $2, $3, 'compliance_admin', $4) RETURNING id",
    )
    .bind(&email)
    .bind(&password_hash)
    .bind(FIRST_ADMIN_DEPARTMENT)
    .bind(org_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| map_unique_violation(e, "email", "That email address is already registered."))?;

    let expires_at = Utc::now() + Duration::hours(SESSION_TTL_HOURS);
    let session_id: uuid::Uuid =
        sqlx::query_scalar("INSERT INTO sessions (user_id, expires_at) VALUES ($1, $2) RETURNING id")
            .bind(user_id)
            .bind(expires_at)
            .fetch_one(&mut *tx)
            .await?;

    tx.commit().await?;

    let token = issue_token(
        &state.config.jwt_signing_secret,
        user_id,
        session_id,
        &email,
        FIRST_ADMIN_DEPARTMENT,
        "compliance_admin",
        org_id,
    )?;

    Ok(Json(LoginResponse {
        token,
        user: UserPublic {
            id: user_id,
            email,
            department: FIRST_ADMIN_DEPARTMENT.to_string(),
            role: "compliance_admin".to_string(),
            organisation_id: org_id,
        },
        // Always true — a brand-new organisation's consent_policy_version
        // default ('v1', see migration 0009) has by definition never been
        // accepted by this brand-new user.
        requires_consent: true,
        consent_policy_version: "v1".to_string(),
    }))
}

/// Turns a Postgres unique-constraint violation into a clear 400 with
/// `field_hint` naming what was actually taken, instead of a raw
/// `AppError::Database` (which the client would just see as "An internal
/// error occurred." — accurate for a real DB failure, misleading for an
/// entirely ordinary "that name's taken" case). Any OTHER database error
/// still falls through to the normal `AppError::Database` handling.
fn map_unique_violation(err: sqlx::Error, field_hint: &str, message: &str) -> AppError {
    let is_unique_violation = err
        .as_database_error()
        .map(|db_err| db_err.code().as_deref() == Some("23505"))
        .unwrap_or(false);
    if is_unique_violation {
        AppError::BadRequest(format!("{message} ({field_hint})"))
    } else {
        AppError::from(err)
    }
}
