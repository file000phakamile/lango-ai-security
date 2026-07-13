use axum::{extract::{Query, State}, Json};
use sqlx::{Postgres, QueryBuilder};

use crate::{
    auth::{require_role, AuthUser},
    error::{AppError, AppResult},
    models::{AuditLogEntry, AuditLogPage, AuditLogQuery, AuditLogRow},
    state::AppState,
};

const VALID_DECISIONS: [&str; 4] = [
    "cleared_no_entities",
    "blocked_low_confidence",
    "redacted_and_forwarded",
    "redacted_low_confidence_review",
];

pub async fn get_audit_log(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(query): Query<AuditLogQuery>,
) -> AppResult<Json<AuditLogPage>> {
    // Both dashboard-facing roles can read the audit log — 'staff' cannot
    // (see the multi-tenancy task: "staff can only scan prompts... they see
    // nothing in the dashboard"). `department_reviewer` is further scoped to
    // their own department below, in the query itself — see the `WHERE`
    // clauses built below, not just this role gate.
    require_role(&claims, &["compliance_admin", "department_reviewer"])?;

    if let Some(d) = &query.decision {
        if !VALID_DECISIONS.contains(&d.as_str()) {
            return Err(AppError::BadRequest(format!(
                "invalid decision filter '{}'. Expected one of: {}",
                d,
                VALID_DECISIONS.join(", ")
            )));
        }
    }

    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) as i64 * page_size as i64;

    // `department_reviewer` only ever sees their own department's rows — a
    // real WHERE clause below, not merely a role check, so this is a
    // genuine query-level tenant/department boundary, not just an API
    // surface restriction. `compliance_admin` gets every department within
    // the same organisation. Neither role ever sees another organisation's
    // rows, regardless: `a.organisation_id = $1` is present in EVERY branch
    // below with no exceptions — see the cross-tenant isolation tests in
    // backend/tests/multi_tenant_isolation.rs.
    let department_scope: Option<&str> = if claims.role == "department_reviewer" {
        Some(claims.department.as_str())
    } else {
        None
    };

    // sqlx::QueryBuilder, not manual string concatenation — builds
    // parameterized SQL (still fully bind-parameter-safe, no string
    // interpolation of values) while keeping the "org, optionally
    // department, optionally decision" filter combination from turning into
    // 2^3 hand-written match arms the way the pre-multi-tenancy version of
    // this file had for just the 2-way decision-filter case.
    let mut count_qb: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM audit_log a WHERE a.organisation_id = ");
    count_qb.push_bind(claims.organisation_id);
    if let Some(dept) = department_scope {
        count_qb.push(" AND a.department = ").push_bind(dept);
    }
    if let Some(d) = &query.decision {
        count_qb.push(" AND a.decision = ").push_bind(d);
    }
    let total: i64 = count_qb.build_query_scalar().fetch_one(&state.db).await?;

    let mut rows_qb: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"
        SELECT a.id, u.email AS user_email, a.department, a."timestamp",
               a.entities_detected, a.risk_score, a.decision, a.reason_string,
               a.ai_model_used, a.response_scan_result, a.sensitivity_class
        FROM audit_log a
        JOIN users u ON u.id = a.user_id
        WHERE a.organisation_id =
        "#,
    );
    rows_qb.push_bind(claims.organisation_id);
    if let Some(dept) = department_scope {
        rows_qb.push(" AND a.department = ").push_bind(dept);
    }
    if let Some(d) = &query.decision {
        rows_qb.push(" AND a.decision = ").push_bind(d);
    }
    rows_qb.push(" ORDER BY a.\"timestamp\" DESC LIMIT ");
    rows_qb.push_bind(page_size as i64);
    rows_qb.push(" OFFSET ");
    rows_qb.push_bind(offset);

    let rows: Vec<AuditLogRow> = rows_qb
        .build_query_as::<AuditLogRow>()
        .fetch_all(&state.db)
        .await?;

    let entries: Vec<AuditLogEntry> = rows.into_iter().map(AuditLogEntry::from).collect();

    Ok(Json(AuditLogPage {
        rows: entries,
        total,
        page,
        page_size,
    }))
}
