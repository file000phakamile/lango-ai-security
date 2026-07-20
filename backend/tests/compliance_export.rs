//! Compliance export (product-depth task, Part 2) integration tests. Same
//! `#[sqlx::test]` pattern as `multi_tenant_isolation.rs` — real Postgres,
//! real route handlers called directly, no HTTP server, no mocks. Run with
//! `cargo test --test compliance_export` (requires `DATABASE_URL`).

use axum::extract::{Query, State};
use chrono::{Duration, NaiveDate, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use lango_backend::{
    auth::AuthUser,
    config::Config,
    models::Claims,
    routes::{self, compliance_export::ComplianceExportQuery},
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

async fn insert_audit_log_row(
    pool: &PgPool,
    org_id: Uuid,
    user: &SeededUser,
    reason_marker: &str,
    timestamp: &str,
) {
    sqlx::query(
        r#"
        INSERT INTO audit_log (
            session_id, user_id, department, language, "timestamp", entities_detected, risk_score,
            decision, reason_string, ai_model_used, response_scan_result,
            original_prompt_hash, organisation_id
        )
        VALUES ($1, $2, 'Credit Risk', 'English', $3::timestamptz, '["national_id"]'::jsonb, 0.5,
                'redacted_and_forwarded', $4, 'none', 'none', 'testhash', $5)
        "#,
    )
    .bind(user.session_id)
    .bind(user.user_id)
    .bind(timestamp)
    .bind(reason_marker)
    .bind(org_id)
    .execute(pool)
    .await
    .expect("insert audit_log row");
}

async fn body_bytes(response: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collecting the response body must succeed")
        .to_vec()
}

#[sqlx::test]
async fn compliance_admin_gets_a_real_csv_covering_only_rows_in_range(pool: PgPool) {
    let org = insert_org(&pool, "Export CSV Org").await;
    let admin = insert_user(&pool, org, "admin@exportcsv.test", "compliance_admin").await;
    insert_audit_log_row(&pool, org, &admin, "IN_RANGE_ROW", "2026-03-15T12:00:00Z").await;
    insert_audit_log_row(&pool, org, &admin, "OUT_OF_RANGE_ROW", "2025-01-01T12:00:00Z").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let response = routes::compliance_export::export(
        State(state),
        AuthUser(claims),
        Query(ComplianceExportQuery {
            start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            format: "csv".to_string(),
        }),
    )
    .await
    .expect("csv export should succeed for compliance_admin");

    assert_eq!(response.headers().get("content-type").unwrap(), "text/csv");
    let body = String::from_utf8(body_bytes(response).await).unwrap();
    assert!(body.contains("Export CSV Org"));
    assert!(body.contains("IN_RANGE_ROW"));
    assert!(!body.contains("OUT_OF_RANGE_ROW"), "a row outside the requested date range must not appear in the export");
    assert!(body.contains("AUDIT LOG"));
    assert!(body.contains("FAIRNESS METRICS"));
    assert!(body.contains("DRIFT HISTORY"));
}

#[sqlx::test]
async fn compliance_admin_gets_a_real_pdf(pool: PgPool) {
    let org = insert_org(&pool, "Export PDF Org").await;
    let admin = insert_user(&pool, org, "admin@exportpdf.test", "compliance_admin").await;
    insert_audit_log_row(&pool, org, &admin, "PDF_ROW", "2026-03-15T12:00:00Z").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let response = routes::compliance_export::export(
        State(state),
        AuthUser(claims),
        Query(ComplianceExportQuery {
            start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            format: "pdf".to_string(),
        }),
    )
    .await
    .expect("pdf export should succeed for compliance_admin");

    assert_eq!(response.headers().get("content-type").unwrap(), "application/pdf");
    let bytes = body_bytes(response).await;
    assert_eq!(&bytes[0..5], b"%PDF-", "the response body must be a real, well-formed PDF");
}

#[sqlx::test]
async fn non_compliance_admin_roles_are_forbidden(pool: PgPool) {
    let org = insert_org(&pool, "Export RBAC Org").await;
    let reviewer = insert_user(&pool, org, "reviewer@exportrbac.test", "department_reviewer").await;
    let staff = insert_user(&pool, org, "staff@exportrbac.test", "staff").await;
    let state = AppState { db: pool.clone(), config: test_config() };

    for user in [reviewer, staff] {
        let claims = claims_for(user.user_id, user.session_id, "irrelevant", org);
        let result = routes::compliance_export::export(
            State(state.clone()),
            AuthUser(claims),
            Query(ComplianceExportQuery {
                start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                end: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
                format: "csv".to_string(),
            }),
        )
        .await;
        assert!(result.is_err(), "only compliance_admin may run a compliance export");
    }
}

#[sqlx::test]
async fn a_start_date_after_the_end_date_is_rejected(pool: PgPool) {
    let org = insert_org(&pool, "Export Date Order Org").await;
    let admin = insert_user(&pool, org, "admin@dateorder.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let result = routes::compliance_export::export(
        State(state),
        AuthUser(claims),
        Query(ComplianceExportQuery {
            start: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            format: "csv".to_string(),
        }),
    )
    .await;
    assert!(result.is_err(), "start date after end date must be rejected");
}

#[sqlx::test]
async fn an_invalid_format_value_is_rejected(pool: PgPool) {
    let org = insert_org(&pool, "Export Format Org").await;
    let admin = insert_user(&pool, org, "admin@format.test", "compliance_admin").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", org);

    let result = routes::compliance_export::export(
        State(state),
        AuthUser(claims),
        Query(ComplianceExportQuery {
            start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            format: "xlsx".to_string(),
        }),
    )
    .await;
    assert!(result.is_err(), "an unsupported format value must be rejected");
}

#[sqlx::test]
async fn an_organisations_export_never_includes_another_organisations_rows(pool: PgPool) {
    let org_a = insert_org(&pool, "Export Isolation Org A").await;
    let org_b = insert_org(&pool, "Export Isolation Org B").await;
    let admin_a = insert_user(&pool, org_a, "admin@iso-export-a.test", "compliance_admin").await;
    let admin_b = insert_user(&pool, org_b, "admin@iso-export-b.test", "compliance_admin").await;
    insert_audit_log_row(&pool, org_a, &admin_a, "ORG_A_EXPORT_ROW", "2026-03-15T12:00:00Z").await;
    insert_audit_log_row(&pool, org_b, &admin_b, "ORG_B_EXPORT_ROW", "2026-03-15T12:00:00Z").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims_a = claims_for(admin_a.user_id, admin_a.session_id, "compliance_admin", org_a);

    let response = routes::compliance_export::export(
        State(state),
        AuthUser(claims_a),
        Query(ComplianceExportQuery {
            start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            format: "csv".to_string(),
        }),
    )
    .await
    .expect("csv export should succeed");
    let body = String::from_utf8(body_bytes(response).await).unwrap();
    assert!(body.contains("ORG_A_EXPORT_ROW"));
    assert!(!body.contains("ORG_B_EXPORT_ROW"), "org B's audit_log row must never appear in org A's export");
}
