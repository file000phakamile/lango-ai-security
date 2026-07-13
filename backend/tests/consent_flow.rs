//! Consent-gate integration tests (Part 4 of the multi-tenancy task). Real
//! Postgres, via `#[sqlx::test]` — same approach as
//! `tests/multi_tenant_isolation.rs`, see that file's own doc comment for
//! why this needs a real database and how to run it.

use axum::extract::State;
use axum::Json;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use lango_backend::{
    auth::AuthUser,
    config::Config,
    models::{ConsentAcceptRequest, Claims, ScanRequest},
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
        email: "newuser@org.test".to_string(),
        department: "Credit Risk".to_string(),
        role: "staff".to_string(),
        organisation_id,
        exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
    }
}

#[sqlx::test]
async fn a_brand_new_unconsented_user_is_blocked_from_scanning_until_they_accept(pool: PgPool) {
    let org_id: Uuid = sqlx::query_scalar("INSERT INTO organisations (name) VALUES ('Consent Test Org') RETURNING id")
        .fetch_one(&pool)
        .await
        .expect("insert organisation");

    // Deliberately NOT setting consent_accepted_at — this is the "brand
    // new user, never consented" case the whole gate exists for.
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, department, role, organisation_id) \
         VALUES ('newuser@org.test', 'unused-hash', 'Credit Risk', 'staff', $1) RETURNING id",
    )
    .bind(org_id)
    .fetch_one(&pool)
    .await
    .expect("insert user");
    let session_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions (user_id, expires_at) VALUES ($1, now() + interval '1 day') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("insert session");

    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user_id, session_id, org_id);

    // 1. Scanning before consent must fail closed with CONSENT_REQUIRED,
    // not silently proceed and not a generic error.
    let scan_request = ScanRequest {
        prompt: "What is the capital of Zimbabwe?".to_string(),
        language: None,
        facility_type: None,
    };
    let err = routes::scan::scan(State(state.clone()), AuthUser(claims.clone()), Json(scan_request))
        .await
        .expect_err("scan must be blocked before consent is accepted");
    assert!(
        matches!(err, lango_backend::error::AppError::ConsentRequired(_)),
        "expected ConsentRequired, got: {err:?}"
    );

    // No audit_log row should have been written for the blocked attempt.
    let row_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("count audit_log rows");
    assert_eq!(row_count, 0, "a consent-blocked scan must not write an audit_log row at all");

    // 2. Accept consent with the correct current policy version ('v1', the
    // organisations table's default).
    let accept_response = routes::consent::accept_consent(
        State(state.clone()),
        AuthUser(claims.clone()),
        Json(ConsentAcceptRequest { policy_version: "v1".to_string() }),
    )
    .await
    .expect("accepting the current policy version should succeed")
    .0;
    assert!(accept_response.accepted);
    assert_eq!(accept_response.policy_version, "v1");

    // 3. The SAME user can now scan successfully.
    let scan_request_after_consent = ScanRequest {
        prompt: "What is the capital of Zimbabwe?".to_string(),
        language: None,
        facility_type: None,
    };
    let scan_result = routes::scan::scan(State(state.clone()), AuthUser(claims), Json(scan_request_after_consent))
        .await
        .expect("scan must succeed after consent is accepted")
        .0;
    assert_eq!(scan_result.decision, "cleared_no_entities");

    let row_count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .expect("count audit_log rows");
    assert_eq!(row_count_after, 1, "the post-consent scan must have written exactly one audit_log row");
}

#[sqlx::test]
async fn accepting_a_stale_policy_version_is_rejected(pool: PgPool) {
    let org_id: Uuid = sqlx::query_scalar("INSERT INTO organisations (name) VALUES ('Stale Consent Test Org') RETURNING id")
        .fetch_one(&pool)
        .await
        .expect("insert organisation");
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, department, role, organisation_id) \
         VALUES ('newuser@org.test', 'unused-hash', 'Credit Risk', 'staff', $1) RETURNING id",
    )
    .bind(org_id)
    .fetch_one(&pool)
    .await
    .expect("insert user");
    let session_id: Uuid = sqlx::query_scalar(
        "INSERT INTO sessions (user_id, expires_at) VALUES ($1, now() + interval '1 day') RETURNING id",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .expect("insert session");

    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user_id, session_id, org_id);

    let err = routes::consent::accept_consent(
        State(state),
        AuthUser(claims),
        Json(ConsentAcceptRequest { policy_version: "v0-stale".to_string() }),
    )
    .await
    .expect_err("accepting a version that doesn't match the org's current version must be rejected");
    assert!(matches!(err, lango_backend::error::AppError::BadRequest(_)));
}
