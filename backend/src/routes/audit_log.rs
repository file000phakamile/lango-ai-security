use axum::{extract::{Query, State}, Json};

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
    // their own department by the query itself, below (see Part 3 of the
    // multi-tenancy task — org/department scoping is enforced in the query,
    // not just at this role gate).
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

    let total: i64 = match &query.decision {
        Some(d) => {
            sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE decision = $1")
                .bind(d)
                .fetch_one(&state.db)
                .await?
        }
        None => {
            sqlx::query_scalar("SELECT COUNT(*) FROM audit_log")
                .fetch_one(&state.db)
                .await?
        }
    };

    let rows: Vec<AuditLogRow> = match &query.decision {
        Some(d) => {
            sqlx::query_as::<_, AuditLogRow>(
                r#"
                SELECT a.id, u.email AS user_email, a.department, a."timestamp",
                       a.entities_detected, a.risk_score, a.decision, a.reason_string,
                       a.ai_model_used, a.response_scan_result, a.sensitivity_class
                FROM audit_log a
                JOIN users u ON u.id = a.user_id
                WHERE a.decision = $1
                ORDER BY a."timestamp" DESC
                LIMIT $2 OFFSET $3
                "#,
            )
            .bind(d)
            .bind(page_size as i64)
            .bind(offset)
            .fetch_all(&state.db)
            .await?
        }
        None => {
            sqlx::query_as::<_, AuditLogRow>(
                r#"
                SELECT a.id, u.email AS user_email, a.department, a."timestamp",
                       a.entities_detected, a.risk_score, a.decision, a.reason_string,
                       a.ai_model_used, a.response_scan_result, a.sensitivity_class
                FROM audit_log a
                JOIN users u ON u.id = a.user_id
                ORDER BY a."timestamp" DESC
                LIMIT $1 OFFSET $2
                "#,
            )
            .bind(page_size as i64)
            .bind(offset)
            .fetch_all(&state.db)
            .await?
        }
    };

    let entries: Vec<AuditLogEntry> = rows.into_iter().map(AuditLogEntry::from).collect();

    Ok(Json(AuditLogPage {
        rows: entries,
        total,
        page,
        page_size,
    }))
}
