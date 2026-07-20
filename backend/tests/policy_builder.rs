//! Policy builder (product-depth task, Part 1) integration tests.
//!
//! The task explicitly asked to "test the API directly, do not just trust
//! the UI to prevent it" for the threshold safe-bounds enforcement — this
//! file is that direct API test, calling `routes::policy` handlers exactly
//! the way `multi_tenant_isolation.rs` calls every other route handler (no
//! HTTP server, real Postgres, real constraints). Also covers custom-pattern
//! validation and cross-tenant isolation of both settings and patterns, plus
//! an end-to-end proof that `/api/scan` actually applies an organisation's
//! configured threshold and custom pattern, not just that the settings
//! endpoints accept and echo them back.
//!
//! Requires a real Postgres reachable via `DATABASE_URL` — same requirement
//! as every other file in this directory. Run with
//! `cargo test --test policy_builder`.

use axum::extract::{Path, State};
use axum::Json;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use lango_backend::{
    auth::AuthUser,
    config::Config,
    detection::scan,
    models::{Claims, CreateCustomPatternRequest, ScanRequest, UpdateThresholdRequest},
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

struct SeededUser {
    user_id: Uuid,
    session_id: Uuid,
}

async fn insert_user(pool: &PgPool, org_id: Uuid, email: &str, role: &str) -> SeededUser {
    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (email, password_hash, department, role, organisation_id, consent_accepted_at, consent_policy_version) \
         VALUES ($1, 'unused-hash', 'Credit Risk', $2, $3, now(), 'v1') RETURNING id",
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

    SeededUser { user_id, session_id }
}

// --- Threshold bounds: the actual, direct-API safe-bounds test -------------

#[sqlx::test]
async fn compliance_admin_can_set_threshold_within_bounds(pool: PgPool) {
    let org = insert_org(&pool, "Policy Bounds Org").await;
    let admin = insert_user(&pool, org, "admin@policy.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let response = routes::policy::update_threshold(
        State(state.clone()),
        AuthUser(claims.clone()),
        Json(UpdateThresholdRequest { confidence_threshold: 0.75 }),
    )
    .await
    .expect("a within-bounds threshold must be accepted")
    .0;
    assert_eq!(response.confidence_threshold, 0.75);

    let refetched = routes::policy::get_settings(State(state), AuthUser(claims))
        .await
        .expect("get_settings should succeed")
        .0;
    assert_eq!(refetched.confidence_threshold, 0.75, "the new value must persist and be reflected on re-fetch");
}

#[sqlx::test]
async fn compliance_admin_cannot_set_threshold_below_the_safe_floor(pool: PgPool) {
    // THE direct-API test the task asked for: an attempt to set the
    // threshold below MIN_ORG_CONFIDENCE_THRESHOLD must be REJECTED by the
    // API itself — not merely discouraged by the dashboard UI.
    let org = insert_org(&pool, "Policy Floor Org").await;
    let admin = insert_user(&pool, org, "admin@floor.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let result = routes::policy::update_threshold(
        State(state.clone()),
        AuthUser(claims.clone()),
        Json(UpdateThresholdRequest { confidence_threshold: 0.05 }),
    )
    .await;
    assert!(result.is_err(), "a threshold near zero must be rejected by the API, not silently clamped or accepted");

    // And the rejection must not have partially applied — the organisation
    // is still on the system default, not a half-written 0.05.
    let settings = routes::policy::get_settings(State(state), AuthUser(claims))
        .await
        .expect("get_settings should succeed")
        .0;
    assert_eq!(settings.confidence_threshold, scan::CONFIDENCE_THRESHOLD, "a rejected update must not have been persisted");
}

#[sqlx::test]
async fn compliance_admin_cannot_set_threshold_above_the_safe_ceiling(pool: PgPool) {
    let org = insert_org(&pool, "Policy Ceiling Org").await;
    let admin = insert_user(&pool, org, "admin@ceiling.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let result = routes::policy::update_threshold(
        State(state),
        AuthUser(claims),
        Json(UpdateThresholdRequest { confidence_threshold: 1.0 }),
    )
    .await;
    assert!(result.is_err(), "a threshold of 1.0 (blocks everything) must be rejected by the API");
}

#[sqlx::test]
async fn exactly_the_documented_min_and_max_bounds_are_accepted_not_rejected(pool: PgPool) {
    // Boundary-inclusive check: MIN and MAX themselves are valid values, not
    // off-by-one excluded — the task says "never below"/bounds are the
    // actual safe limits, not open intervals.
    let org = insert_org(&pool, "Policy Boundary Org").await;
    let admin = insert_user(&pool, org, "admin@boundary.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    for bound in [scan::MIN_ORG_CONFIDENCE_THRESHOLD, scan::MAX_ORG_CONFIDENCE_THRESHOLD] {
        let result = routes::policy::update_threshold(
            State(state.clone()),
            AuthUser(claims.clone()),
            Json(UpdateThresholdRequest { confidence_threshold: bound }),
        )
        .await;
        assert!(result.is_ok(), "the documented bound {bound} itself must be accepted, not treated as out-of-range");
    }
}

#[sqlx::test]
async fn non_compliance_admin_roles_are_forbidden_from_policy_endpoints(pool: PgPool) {
    let org = insert_org(&pool, "Policy RBAC Org").await;
    let reviewer = insert_user(&pool, org, "reviewer@rbac.test", "department_reviewer").await;
    let staff = insert_user(&pool, org, "staff@rbac.test", "staff").await;
    let state = AppState { db: pool.clone(), config: test_config() };

    for user in [reviewer, staff] {
        let claims = claims_for(user.user_id, user.session_id, "irrelevant", org);
        assert!(
            routes::policy::get_settings(State(state.clone()), AuthUser(claims.clone())).await.is_err(),
            "only compliance_admin may read policy settings"
        );
        assert!(
            routes::policy::update_threshold(
                State(state.clone()),
                AuthUser(claims),
                Json(UpdateThresholdRequest { confidence_threshold: 0.70 }),
            )
            .await
            .is_err(),
            "only compliance_admin may change the threshold"
        );
    }
}

// --- Custom patterns: validation + cross-tenant isolation ------------------

#[sqlx::test]
async fn custom_pattern_with_invalid_regex_is_rejected(pool: PgPool) {
    let org = insert_org(&pool, "Pattern Validation Org").await;
    let admin = insert_user(&pool, org, "admin@pattern.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let result = routes::policy::create_custom_pattern(
        State(state),
        AuthUser(claims),
        Json(CreateCustomPatternRequest {
            entity_label: "acme_broken".to_string(),
            pattern: "[unterminated".to_string(),
            confidence: None,
        }),
    )
    .await;
    assert!(result.is_err(), "an invalid regex must be rejected, not stored");
}

#[sqlx::test]
async fn custom_pattern_cannot_reuse_a_built_in_entity_label(pool: PgPool) {
    let org = insert_org(&pool, "Pattern Reserved Org").await;
    let admin = insert_user(&pool, org, "admin@reserved.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let result = routes::policy::create_custom_pattern(
        State(state),
        AuthUser(claims),
        Json(CreateCustomPatternRequest {
            entity_label: "national_id".to_string(),
            pattern: r"\d{9}".to_string(),
            confidence: None,
        }),
    )
    .await;
    assert!(result.is_err(), "a custom pattern must not be allowed to reuse a built-in entity type name");
}

#[sqlx::test]
async fn valid_custom_pattern_is_created_and_listed(pool: PgPool) {
    let org = insert_org(&pool, "Pattern Success Org").await;
    let admin = insert_user(&pool, org, "admin@success.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let response = routes::policy::create_custom_pattern(
        State(state),
        AuthUser(claims),
        Json(CreateCustomPatternRequest {
            entity_label: "acme_account_format".to_string(),
            pattern: r"ACME-\d{8}".to_string(),
            confidence: Some(0.85),
        }),
    )
    .await
    .expect("a valid custom pattern must be accepted")
    .0;
    assert_eq!(response.custom_patterns.len(), 1);
    assert_eq!(response.custom_patterns[0].entity_label, "acme_account_format");
    assert_eq!(response.custom_patterns[0].confidence, 0.85);
}

#[sqlx::test]
async fn an_organisations_custom_pattern_is_invisible_and_undeletable_by_another_organisation(pool: PgPool) {
    let org_a = insert_org(&pool, "Pattern Isolation Org A").await;
    let org_b = insert_org(&pool, "Pattern Isolation Org B").await;
    let admin_a = insert_user(&pool, org_a, "admin@iso-a.test", "compliance_admin").await;
    let admin_b = insert_user(&pool, org_b, "admin@iso-b.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims_a = claims_for(admin_a.user_id, admin_a.session_id, "compliance_admin", org_a);
    let claims_b = claims_for(admin_b.user_id, admin_b.session_id, "compliance_admin", org_b);

    let created = routes::policy::create_custom_pattern(
        State(state.clone()),
        AuthUser(claims_a.clone()),
        Json(CreateCustomPatternRequest {
            entity_label: "org_a_only_pattern".to_string(),
            pattern: r"AONLY-\d{6}".to_string(),
            confidence: None,
        }),
    )
    .await
    .expect("org A's pattern creation must succeed")
    .0;
    let pattern_id = created.custom_patterns[0].id;

    // Org B must not see org A's pattern in its own settings view.
    let org_b_settings = routes::policy::get_settings(State(state.clone()), AuthUser(claims_b.clone()))
        .await
        .expect("get_settings should succeed for org B")
        .0;
    assert!(
        org_b_settings.custom_patterns.iter().all(|p| p.id != pattern_id),
        "org A's custom pattern must never appear in org B's settings"
    );

    // Org B must not be able to delete org A's pattern by guessing its id.
    let delete_result =
        routes::policy::delete_custom_pattern(State(state.clone()), AuthUser(claims_b), Path(pattern_id)).await;
    assert!(delete_result.is_err(), "org B must not be able to delete org A's custom pattern");

    // It must still exist for org A afterward.
    let org_a_settings_after = routes::policy::get_settings(State(state), AuthUser(claims_a))
        .await
        .expect("get_settings should succeed for org A")
        .0;
    assert!(org_a_settings_after.custom_patterns.iter().any(|p| p.id == pattern_id));
}

// --- End-to-end: /api/scan actually applies the organisation's policy ------

#[sqlx::test]
async fn scan_endpoint_applies_the_organisations_configured_threshold(pool: PgPool) {
    // bank_account's built-in pattern is fixed at 0.50 confidence — below
    // the system default (0.60), so it blocks by default (see
    // detection::scan's own unit test coverage). An org that has lowered
    // its OWN threshold to the safe floor (0.50) via the policy-builder API
    // must see the live /api/scan endpoint actually honor that — not just
    // the settings endpoint echoing the value back.
    let org = insert_org(&pool, "Scan Policy Org").await;
    let admin = insert_user(&pool, org, "admin@scanpolicy.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let admin_claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let _ = routes::policy::update_threshold(
        State(state.clone()),
        AuthUser(admin_claims),
        Json(UpdateThresholdRequest { confidence_threshold: scan::MIN_ORG_CONFIDENCE_THRESHOLD }),
    )
    .await
    .expect("lowering to the safe floor must succeed");

    let scan_claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);
    let response = routes::scan::scan(
        State(state),
        AuthUser(scan_claims),
        Json(ScanRequest {
            prompt: "Please refund via account 9988776655443 once approved.".to_string(),
            language: None,
            facility_type: None,
        }),
    )
    .await
    .expect("scan should succeed")
    .0;
    assert_eq!(
        response.decision, "redacted_and_forwarded",
        "with the org threshold lowered to the safe floor, a 0.50-confidence bank_account match must now clear instead of blocking"
    );
}

#[sqlx::test]
async fn scan_endpoint_applies_the_organisations_custom_pattern(pool: PgPool) {
    let org = insert_org(&pool, "Scan Custom Pattern Org").await;
    let admin = insert_user(&pool, org, "admin@scanpattern.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let admin_claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let _ = routes::policy::create_custom_pattern(
        State(state.clone()),
        AuthUser(admin_claims),
        Json(CreateCustomPatternRequest {
            entity_label: "acme_account_format".to_string(),
            pattern: r"ACME-\d{8}".to_string(),
            confidence: Some(0.90),
        }),
    )
    .await
    .expect("custom pattern creation must succeed");

    let scan_claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);
    let response = routes::scan::scan(
        State(state.clone()),
        AuthUser(scan_claims),
        Json(ScanRequest {
            prompt: "Please close account ACME-12345678 today.".to_string(),
            language: None,
            facility_type: None,
        }),
    )
    .await
    .expect("scan should succeed")
    .0;
    assert!(response.entities_detected.contains(&"acme_account_format".to_string()));
    assert!(!response.redacted_prompt.contains("ACME-12345678"));

    // A DIFFERENT organisation, with no such pattern registered, must not
    // have this custom detector applied to its own scans.
    let other_org = insert_org(&pool, "Unrelated Org").await;
    let other_admin = insert_user(&pool, other_org, "admin@unrelated.test", "compliance_admin").await;
    let other_claims = claims_for(other_admin.user_id, other_admin.session_id, "compliance_admin", other_org);
    let other_response = routes::scan::scan(
        State(state),
        AuthUser(other_claims),
        Json(ScanRequest {
            prompt: "Please close account ACME-12345678 today.".to_string(),
            language: None,
            facility_type: None,
        }),
    )
    .await
    .expect("scan should succeed")
    .0;
    assert!(
        !other_response.entities_detected.contains(&"acme_account_format".to_string()),
        "an organisation-specific custom pattern must never apply to a different organisation's scans"
    );
}
