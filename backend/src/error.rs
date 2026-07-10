use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// Every error response from this API has the same JSON shape:
/// `{ "error": { "code": "SOME_CODE", "message": "human readable" } }`
/// so the frontend can handle failures generically instead of per-endpoint.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("password hashing error: {0}")]
    Hash(String),
    #[error("token error: {0}")]
    Token(#[from] jsonwebtoken::errors::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: String,
}

impl AppError {
    fn code_and_status(&self) -> (&'static str, StatusCode) {
        match self {
            AppError::BadRequest(_) => ("BAD_REQUEST", StatusCode::BAD_REQUEST),
            AppError::Unauthorized(_) => ("UNAUTHORIZED", StatusCode::UNAUTHORIZED),
            AppError::Forbidden(_) => ("FORBIDDEN", StatusCode::FORBIDDEN),
            AppError::NotFound(_) => ("NOT_FOUND", StatusCode::NOT_FOUND),
            AppError::Database(_) => ("DATABASE_ERROR", StatusCode::INTERNAL_SERVER_ERROR),
            AppError::Hash(_) => ("HASH_ERROR", StatusCode::INTERNAL_SERVER_ERROR),
            AppError::Token(_) => ("TOKEN_ERROR", StatusCode::UNAUTHORIZED),
            AppError::Internal(_) => ("INTERNAL_ERROR", StatusCode::INTERNAL_SERVER_ERROR),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (code, status) = self.code_and_status();
        // Log the real error server-side; don't leak internals (e.g. raw SQL
        // error text) to the client for 5xx-class failures.
        if status.is_server_error() {
            tracing::error!(error = %self, "request failed");
        }
        let message = match &self {
            AppError::Database(_) => "An internal error occurred.".to_string(),
            AppError::Hash(_) => "An internal error occurred.".to_string(),
            AppError::Internal(_) => "An internal error occurred.".to_string(),
            other => other.to_string(),
        };
        let body = ErrorBody {
            error: ErrorDetail { code, message },
        };
        (status, Json(body)).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
