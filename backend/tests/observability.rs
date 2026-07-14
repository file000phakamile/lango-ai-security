//! Real observability ("response scanning + observability + hardening"
//! task, Part 2) integration tests. Unlike every other file in this
//! directory (which call route handlers as plain async functions), this
//! one needs a real `Router` with the real middleware layered on, since
//! `error_log_middleware` only does anything by wrapping request
//! dispatch — dispatched via `tower::ServiceExt::oneshot` to exercise it
//! for real, not by calling the middleware function directly. Run with
//! `cargo test --test observability` (requires `DATABASE_URL`).

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

use lango_backend::{
    auth::AuthUser,
    config::Config,
    error::{AppError, AppResult},
    models::Claims,
    observability::error_log_middleware,
    routes,
    state::AppState,
};

fn test_config() -> Config {
    Config {
        database_url: String::new(),
        jwt_signing_secret: "test-only-secret".to_string(),
        port: 0,
        cors_origin: "http://localhost".to_string(),
    }
}

fn claims_for(user_id: Uuid, session_id: Uuid, role: &str, organisation_id: Uuid) -> Claims {
    Claims {
        sub: user_id,
        session_id,
        email: format!("{role}@test"),
        department: "Credit Risk".to_string(),
        role: role.to_string(),
        organisation_id,
        exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
    }
}

async fn insert_org(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (name) VALUES ($1) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("insert organisation")
}

async fn insert_user(pool: &PgPool, org_id: Uuid, email: &str, role: &str) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, department, role, organisation_id, consent_accepted_at, consent_policy_version) \
         VALUES ($1, 'unused-hash', 'Credit Risk', $2, $3, now(), 'v1') RETURNING id",
    )
    .bind(email)
    .bind(role)
    .bind(org_id)
    .fetch_one(pool)
    .await
    .expect("insert user")
}

/// Always fails with a real, deliberate `AppError::Internal` — a
/// synthetic-but-real 500, exercised through the actual `AppError ->
/// Response` conversion the real backend uses, so this test verifies the
/// middleware's real behavior rather than a mock of it.
async fn always_500() -> AppResult<Json<serde_json::Value>> {
    Err(AppError::Internal("synthetic failure for observability test".to_string()))
}

#[sqlx::test]
async fn a_real_500_response_is_recorded_in_the_backend_errors_table(pool: PgPool) {
    let state = AppState { db: pool.clone(), config: test_config() };

    let app = Router::new()
        .route("/test/boom", get(always_500))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(state, error_log_middleware));

    let request = axum::http::Request::builder()
        .uri("/test/boom")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 500);

    // The middleware's DB write is spawned, not awaited inline (see its own
    // doc comment on why) — give it a moment to actually land before
    // querying for it.
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let (status_code, message): (i16, Option<String>) =
        sqlx::query_as("SELECT status_code, message FROM backend_errors WHERE path = $1 ORDER BY created_at DESC LIMIT 1")
            .bind("/test/boom")
            .fetch_one(&pool)
            .await
            .expect("a row must have been written for the 500 response");
    assert_eq!(status_code, 500);
    // Sanitized message only — never the raw internal error text passed to
    // AppError::Internal above ("synthetic failure for observability test"
    // must NOT appear verbatim; see error.rs's own message-sanitizing match).
    assert_eq!(message.as_deref(), Some("An internal error occurred."));
}

#[sqlx::test]
async fn a_successful_response_is_not_recorded(pool: PgPool) {
    async fn always_ok() -> Json<serde_json::Value> {
        Json(serde_json::json!({"ok": true}))
    }

    let state = AppState { db: pool.clone(), config: test_config() };
    let app = Router::new()
        .route("/test/ok", get(always_ok))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(state, error_log_middleware));

    let request = axum::http::Request::builder()
        .uri("/test/ok")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 200);

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backend_errors WHERE path = $1")
        .bind("/test/ok")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "a successful (2xx) response must never be recorded as a backend error");
}

#[sqlx::test]
async fn a_client_error_like_bad_request_is_not_recorded_as_a_backend_error(pool: PgPool) {
    async fn always_400() -> AppResult<Json<serde_json::Value>> {
        Err(AppError::BadRequest("deliberately bad request for this test".to_string()))
    }

    let state = AppState { db: pool.clone(), config: test_config() };
    let app = Router::new()
        .route("/test/bad", get(always_400))
        .with_state(state.clone())
        .layer(axum::middleware::from_fn_with_state(state, error_log_middleware));

    let request = axum::http::Request::builder()
        .uri("/test/bad")
        .body(axum::body::Body::empty())
        .unwrap();
    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), 400);

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backend_errors WHERE path = $1")
        .bind("/test/bad")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 0, "a 4xx client error is a user mistake, not a backend error — must not be logged here");
}

#[sqlx::test]
async fn only_compliance_admin_can_read_the_backend_errors_endpoint(pool: PgPool) {
    let org = insert_org(&pool, "Backend Errors RBAC Org").await;
    let staff_id = insert_user(&pool, org, "staff@errors.test", "staff").await;
    let admin_id = insert_user(&pool, org, "admin@errors.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };

    let staff_claims = claims_for(staff_id, Uuid::new_v4(), "staff", org);
    let staff_result = routes::backend_errors::get_backend_errors(State(state.clone()), AuthUser(staff_claims)).await;
    assert!(staff_result.is_err(), "staff must not be able to read the backend error log");

    let admin_claims = claims_for(admin_id, Uuid::new_v4(), "compliance_admin", org);
    let admin_result = routes::backend_errors::get_backend_errors(State(state), AuthUser(admin_claims)).await;
    assert!(admin_result.is_ok(), "compliance_admin must be able to read the backend error log");
}
