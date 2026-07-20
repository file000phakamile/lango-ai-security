//! Cross-tenant isolation tests — THE most important test in the entire
//! multi-tenancy change (per the task this file was written for). Every
//! dashboard-reading endpoint must return data from the authenticated
//! user's own organisation ONLY, with zero exceptions. This file creates a
//! second, fully independent organisation with its own users and data, then
//! proves — for every endpoint, not just some of them — that a caller in
//! organisation A gets back zero rows/values derived from organisation B,
//! not merely "correct-looking" results that happen not to be checked
//! carefully enough to notice a leak.
//!
//! Uses `#[sqlx::test]`: each test function gets a freshly migrated,
//! throwaway Postgres database (via `DATABASE_URL`, same as any other sqlx
//! integration test) — real schema, real constraints, real queries, not
//! mocked. Route handlers are called directly as plain async functions
//! (they all are exactly that — `pub async fn(State<AppState>, AuthUser,
//! ...)`), so this exercises the real query logic without needing a running
//! HTTP server.
//!
//! Requires a real Postgres server reachable via `DATABASE_URL` to run (the
//! same requirement as any `#[sqlx::test]`-based suite) — this is why these
//! live in `tests/`, separate from the DB-free unit tests `cargo test --lib`
//! already covers. Run with `cargo test --test multi_tenant_isolation`
//! (DATABASE_URL must point at a real, reachable Postgres server; sqlx
//! creates and tears down an ephemeral database per test automatically).

use axum::extract::{Query, State};
use axum::Json;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use lango_backend::{
    auth::AuthUser,
    config::Config,
    models::{AuditLogQuery, Claims},
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
    }
}

fn claims_for(
    user_id: Uuid,
    session_id: Uuid,
    email: &str,
    department: &str,
    role: &str,
    organisation_id: Uuid,
) -> Claims {
    Claims {
        sub: user_id,
        session_id,
        email: email.to_string(),
        department: department.to_string(),
        role: role.to_string(),
        organisation_id,
        exp: (Utc::now() + Duration::hours(1)).timestamp() as usize,
    }
}

struct SeededUser {
    user_id: Uuid,
    session_id: Uuid,
}

async fn insert_org(pool: &PgPool, name: &str) -> Uuid {
    sqlx::query_scalar("INSERT INTO organisations (name) VALUES ($1) RETURNING id")
        .bind(name)
        .fetch_one(pool)
        .await
        .expect("insert organisation")
}

async fn insert_user(pool: &PgPool, org_id: Uuid, email: &str, department: &str, role: &str) -> SeededUser {
    let user_id: Uuid = sqlx::query_scalar(
        // consent_accepted_at/consent_policy_version are set immediately so
        // the scan-endpoint test below isn't blocked by the consent gate
        // (routes/scan.rs) added in Part 4 of the multi-tenancy task — this
        // file is testing tenant isolation, not the consent flow itself,
        // which has its own dedicated coverage.
        "INSERT INTO users (email, password_hash, department, role, organisation_id, consent_accepted_at, consent_policy_version) \
         VALUES ($1, 'unused-hash', $2, $3, $4, now(), 'v1') RETURNING id",
    )
    .bind(email)
    .bind(department)
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

#[allow(clippy::too_many_arguments)]
async fn insert_audit_log_row(
    pool: &PgPool,
    org_id: Uuid,
    user: &SeededUser,
    department: &str,
    decision: &str,
    reason_marker: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO audit_log (
            session_id, user_id, department, "timestamp", entities_detected, risk_score,
            decision, reason_string, ai_model_used, response_scan_result,
            original_prompt_hash, organisation_id
        )
        VALUES ($1, $2, $3, now(), '[]'::jsonb, 0.1, $4, $5, 'none', 'none', 'testhash', $6)
        "#,
    )
    .bind(user.session_id)
    .bind(user.user_id)
    .bind(department)
    .bind(decision)
    .bind(reason_marker)
    .bind(org_id)
    .execute(pool)
    .await
    .expect("insert audit_log row");
}

async fn insert_drift_snapshot(pool: &PgPool, org_id: Uuid, week_start: &str, psi: f32) {
    sqlx::query(
        "INSERT INTO drift_snapshots (week_start, psi_score, kl_divergence_score, organisation_id) \
         VALUES ($1::date, $2, 0.01, $3)",
    )
    .bind(week_start)
    .bind(psi)
    .bind(org_id)
    .execute(pool)
    .await
    .expect("insert drift snapshot");
}

async fn insert_security_event(pool: &PgPool, org_id: Uuid, detail: &str) {
    sqlx::query(
        "INSERT INTO security_events (event_type, detail, organisation_id) VALUES ('rate_limit_triggered', $1, $2)",
    )
    .bind(detail)
    .bind(org_id)
    .execute(pool)
    .await
    .expect("insert security event");
}

/// Full fixture: two independent organisations ("Org A" / "Org B"), each
/// with a compliance_admin, a department_reviewer, one audit_log row per
/// department, one drift_snapshots row, and one security_events row — all
/// with values that make it obvious in an assertion failure which
/// organisation's data leaked, if any did.
struct TwoOrgFixture {
    org_a: Uuid,
    org_a_admin: SeededUser,
    org_a_reviewer_dept_x: SeededUser,
    org_b: Uuid,
    org_b_admin: SeededUser,
}

async fn seed_two_orgs(pool: &PgPool) -> TwoOrgFixture {
    let org_a = insert_org(pool, "Isolation Test Org A").await;
    let org_b = insert_org(pool, "Isolation Test Org B").await;

    let org_a_admin = insert_user(pool, org_a, "admin@org-a.test", "Credit Risk", "compliance_admin").await;
    let org_a_reviewer_dept_x =
        insert_user(pool, org_a, "reviewer@org-a.test", "Credit Risk", "department_reviewer").await;
    let org_a_dept_y_user = insert_user(pool, org_a, "other@org-a.test", "Claims Processing", "staff").await;

    let org_b_admin = insert_user(pool, org_b, "admin@org-b.test", "Legal Affairs", "compliance_admin").await;

    insert_audit_log_row(pool, org_a, &org_a_admin, "Credit Risk", "blocked_low_confidence", "ORG_A_CREDIT_RISK_ROW").await;
    insert_audit_log_row(pool, org_a, &org_a_dept_y_user, "Claims Processing", "cleared_no_entities", "ORG_A_CLAIMS_ROW").await;
    insert_audit_log_row(pool, org_b, &org_b_admin, "Legal Affairs", "redacted_and_forwarded", "ORG_B_LEGAL_ROW").await;

    insert_drift_snapshot(pool, org_a, "2026-01-05", 0.05).await;
    insert_drift_snapshot(pool, org_b, "2026-01-05", 0.99).await; // deliberately alarming value — must never surface under org A

    insert_security_event(pool, org_a, "ORG_A_SECURITY_EVENT").await;
    insert_security_event(pool, org_b, "ORG_B_SECURITY_EVENT").await;

    TwoOrgFixture {
        org_a,
        org_a_admin,
        org_a_reviewer_dept_x,
        org_b,
        org_b_admin,
    }
}

#[sqlx::test]
async fn compliance_admin_sees_only_their_own_organisations_data_everywhere(pool: PgPool) {
    let fixture = seed_two_orgs(&pool).await;
    let state = AppState { db: pool.clone(), config: test_config() };

    let org_a_claims = claims_for(
        fixture.org_a_admin.user_id,
        fixture.org_a_admin.session_id,
        "admin@org-a.test",
        "Credit Risk",
        "compliance_admin",
        fixture.org_a,
    );

    // --- Audit log: org A's admin must see ONLY org A rows -----------------
    let audit_page = routes::audit_log::get_audit_log(
        State(state.clone()),
        AuthUser(org_a_claims.clone()),
        Query(AuditLogQuery { decision: None, page: None, page_size: Some(100) }),
    )
    .await
    .expect("get_audit_log should succeed for compliance_admin")
    .0;
    assert_eq!(audit_page.total, 2, "org A has exactly 2 audit_log rows seeded");
    assert!(
        audit_page.rows.iter().all(|r| r.reason != "ORG_B_LEGAL_ROW"),
        "org B's audit_log row must never appear for an org A caller"
    );
    assert!(
        audit_page.rows.iter().any(|r| r.reason == "ORG_A_CREDIT_RISK_ROW"),
        "org A's own row must still be visible — a filter that hides EVERYTHING is not a pass"
    );

    // --- Drift: org A's alarming-looking neighbor (PSI 0.99) must not leak -
    let drift = routes::drift::get_drift(State(state.clone()), AuthUser(org_a_claims.clone()))
        .await
        .expect("get_drift should succeed")
        .0;
    assert_eq!(drift.weeks.len(), 1, "org A has exactly 1 drift_snapshots row seeded");
    assert!(
        drift.weeks.iter().all(|w| w.psi < 0.5),
        "org B's PSI-0.99 row must never surface for an org A caller: got {:?}",
        drift.weeks.iter().map(|w| w.psi).collect::<Vec<_>>()
    );

    // --- Security events: org B's event string must never appear ----------
    let events = routes::security_events::get_security_events(State(state.clone()), AuthUser(org_a_claims.clone()))
        .await
        .expect("get_security_events should succeed")
        .0;
    assert!(events.events.iter().all(|e| e.detail != "ORG_B_SECURITY_EVENT"));
    assert!(events.events.iter().any(|e| e.detail == "ORG_A_SECURITY_EVENT"));

    // --- Command Center summary: counts must reflect ONLY org A's 2 rows --
    let summary = routes::command_center::get_summary(State(state.clone()), AuthUser(org_a_claims.clone()))
        .await
        .expect("get_summary should succeed")
        .0;
    assert_eq!(
        summary.sessions_scanned_today, 2,
        "command center's today-count must be scoped to org A's 2 rows, not org A+B's combined 3"
    );

    // --- Fairness: department list must be org A's departments only -------
    let fairness = routes::fairness::get_fairness(State(state.clone()), AuthUser(org_a_claims.clone()))
        .await
        .expect("get_fairness should succeed")
        .0;
    assert!(
        fairness.department_parity.iter().all(|p| p.group != "Legal Affairs"),
        "org B's department ('Legal Affairs') must never appear in org A's fairness breakdown"
    );
    assert!(fairness.department_parity.iter().any(|p| p.group == "Credit Risk"));

    // --- Health Data Guard summary: totals scoped to org A only -----------
    let health = routes::health::get_health_summary(State(state.clone()), AuthUser(org_a_claims.clone()))
        .await
        .expect("get_health_summary should succeed")
        .0;
    assert_eq!(
        health.standard_count + health.special_category_count,
        2,
        "health summary's row totals must be scoped to org A's 2 rows, not org A+B's combined 3"
    );

    // --- Reverse direction: org B's admin must see none of org A's data ---
    let org_b_claims = claims_for(
        fixture.org_b_admin.user_id,
        fixture.org_b_admin.session_id,
        "admin@org-b.test",
        "Legal Affairs",
        "compliance_admin",
        fixture.org_b,
    );
    let org_b_audit = routes::audit_log::get_audit_log(
        State(state.clone()),
        AuthUser(org_b_claims),
        Query(AuditLogQuery { decision: None, page: None, page_size: Some(100) }),
    )
    .await
    .expect("get_audit_log should succeed for org B's admin")
    .0;
    assert_eq!(org_b_audit.total, 1, "org B has exactly 1 audit_log row seeded");
    assert!(org_b_audit.rows.iter().all(|r| r.reason != "ORG_A_CREDIT_RISK_ROW" && r.reason != "ORG_A_CLAIMS_ROW"));

    // Silence an unused-field warning — org_a_reviewer_dept_x is exercised
    // by the dedicated department-scoping test below, not this one.
    let _ = fixture.org_a_reviewer_dept_x.user_id;
}

#[sqlx::test]
async fn department_reviewer_sees_only_their_own_department_within_their_own_organisation(pool: PgPool) {
    let fixture = seed_two_orgs(&pool).await;
    let state = AppState { db: pool.clone(), config: test_config() };

    let reviewer_claims = claims_for(
        fixture.org_a_reviewer_dept_x.user_id,
        fixture.org_a_reviewer_dept_x.session_id,
        "reviewer@org-a.test",
        "Credit Risk",
        "department_reviewer",
        fixture.org_a,
    );

    let audit_page = routes::audit_log::get_audit_log(
        State(state.clone()),
        AuthUser(reviewer_claims),
        Query(AuditLogQuery { decision: None, page: None, page_size: Some(100) }),
    )
    .await
    .expect("get_audit_log should succeed for department_reviewer")
    .0;

    // Only the Credit Risk row from org A — not org A's OWN Claims
    // Processing row (a different department, same organisation), and
    // definitely not org B's row.
    assert_eq!(audit_page.total, 1, "department_reviewer must see exactly 1 row (their own department only)");
    assert_eq!(audit_page.rows[0].reason, "ORG_A_CREDIT_RISK_ROW");
    assert!(audit_page.rows.iter().all(|r| r.reason != "ORG_A_CLAIMS_ROW"));
    assert!(audit_page.rows.iter().all(|r| r.reason != "ORG_B_LEGAL_ROW"));
}

#[sqlx::test]
async fn staff_role_is_forbidden_from_every_dashboard_endpoint(pool: PgPool) {
    let org_id = insert_org(&pool, "Staff Forbidden Test Org").await;
    let staff = insert_user(&pool, org_id, "staff@org.test", "Credit Risk", "staff").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let staff_claims = claims_for(staff.user_id, staff.session_id, "staff@org.test", "Credit Risk", "staff", org_id);

    assert!(routes::audit_log::get_audit_log(
        State(state.clone()),
        AuthUser(staff_claims.clone()),
        Query(AuditLogQuery { decision: None, page: None, page_size: None }),
    )
    .await
    .is_err(), "staff must be forbidden from the audit log");

    assert!(routes::command_center::get_summary(State(state.clone()), AuthUser(staff_claims.clone()))
        .await
        .is_err(), "staff must be forbidden from the command center summary");

    assert!(routes::drift::get_drift(State(state.clone()), AuthUser(staff_claims.clone()))
        .await
        .is_err(), "staff must be forbidden from drift");

    assert!(routes::fairness::get_fairness(State(state.clone()), AuthUser(staff_claims.clone()))
        .await
        .is_err(), "staff must be forbidden from fairness");

    assert!(routes::health::get_health_summary(State(state.clone()), AuthUser(staff_claims.clone()))
        .await
        .is_err(), "staff must be forbidden from the health summary");

    assert!(routes::security_events::get_security_events(State(state.clone()), AuthUser(staff_claims))
        .await
        .is_err(), "staff must be forbidden from security events");
}

#[sqlx::test]
async fn scanning_writes_the_calling_users_own_organisation_id_not_any_other(pool: PgPool) {
    let fixture = seed_two_orgs(&pool).await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let org_a_claims = claims_for(
        fixture.org_a_admin.user_id,
        fixture.org_a_admin.session_id,
        "admin@org-a.test",
        "Credit Risk",
        "compliance_admin",
        fixture.org_a,
    );

    let scan_request = lango_backend::models::ScanRequest {
        prompt: "What is the capital of Zimbabwe?".to_string(),
        language: None,
        facility_type: None,
    };
    let _ = routes::scan::scan(State(state.clone()), AuthUser(org_a_claims), Json(scan_request))
        .await
        .expect("scan should succeed for a clean prompt");

    let org_id_written: Uuid = sqlx::query_scalar(
        "SELECT organisation_id FROM audit_log WHERE reason_string LIKE '%No sensitive entities%' ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .expect("the scan above must have written an audit_log row");
    assert_eq!(org_id_written, fixture.org_a, "a scan by an org A user must be recorded under org A, never org B");
    assert_ne!(org_id_written, fixture.org_b);
}
