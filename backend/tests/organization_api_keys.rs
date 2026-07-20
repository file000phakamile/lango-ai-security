//! Organisation OpenAI API key management (native chat feature, Phase 3)
//! integration tests. Same `#[sqlx::test]` pattern as every other file in
//! this directory. Run with `cargo test --test organization_api_keys`
//! (requires `DATABASE_URL`).

use axum::extract::{Query, State};
use axum::Json;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

use lango_backend::{
    auth::AuthUser,
    config::Config,
    crypto,
    models::{Claims, SetOpenAiKeyRequest},
    routes,
    state::AppState,
};

fn test_config() -> Config {
    Config {
        database_url: String::new(),
        jwt_signing_secret: "test-only-secret".to_string(),
        port: 0,
        cors_origin: "http://localhost".to_string(),
        api_key_encryption_key: "a".repeat(64),
        openai_api_base_url: "unused".to_string(),
    }
}

fn claims_for(user_id: Uuid, session_id: Uuid, role: &str, organisation_id: Uuid) -> Claims {
    Claims {
        sub: user_id,
        session_id,
        email: "user@key-test.test".to_string(),
        department: "General".to_string(),
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

async fn insert_user(pool: &PgPool, org_id: Uuid, email: &str, role: &str) -> (Uuid, Uuid) {
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, department, role, organisation_id, consent_accepted_at, consent_policy_version) \
         VALUES ($1, 'unused-hash', 'General', $2, $3, now(), 'v1') RETURNING id",
    )
    .bind(email)
    .bind(role)
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
    (user_id, session_id)
}

#[sqlx::test]
async fn a_compliance_admin_can_provision_a_key_and_only_sees_a_masked_confirmation(pool: PgPool) {
    let org = insert_org(&pool, "Key Test Org Provision").await;
    let (user_id, session_id) = insert_user(&pool, org, "admin@key-test.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user_id, session_id, "compliance_admin", org);

    let before = routes::organization_api_keys::get_status(State(state.clone()), AuthUser(claims.clone()))
        .await
        .unwrap()
        .0;
    assert!(!before.configured);

    let payload = SetOpenAiKeyRequest { api_key: "sk-test-real-looking-key-0000000000".to_string() };
    let after = routes::organization_api_keys::set_key(State(state.clone()), AuthUser(claims.clone()), Json(payload))
        .await
        .unwrap()
        .0;
    assert!(after.configured);
    assert_eq!(after.last_four, Some("0000".to_string()));
    assert!(after.created_at.is_some());
    assert!(after.rotated_at.is_none(), "a first-time provision must not set rotated_at");

    // The raw key is never stored in the clear anywhere.
    let stored: String = sqlx::query_scalar(
        "SELECT encrypted_key FROM organization_api_keys WHERE organisation_id = $1",
    )
    .bind(org)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!stored.contains("sk-test-real-looking-key"));
    let decrypted = crypto::decrypt_secret(&stored, &state.config.api_key_encryption_key).unwrap();
    assert_eq!(decrypted, "sk-test-real-looking-key-0000000000");
}

#[sqlx::test]
async fn rotating_replaces_the_key_and_sets_rotated_at_but_keeps_created_at(pool: PgPool) {
    let org = insert_org(&pool, "Key Test Org Rotate").await;
    let (user_id, session_id) = insert_user(&pool, org, "admin@key-test.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user_id, session_id, "compliance_admin", org);

    let first = routes::organization_api_keys::set_key(
        State(state.clone()),
        AuthUser(claims.clone()),
        Json(SetOpenAiKeyRequest { api_key: "sk-original-key-00000000000000000".to_string() }),
    )
    .await
    .unwrap()
    .0;
    let original_created_at = first.created_at.unwrap();

    let rotated = routes::organization_api_keys::set_key(
        State(state.clone()),
        AuthUser(claims.clone()),
        Json(SetOpenAiKeyRequest { api_key: "sk-rotated-key-000000000000000000".to_string() }),
    )
    .await
    .unwrap()
    .0;
    assert_eq!(rotated.created_at, Some(original_created_at), "created_at must not change on rotation");
    assert!(rotated.rotated_at.is_some(), "rotation must stamp rotated_at");
    assert_eq!(rotated.last_four, Some("0000".to_string()));

    // Still exactly one row for this organisation+provider — an upsert, not
    // a second row.
    let row_count: i64 = sqlx::query_scalar("SELECT count(*) FROM organization_api_keys WHERE organisation_id = $1")
        .bind(org)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row_count, 1);

    let stored: String = sqlx::query_scalar("SELECT encrypted_key FROM organization_api_keys WHERE organisation_id = $1")
        .bind(org)
        .fetch_one(&pool)
        .await
        .unwrap();
    let decrypted = crypto::decrypt_secret(&stored, &state.config.api_key_encryption_key).unwrap();
    assert_eq!(decrypted, "sk-rotated-key-000000000000000000", "the stored key must be the NEW one after rotation");
}

#[sqlx::test]
async fn a_malformed_key_is_rejected_before_being_stored(pool: PgPool) {
    let org = insert_org(&pool, "Key Test Org Malformed").await;
    let (user_id, session_id) = insert_user(&pool, org, "admin@key-test.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user_id, session_id, "compliance_admin", org);

    for bad_key in ["not-an-openai-key", "sk-short", "sk-has a space in it 000000000"] {
        let result = routes::organization_api_keys::set_key(
            State(state.clone()),
            AuthUser(claims.clone()),
            Json(SetOpenAiKeyRequest { api_key: bad_key.to_string() }),
        )
        .await;
        assert!(result.is_err(), "'{bad_key}' should have been rejected");
    }

    let row_count: i64 = sqlx::query_scalar("SELECT count(*) FROM organization_api_keys WHERE organisation_id = $1")
        .bind(org)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(row_count, 0, "no malformed key should ever reach the database");
}

#[sqlx::test]
async fn non_compliance_admin_roles_cannot_manage_or_view_the_key(pool: PgPool) {
    let org = insert_org(&pool, "Key Test Org RBAC").await;
    let (staff_id, staff_session) = insert_user(&pool, org, "staff@key-test.test", "staff").await;
    let (reviewer_id, reviewer_session) = insert_user(&pool, org, "reviewer@key-test.test", "department_reviewer").await;
    let state = AppState { db: pool.clone(), config: test_config() };

    for (user_id, session_id, role) in [
        (staff_id, staff_session, "staff"),
        (reviewer_id, reviewer_session, "department_reviewer"),
    ] {
        let claims = claims_for(user_id, session_id, role, org);
        let status_result = routes::organization_api_keys::get_status(State(state.clone()), AuthUser(claims.clone())).await;
        assert!(status_result.is_err(), "{role} must not be able to view key status");

        let set_result = routes::organization_api_keys::set_key(
            State(state.clone()),
            AuthUser(claims),
            Json(SetOpenAiKeyRequest { api_key: "sk-should-never-be-stored-00000000".to_string() }),
        )
        .await;
        assert!(set_result.is_err(), "{role} must not be able to set a key");
    }
}

#[sqlx::test]
async fn an_organisations_key_is_invisible_to_another_organisation(pool: PgPool) {
    let org_a = insert_org(&pool, "Key Test Org A").await;
    let org_b = insert_org(&pool, "Key Test Org B").await;
    let (admin_a, session_a) = insert_user(&pool, org_a, "admin@key-org-a.test", "compliance_admin").await;
    let (admin_b, session_b) = insert_user(&pool, org_b, "admin@key-org-b.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };

    let _ = routes::organization_api_keys::set_key(
        State(state.clone()),
        AuthUser(claims_for(admin_a, session_a, "compliance_admin", org_a)),
        Json(SetOpenAiKeyRequest { api_key: "sk-org-a-secret-key-000000000000000".to_string() }),
    )
    .await
    .unwrap();

    let org_b_status = routes::organization_api_keys::get_status(
        State(state),
        AuthUser(claims_for(admin_b, session_b, "compliance_admin", org_b)),
    )
    .await
    .unwrap()
    .0;
    assert!(!org_b_status.configured, "org B must never see org A's key as configured");
}

#[sqlx::test]
async fn usage_count_reflects_forwarded_chat_turns_only_not_blocked_ones(pool: PgPool) {
    let org = insert_org(&pool, "Key Test Org Usage").await;
    let (user_id, session_id) = insert_user(&pool, org, "admin@key-test.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user_id, session_id, "compliance_admin", org);

    async fn insert_audit_row(pool: &PgPool, org: Uuid, user_id: Uuid, session_id: Uuid, ai_model_used: &str) {
        sqlx::query(
            r#"
            INSERT INTO audit_log (
                session_id, user_id, department, "timestamp", entities_detected, risk_score,
                decision, reason_string, ai_model_used, response_scan_result,
                original_prompt_hash, organisation_id
            )
            VALUES ($1, $2, 'General', now(), '[]'::jsonb, 0.1, 'cleared_no_entities', 'test', $3, 'test', 'testhash', $4)
            "#,
        )
        .bind(session_id)
        .bind(user_id)
        .bind(ai_model_used)
        .bind(org)
        .execute(pool)
        .await
        .expect("insert audit_log row");
    }

    insert_audit_row(&pool, org, user_id, session_id, "openai:gpt-4o-mini").await;
    insert_audit_row(&pool, org, user_id, session_id, "openai:gpt-4o-mini").await;
    insert_audit_row(&pool, org, user_id, session_id, "not applicable - blocked pre-gateway, no provider called").await;

    let mut params = HashMap::new();
    params.insert("days".to_string(), "30".to_string());
    let usage = routes::organization_api_keys::get_usage(State(state), AuthUser(claims), Query(params))
        .await
        .unwrap()
        .0;
    assert_eq!(usage.request_count, 2, "only the two forwarded (real OpenAI) turns should count, not the blocked one");
}

#[sqlx::test]
async fn an_invalid_usage_window_is_rejected(pool: PgPool) {
    let org = insert_org(&pool, "Key Test Org Usage Window").await;
    let (user_id, session_id) = insert_user(&pool, org, "admin@key-test.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(user_id, session_id, "compliance_admin", org);

    let mut params = HashMap::new();
    params.insert("days".to_string(), "999".to_string());
    let result = routes::organization_api_keys::get_usage(State(state), AuthUser(claims), Query(params)).await;
    assert!(result.is_err());
}
