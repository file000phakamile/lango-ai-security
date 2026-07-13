//! Self-service organisation signup integration tests (Part 5 of the
//! multi-tenancy task). Real Postgres via `#[sqlx::test]` — see
//! `tests/multi_tenant_isolation.rs`'s doc comment for how to run these.

use axum::extract::State;
use axum::Json;
use sqlx::PgPool;

use lango_backend::{config::Config, models::OrganisationSignupRequest, routes, state::AppState};

fn test_config() -> Config {
    Config {
        database_url: String::new(),
        jwt_signing_secret: "test-only-secret".to_string(),
        port: 0,
        cors_origin: "http://localhost".to_string(),
    }
}

#[sqlx::test]
async fn signup_creates_a_working_compliance_admin_account_requiring_consent(pool: PgPool) {
    let state = AppState { db: pool.clone(), config: test_config() };

    let response = routes::organisations::signup(
        State(state),
        Json(OrganisationSignupRequest {
            organisation_name: "Brand New Bank".to_string(),
            email: "first-admin@brand-new-bank.test".to_string(),
            password: "a-reasonably-long-password".to_string(),
        }),
    )
    .await
    .expect("signup should succeed for a genuinely new organisation/email")
    .0;

    assert!(!response.token.is_empty());
    assert_eq!(response.user.role, "compliance_admin");
    assert_eq!(response.user.email, "first-admin@brand-new-bank.test");
    assert!(response.requires_consent, "a brand new organisation's first user must not be pre-consented");
    assert_eq!(response.consent_policy_version, "v1");

    // The organisation and user rows must actually exist, correctly linked.
    let org_id: uuid::Uuid = sqlx::query_scalar("SELECT id FROM organisations WHERE name = 'Brand New Bank'")
        .fetch_one(&pool)
        .await
        .expect("organisation row must exist");
    assert_eq!(org_id, response.user.organisation_id);

    let stored_role: String =
        sqlx::query_scalar("SELECT role FROM users WHERE email = 'first-admin@brand-new-bank.test'")
            .fetch_one(&pool)
            .await
            .expect("user row must exist");
    assert_eq!(stored_role, "compliance_admin");
}

#[sqlx::test]
async fn signup_rejects_a_duplicate_organisation_name(pool: PgPool) {
    let state = AppState { db: pool.clone(), config: test_config() };

    let first = OrganisationSignupRequest {
        organisation_name: "Duplicate Bank".to_string(),
        email: "admin1@duplicate-bank.test".to_string(),
        password: "a-reasonably-long-password".to_string(),
    };
    let _ = routes::organisations::signup(State(state.clone()), Json(first))
        .await
        .expect("first signup should succeed");

    let second = OrganisationSignupRequest {
        organisation_name: "Duplicate Bank".to_string(),
        email: "admin2@duplicate-bank.test".to_string(),
        password: "a-reasonably-long-password".to_string(),
    };
    let err = routes::organisations::signup(State(state), Json(second))
        .await
        .expect_err("a second signup with the same organisation name must be rejected");
    assert!(matches!(err, lango_backend::error::AppError::BadRequest(_)));

    // The rejected second signup must not have left a partial/orphaned user
    // row behind — the transaction must have rolled back cleanly.
    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = 'admin2@duplicate-bank.test'")
        .fetch_one(&pool)
        .await
        .expect("count users");
    assert_eq!(user_count, 0);
}

#[sqlx::test]
async fn signup_rejects_a_duplicate_email(pool: PgPool) {
    let state = AppState { db: pool.clone(), config: test_config() };

    let first = OrganisationSignupRequest {
        organisation_name: "First Org For Email Test".to_string(),
        email: "reused@example.test".to_string(),
        password: "a-reasonably-long-password".to_string(),
    };
    let _ = routes::organisations::signup(State(state.clone()), Json(first))
        .await
        .expect("first signup should succeed");

    let second = OrganisationSignupRequest {
        organisation_name: "Second Org For Email Test".to_string(),
        email: "reused@example.test".to_string(),
        password: "a-reasonably-long-password".to_string(),
    };
    let err = routes::organisations::signup(State(state), Json(second))
        .await
        .expect_err("a second signup reusing the same email must be rejected");
    assert!(matches!(err, lango_backend::error::AppError::BadRequest(_)));

    // The rejected second signup must not have left an orphaned
    // organisation row behind, since its user insert failed inside the
    // same transaction.
    let org_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM organisations WHERE name = 'Second Org For Email Test'")
            .fetch_one(&pool)
            .await
            .expect("count organisations");
    assert_eq!(org_count, 0, "the transaction must roll back the organisation insert too, not just the user insert");
}

#[sqlx::test]
async fn signup_rejects_a_too_short_password(pool: PgPool) {
    let state = AppState { db: pool.clone(), config: test_config() };
    let err = routes::organisations::signup(
        State(state),
        Json(OrganisationSignupRequest {
            organisation_name: "Weak Password Org".to_string(),
            email: "admin@weak-password-org.test".to_string(),
            password: "short".to_string(),
        }),
    )
    .await
    .expect_err("a too-short password must be rejected");
    assert!(matches!(err, lango_backend::error::AppError::BadRequest(_)));
}
