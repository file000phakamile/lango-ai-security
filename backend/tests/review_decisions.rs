//! Active learning loop (product-depth task, Part 3) integration tests.
//! Same `#[sqlx::test]` pattern as the other files in this directory — real
//! Postgres, real route handlers called directly. Run with
//! `cargo test --test review_decisions` (requires `DATABASE_URL`).

use axum::extract::{Path, Query, State};
use axum::Json;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use lango_backend::{
    auth::AuthUser,
    config::Config,
    models::{Claims, RecordReviewDecisionRequest},
    routes::{self, labelled_dataset::LabelledDatasetQuery},
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

fn claims_for(user_id: Uuid, session_id: Uuid, role: &str, department: &str, organisation_id: Uuid) -> Claims {
    Claims {
        sub: user_id,
        session_id,
        email: format!("{role}@test"),
        department: department.to_string(),
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

async fn insert_user(pool: &PgPool, org_id: Uuid, email: &str, department: &str, role: &str) -> SeededUser {
    let user_id: Uuid = sqlx::query_scalar(
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

async fn insert_audit_log_row(pool: &PgPool, org_id: Uuid, user: &SeededUser, department: &str, decision: &str) -> Uuid {
    sqlx::query_scalar(
        r#"
        INSERT INTO audit_log (
            session_id, user_id, department, "timestamp", entities_detected, risk_score,
            decision, reason_string, ai_model_used, response_scan_result,
            original_prompt_hash, organisation_id
        )
        VALUES ($1, $2, $3, now(), '["full_name"]'::jsonb, 0.2, $4, 'low-confidence name match', 'none', 'none', 'testhash', $5)
        RETURNING id
        "#,
    )
    .bind(user.session_id)
    .bind(user.user_id)
    .bind(department)
    .bind(decision)
    .bind(org_id)
    .fetch_one(pool)
    .await
    .expect("insert audit_log row")
}

#[sqlx::test]
async fn compliance_admin_can_confirm_a_flagged_low_confidence_row(pool: PgPool) {
    let org = insert_org(&pool, "Review Org").await;
    let admin = insert_user(&pool, org, "admin@review.test", "Credit Risk", "compliance_admin").await;
    let audit_id = insert_audit_log_row(&pool, org, &admin, "Credit Risk", "redacted_low_confidence_review").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", "Credit Risk", org);

    let response = routes::review_decisions::record_review_decision(
        State(state),
        AuthUser(claims),
        Path(audit_id),
        Json(RecordReviewDecisionRequest {
            decision: "confirmed".to_string(),
            reasoning: Some("Genuinely a person's name, correctly redacted.".to_string()),
        }),
    )
    .await
    .expect("recording a review decision on an eligible row must succeed");
    assert!(response.recorded);
}

#[sqlx::test]
async fn a_second_review_decision_on_the_same_row_is_rejected(pool: PgPool) {
    let org = insert_org(&pool, "Review Duplicate Org").await;
    let admin = insert_user(&pool, org, "admin@dup.test", "Credit Risk", "compliance_admin").await;
    let audit_id = insert_audit_log_row(&pool, org, &admin, "Credit Risk", "blocked_low_confidence").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", "Credit Risk", org);

    let _ = routes::review_decisions::record_review_decision(
        State(state.clone()),
        AuthUser(claims.clone()),
        Path(audit_id),
        Json(RecordReviewDecisionRequest { decision: "overturned".to_string(), reasoning: None }),
    )
    .await
    .expect("first review decision must succeed");

    let second = routes::review_decisions::record_review_decision(
        State(state),
        AuthUser(claims),
        Path(audit_id),
        Json(RecordReviewDecisionRequest { decision: "confirmed".to_string(), reasoning: None }),
    )
    .await;
    assert!(second.is_err(), "a second review decision on the same row must be rejected, not silently overwrite the first");
}

#[sqlx::test]
async fn a_high_confidence_row_is_not_eligible_for_review(pool: PgPool) {
    let org = insert_org(&pool, "Review Ineligible Org").await;
    let admin = insert_user(&pool, org, "admin@ineligible.test", "Credit Risk", "compliance_admin").await;
    let audit_id = insert_audit_log_row(&pool, org, &admin, "Credit Risk", "redacted_and_forwarded").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", "Credit Risk", org);

    let result = routes::review_decisions::record_review_decision(
        State(state),
        AuthUser(claims),
        Path(audit_id),
        Json(RecordReviewDecisionRequest { decision: "confirmed".to_string(), reasoning: None }),
    )
    .await;
    assert!(result.is_err(), "a fully-trusted redacted_and_forwarded row is not a flagged low-confidence item and must be rejected");
}

#[sqlx::test]
async fn staff_role_cannot_record_a_review_decision(pool: PgPool) {
    let org = insert_org(&pool, "Review Staff Org").await;
    let staff = insert_user(&pool, org, "staff@review.test", "Credit Risk", "staff").await;
    let audit_id = insert_audit_log_row(&pool, org, &staff, "Credit Risk", "blocked_low_confidence").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(staff.user_id, staff.session_id, "staff", "Credit Risk", org);

    let result = routes::review_decisions::record_review_decision(
        State(state),
        AuthUser(claims),
        Path(audit_id),
        Json(RecordReviewDecisionRequest { decision: "confirmed".to_string(), reasoning: None }),
    )
    .await;
    assert!(result.is_err(), "staff must not be able to record a review decision");
}

#[sqlx::test]
async fn department_reviewer_cannot_review_a_row_outside_their_own_department(pool: PgPool) {
    let org = insert_org(&pool, "Review Dept Scope Org").await;
    let submitter = insert_user(&pool, org, "submitter@dept.test", "Claims Processing", "staff").await;
    let reviewer = insert_user(&pool, org, "reviewer@dept.test", "Credit Risk", "department_reviewer").await;
    let audit_id = insert_audit_log_row(&pool, org, &submitter, "Claims Processing", "blocked_low_confidence").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(reviewer.user_id, reviewer.session_id, "department_reviewer", "Credit Risk", org);

    let result = routes::review_decisions::record_review_decision(
        State(state),
        AuthUser(claims),
        Path(audit_id),
        Json(RecordReviewDecisionRequest { decision: "overturned".to_string(), reasoning: None }),
    )
    .await;
    assert!(
        result.is_err(),
        "a department_reviewer must not be able to review a row from a different department"
    );
}

#[sqlx::test]
async fn a_row_belonging_to_another_organisation_cannot_be_reviewed(pool: PgPool) {
    let org_a = insert_org(&pool, "Review Iso Org A").await;
    let org_b = insert_org(&pool, "Review Iso Org B").await;
    let admin_a = insert_user(&pool, org_a, "admin@iso-review-a.test", "Credit Risk", "compliance_admin").await;
    let admin_b = insert_user(&pool, org_b, "admin@iso-review-b.test", "Credit Risk", "compliance_admin").await;
    let audit_id_b = insert_audit_log_row(&pool, org_b, &admin_b, "Credit Risk", "blocked_low_confidence").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims_a = claims_for(admin_a.user_id, admin_a.session_id, "compliance_admin", "Credit Risk", org_a);

    let result = routes::review_decisions::record_review_decision(
        State(state),
        AuthUser(claims_a),
        Path(audit_id_b),
        Json(RecordReviewDecisionRequest { decision: "confirmed".to_string(), reasoning: None }),
    )
    .await;
    assert!(result.is_err(), "org A must not be able to record a review decision on org B's audit_log row");
}

#[sqlx::test]
async fn recorded_decisions_appear_in_the_audit_log_response_and_the_labelled_dataset_export(pool: PgPool) {
    let org = insert_org(&pool, "Review E2E Org").await;
    let admin = insert_user(&pool, org, "admin@e2e.test", "Credit Risk", "compliance_admin").await;
    let audit_id = insert_audit_log_row(&pool, org, &admin, "Credit Risk", "redacted_low_confidence_review").await;
    let state = AppState { db: pool.clone(), config: test_config() };
    let claims = claims_for(admin.user_id, admin.session_id, "compliance_admin", "Credit Risk", org);

    let _ = routes::review_decisions::record_review_decision(
        State(state.clone()),
        AuthUser(claims.clone()),
        Path(audit_id),
        Json(RecordReviewDecisionRequest {
            decision: "overturned".to_string(),
            reasoning: Some("False positive - ordinary capitalized phrase.".to_string()),
        }),
    )
    .await
    .expect("review decision must succeed");

    // The audit log's own read path must show the recorded review inline.
    let audit_page = routes::audit_log::get_audit_log(
        State(state.clone()),
        AuthUser(claims.clone()),
        Query(lango_backend::models::AuditLogQuery { decision: None, page: None, page_size: None }),
    )
    .await
    .expect("get_audit_log should succeed")
    .0;
    let row = audit_page.rows.iter().find(|r| r.id == audit_id).expect("the reviewed row must be present");
    let review = row.review.as_ref().expect("a recorded review decision must appear inline on the audit log row");
    assert_eq!(review.decision, "overturned");
    // The reviewer's email comes from the `users` table via `reviewer_user_id`
    // (the real DB row), not from `claims.email` (this test helper's
    // synthetic JWT claims value) — confirms the JOIN in
    // routes::audit_log::get_audit_log actually resolves the real user.
    assert_eq!(review.reviewer_email, "admin@e2e.test");

    // And the labelled-dataset export must include it too.
    let csv_response = routes::labelled_dataset::export(
        State(state),
        AuthUser(claims),
        Query(LabelledDatasetQuery { format: "csv".to_string() }),
    )
    .await
    .expect("labelled dataset export should succeed");
    let bytes = axum::body::to_bytes(csv_response.into_body(), usize::MAX).await.unwrap();
    let csv = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(csv.contains("overturned"));
    assert!(csv.contains("False positive - ordinary capitalized phrase."));
    assert!(csv.contains("redacted_low_confidence_review"));
}
