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

    let user = sqlx::query_as::<_, UserRow>(
        "SELECT id, email, password_hash, department, role FROM users WHERE email = $1",
    )
    .bind(payload.email.trim().to_lowercase())
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(|| AppError::Unauthorized("Invalid email or password.".to_string()))?;

    let valid = verify_password(&payload.password, &user.password_hash)?;
    if !valid {
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
    )?;

    Ok(Json(LoginResponse {
        token,
        user: UserPublic {
            id: user.id,
            email: user.email,
            department: user.department,
            role: user.role,
        },
    }))
}
