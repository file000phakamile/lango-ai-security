use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    async_trait,
    extract::{FromRef, FromRequestParts},
    http::request::Parts,
};
use axum_extra::headers::{authorization::Bearer, Authorization};
use axum_extra::TypedHeader;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use uuid::Uuid;

use crate::{error::AppError, models::Claims, state::AppState};

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Hash(e.to_string()))
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|e| AppError::Hash(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

pub const SESSION_TTL_HOURS: i64 = 12;

pub fn issue_token(
    secret: &str,
    user_id: Uuid,
    session_id: Uuid,
    email: &str,
    department: &str,
    role: &str,
) -> Result<String, AppError> {
    let exp = (Utc::now() + Duration::hours(SESSION_TTL_HOURS)).timestamp() as usize;
    let claims = Claims {
        sub: user_id,
        session_id,
        email: email.to_string(),
        department: department.to_string(),
        role: role.to_string(),
        exp,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(AppError::from)
}

pub fn decode_token(secret: &str, token: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| AppError::Unauthorized("Invalid or expired token.".to_string()))
}

/// Axum extractor: pulls `Authorization: Bearer <jwt>` out of the request and
/// validates it. Any handler that takes `AuthUser` as a parameter requires a
/// valid, non-expired token — invalid/missing tokens short-circuit with 401
/// before the handler body ever runs.
pub struct AuthUser(pub Claims);

#[async_trait]
impl<S> FromRequestParts<S> for AuthUser
where
    AppState: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
                .await
                .map_err(|_| {
                    AppError::Unauthorized("Missing or malformed Authorization header.".to_string())
                })?;

        let app_state = AppState::from_ref(state);
        let claims = decode_token(&app_state.config.jwt_signing_secret, bearer.token())?;
        Ok(AuthUser(claims))
    }
}

/// Require the caller's role to be one of `allowed`. Used for the read-only
/// dashboard endpoints (audit log, fairness, drift, security events, command
/// center) which should only be visible to compliance/admin roles, not every
/// staff account that can submit a scan.
pub fn require_role(claims: &Claims, allowed: &[&str]) -> Result<(), AppError> {
    if allowed.contains(&claims.role.as_str()) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Role '{}' is not permitted to access this resource.",
            claims.role
        )))
    }
}
