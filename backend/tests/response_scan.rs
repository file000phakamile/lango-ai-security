//! Response scanning ("response scanning + observability + hardening"
//! task, Part 1) integration tests. Same `#[sqlx::test]` pattern as the
//! other files in this directory. Run with
//! `cargo test --test response_scan` (requires `DATABASE_URL`).

use axum::extract::State;
use axum::Json;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use lango_backend::{
    auth::AuthUser,
    config::Config,
    models::{Claims, ScanRequest, ScanResponseCheckRequest},
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

fn claims_for(user_id: Uuid, session_id: Uuid, organisation_id: Uuid) -> Claims {
    Claims {
        sub: user_id,
        session_id,
        email: "staff@test".to_string(),
        department: "Credit Risk".to_string(),
        role: "staff".to_string(),
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

struct SeededUser {
    user_id: Uuid,
    session_id: Uuid,
}

async fn insert_user(pool: &PgPool, org_id: Uuid, email: &str) -> SeededUser {
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, department, role, organisation_id, consent_accepted_at, consent_policy_version) \
         VALUES ($1, 'unused-hash', 'Credit Risk', 'staff', $2, now(), 'v1') RETURNING id",
    )
    .bind(email)
    .bind(org_id)
    .fetch_one(pool)
    .await
    .expect("insert user");

    let session_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions (user_id, expires_at) VALUES ($1, now() + interval '1 day') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .expect("insert session");

    SeededUser { user_id, session_id }
}

#[sqlx::test]
async fn a_clean_prompt_response_round_trip_leaves_response_not_flagged(pool: PgPool) {
    let org = insert_org(&pool, "Response Scan Org").await;
    let user = insert_user(&pool, org, "staff@response.test").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user.user_id, user.session_id, org);

    let scan_result = routes::scan::scan(
        State(state.clone()),
        AuthUser(claims.clone()),
        Json(ScanRequest { prompt: "What is the capital of Zimbabwe?".to_string(), language: None, facility_type: None }),
    )
    .await
    .expect("prompt scan should succeed")
    .0;
    assert_eq!(scan_result.decision, "cleared_no_entities");

    let response_result = routes::response_scan::scan_response_handler(
        State(state),
        AuthUser(claims),
        Json(ScanResponseCheckRequest {
            audit_log_id: scan_result.id,
            response_text: "The capital of Zimbabwe is Harare.".to_string(),
        }),
    )
    .await
    .expect("response scan should succeed")
    .0;
    assert!(!response_result.flagged);
    assert!(response_result.entities_detected.is_empty());
}

#[sqlx::test]
async fn a_response_leaking_a_national_id_is_flagged_and_recorded(pool: PgPool) {
    let org = insert_org(&pool, "Response Scan Leak Org").await;
    let user = insert_user(&pool, org, "staff@leak.test").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user.user_id, user.session_id, org);

    let scan_result = routes::scan::scan(
        State(state.clone()),
        AuthUser(claims.clone()),
        Json(ScanRequest { prompt: "Summarize this account for me.".to_string(), language: None, facility_type: None }),
    )
    .await
    .expect("prompt scan should succeed")
    .0;

    let response_result = routes::response_scan::scan_response_handler(
        State(state.clone()),
        AuthUser(claims),
        Json(ScanResponseCheckRequest {
            audit_log_id: scan_result.id,
            response_text: "Sure, the account holder's national ID is 63-123456A23.".to_string(),
        }),
    )
    .await
    .expect("response scan should succeed")
    .0;
    assert!(response_result.flagged);
    assert!(response_result.entities_detected.contains(&"national_id".to_string()));

    // Confirm the audit_log row was actually updated, not just the HTTP
    // response returned — the whole point of this endpoint is a durable
    // compliance record, not a stateless check.
    let (flagged, scanned_at, result_text): (Option<bool>, Option<chrono::DateTime<Utc>>, String) = sqlx::query_as(
        "SELECT response_flagged, response_scanned_at, response_scan_result FROM audit_log WHERE id = $1",
    )
    .bind(scan_result.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(flagged, Some(true));
    assert!(scanned_at.is_some());
    assert!(result_text.to_lowercase().contains("national id"));
}

#[sqlx::test]
async fn a_second_response_scan_on_the_same_row_is_rejected(pool: PgPool) {
    let org = insert_org(&pool, "Response Scan Duplicate Org").await;
    let user = insert_user(&pool, org, "staff@dup.test").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user.user_id, user.session_id, org);

    let scan_result = routes::scan::scan(
        State(state.clone()),
        AuthUser(claims.clone()),
        Json(ScanRequest { prompt: "Hello".to_string(), language: None, facility_type: None }),
    )
    .await
    .expect("prompt scan should succeed")
    .0;

    let _ = routes::response_scan::scan_response_handler(
        State(state.clone()),
        AuthUser(claims.clone()),
        Json(ScanResponseCheckRequest { audit_log_id: scan_result.id, response_text: "Hi there!".to_string() }),
    )
    .await
    .expect("first response scan should succeed");

    let second = routes::response_scan::scan_response_handler(
        State(state),
        AuthUser(claims),
        Json(ScanResponseCheckRequest { audit_log_id: scan_result.id, response_text: "Hi again!".to_string() }),
    )
    .await;
    assert!(second.is_err(), "a second response scan on the same row must be rejected");
}

#[sqlx::test]
async fn a_response_scan_for_a_blocked_prompt_is_rejected(pool: PgPool) {
    let org = insert_org(&pool, "Response Scan Blocked Org").await;
    let user = insert_user(&pool, org, "staff@blocked.test").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user.user_id, user.session_id, org);

    // bank_account's primary pattern is 0.50 confidence — below the system
    // default threshold (0.60) — blocks with nothing sent.
    let scan_result = routes::scan::scan(
        State(state.clone()),
        AuthUser(claims.clone()),
        Json(ScanRequest {
            prompt: "Please refund via account 9988776655443 once approved.".to_string(),
            language: None,
            facility_type: None,
        }),
    )
    .await
    .expect("prompt scan should succeed")
    .0;
    assert_eq!(scan_result.decision, "blocked_low_confidence");

    let result = routes::response_scan::scan_response_handler(
        State(state),
        AuthUser(claims),
        Json(ScanResponseCheckRequest {
            audit_log_id: scan_result.id,
            response_text: "This should never be reachable.".to_string(),
        }),
    )
    .await;
    assert!(result.is_err(), "a blocked prompt has nothing to have a response scanned for");
}

#[sqlx::test]
async fn a_user_cannot_attach_a_response_scan_to_another_users_audit_log_row(pool: PgPool) {
    let org = insert_org(&pool, "Response Scan Isolation Org").await;
    let user_a = insert_user(&pool, org, "staff-a@iso.test").await;
    let user_b = insert_user(&pool, org, "staff-b@iso.test").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims_a = claims_for(user_a.user_id, user_a.session_id, org);
    let claims_b = claims_for(user_b.user_id, user_b.session_id, org);

    let scan_result = routes::scan::scan(
        State(state.clone()),
        AuthUser(claims_a),
        Json(ScanRequest { prompt: "Hello".to_string(), language: None, facility_type: None }),
    )
    .await
    .expect("prompt scan should succeed")
    .0;

    let result = routes::response_scan::scan_response_handler(
        State(state),
        AuthUser(claims_b),
        Json(ScanResponseCheckRequest { audit_log_id: scan_result.id, response_text: "Hi there!".to_string() }),
    )
    .await;
    assert!(result.is_err(), "user B must not be able to attach a response scan to user A's audit_log row");
}
