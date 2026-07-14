//! Compliance export (product-depth task, Part 2): a one-click, date-ranged
//! export of the audit log, fairness metrics, and drift history for the
//! caller's own organisation — CSV or PDF, `compliance_admin` only. This
//! module is the HTTP/DB layer; the actual formatting logic lives in
//! `crate::reports` (pure, unit-tested, no database dependency).
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    auth::{require_role, AuthUser},
    error::{AppError, AppResult},
    models::{Claims, ParityEntry},
    reports::{self, ComplianceExportData, ExportAuditRow, ExportDriftWeek},
    routes::fairness::compute_dir_spd,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct ComplianceExportQuery {
    pub start: NaiveDate,
    pub end: NaiveDate,
    /// "csv" or "pdf" — validated explicitly below rather than modeled as an
    /// enum, so an unrecognized value produces the same clean 400 as every
    /// other validation failure in this handler instead of axum's generic
    /// query-deserialization error text.
    pub format: String,
}

async fn fetch_audit_rows(
    pool: &PgPool,
    org_id: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<ExportAuditRow>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: Uuid,
        timestamp: chrono::DateTime<Utc>,
        user_email: String,
        department: String,
        entities_detected: sqlx::types::Json<Vec<String>>,
        risk_score: f32,
        decision: String,
        sensitivity_class: String,
        reason_string: String,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"
        SELECT a.id, a."timestamp", u.email AS user_email, a.department,
               a.entities_detected, a.risk_score, a.decision, a.sensitivity_class,
               a.reason_string
        FROM audit_log a
        JOIN users u ON u.id = a.user_id
        WHERE a.organisation_id = $1
          AND a."timestamp" >= $2::date
          AND a."timestamp" < ($3::date + interval '1 day')
        ORDER BY a."timestamp" DESC
        "#,
    )
    .bind(org_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ExportAuditRow {
            id: r.id.to_string(),
            timestamp: r.timestamp,
            user_email: r.user_email,
            department: r.department,
            entities_detected: r.entities_detected.0,
            risk_score: r.risk_score,
            decision: r.decision,
            sensitivity_class: r.sensitivity_class,
            reason_string: r.reason_string,
        })
        .collect())
}

/// Same flag-rate math as `routes::fairness::get_fairness`, but scoped to
/// the requested date range — a deliberately separate query rather than
/// adding an optional date-range parameter to the live dashboard endpoint,
/// since the live Fairness Audit view is intentionally "all of this org's
/// history so far," not date-scoped, and conflating the two would risk the
/// live view silently picking up an unintended filter.
async fn fetch_parity(
    pool: &PgPool,
    org_id: Uuid,
    start: NaiveDate,
    end: NaiveDate,
    group_column: &str,
    extra_where: &str,
) -> Result<Vec<ParityEntry>, AppError> {
    let sql = format!(
        r#"
        SELECT {group_column} AS "group",
               ROUND(
                   100.0 * SUM(CASE WHEN decision <> 'cleared_no_entities' THEN 1 ELSE 0 END)
                   / COUNT(*)::numeric,
                   1
               )::float8 AS flag_rate
        FROM audit_log
        WHERE organisation_id = $1
          AND "timestamp" >= $2::date
          AND "timestamp" < ($3::date + interval '1 day')
          {extra_where}
        GROUP BY {group_column}
        ORDER BY {group_column}
        "#
    );
    let rows: Vec<ParityEntry> = sqlx::query_as(&sql).bind(org_id).bind(start).bind(end).fetch_all(pool).await?;
    Ok(rows)
}

async fn fetch_drift_weeks(
    pool: &PgPool,
    org_id: Uuid,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<ExportDriftWeek>, AppError> {
    #[derive(sqlx::FromRow)]
    struct Row {
        week_start: NaiveDate,
        psi_score: f32,
        kl_divergence_score: f32,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT week_start, psi_score, kl_divergence_score FROM drift_snapshots \
         WHERE organisation_id = $1 AND week_start BETWEEN $2 AND $3 ORDER BY week_start ASC",
    )
    .bind(org_id)
    .bind(start)
    .bind(end)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ExportDriftWeek {
            week_start: r.week_start,
            psi_score: r.psi_score,
            kl_divergence_score: r.kl_divergence_score,
            alert: r.psi_score >= crate::models::PSI_ALERT_THRESHOLD,
        })
        .collect())
}

fn file_response(content_type: &'static str, filename: String, bytes: Vec<u8>) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{filename}\"")),
        ],
        bytes,
    )
        .into_response()
}

async fn build_export_data(
    state: &AppState,
    claims: &Claims,
    start: NaiveDate,
    end: NaiveDate,
) -> AppResult<ComplianceExportData> {
    let org_id = claims.organisation_id;
    let organisation_name: String = sqlx::query_scalar("SELECT name FROM organisations WHERE id = $1")
        .bind(org_id)
        .fetch_one(&state.db)
        .await?;

    let audit_rows = fetch_audit_rows(&state.db, org_id, start, end).await?;
    let department_parity = fetch_parity(&state.db, org_id, start, end, "department", "").await?;
    let language_parity =
        fetch_parity(&state.db, org_id, start, end, "language", "AND language IS NOT NULL").await?;
    let (dir_department, spd_department) = compute_dir_spd(&department_parity);
    let (dir_language, spd_language) = compute_dir_spd(&language_parity);
    let drift_weeks = fetch_drift_weeks(&state.db, org_id, start, end).await?;

    Ok(ComplianceExportData {
        organisation_name,
        range_start: start,
        range_end: end,
        generated_at: Utc::now(),
        audit_rows,
        department_parity,
        language_parity,
        dir_department,
        spd_department,
        dir_language,
        spd_language,
        fairness_threshold: crate::routes::fairness::DIR_THRESHOLD,
        drift_weeks,
    })
}

pub async fn export(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Query(query): Query<ComplianceExportQuery>,
) -> AppResult<Response> {
    require_role(&claims, &["compliance_admin"])?;

    if query.start > query.end {
        return Err(AppError::BadRequest(
            "start date must not be after end date.".to_string(),
        ));
    }
    if query.format != "csv" && query.format != "pdf" {
        return Err(AppError::BadRequest(
            "format must be 'csv' or 'pdf'.".to_string(),
        ));
    }

    let data = build_export_data(&state, &claims, query.start, query.end).await?;
    let filename_stem = format!("lango-compliance-export_{}_{}-to-{}", data.organisation_name.replace(' ', "-"), query.start, query.end);

    if query.format == "csv" {
        let csv = reports::build_csv(&data);
        Ok(file_response("text/csv", format!("{filename_stem}.csv"), csv.into_bytes()))
    } else {
        let pdf = reports::build_pdf(&data);
        Ok(file_response("application/pdf", format!("{filename_stem}.pdf"), pdf))
    }
}
