//! Labelled dataset export (product-depth task, Part 3) — the export side
//! of the active learning loop. `routes::review_decisions` is where a human
//! judgment gets recorded; this module just formats what's already there.
//! `compliance_admin` only, org-scoped, CSV or JSONL. No date range — the
//! task asked for "a simple export of this labelled dataset", and unlike
//! the compliance export (Part 2), there's no natural "for this quarter"
//! framing here: every labelled example an organisation has ever produced
//! is training/rule-tuning signal, so this exports the whole thing.
use axum::{extract::State, response::Response};
use serde::Deserialize;

use crate::{
    auth::{require_role, AuthUser},
    error::{AppError, AppResult},
    models::LabelledExampleRow,
    reports,
    routes::compliance_export::file_response,
    state::AppState,
};

#[derive(Debug, Deserialize)]
pub struct LabelledDatasetQuery {
    pub format: String,
}

pub async fn export(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    axum::extract::Query(query): axum::extract::Query<LabelledDatasetQuery>,
) -> AppResult<Response> {
    require_role(&claims, &["compliance_admin"])?;

    if query.format != "csv" && query.format != "jsonl" {
        return Err(AppError::BadRequest("format must be 'csv' or 'jsonl'.".to_string()));
    }

    let rows: Vec<LabelledExampleRow> = sqlx::query_as(
        r#"
        SELECT id, audit_log_id, reviewer_role, decision, reasoning,
               original_decision, original_entities_detected, original_risk_score,
               original_reason_string, original_sensitivity_class, original_department,
               created_at
        FROM review_decisions
        WHERE organisation_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(claims.organisation_id)
    .fetch_all(&state.db)
    .await?;

    if query.format == "jsonl" {
        let jsonl = reports::build_labelled_dataset_jsonl(&rows);
        Ok(file_response(
            "application/x-ndjson",
            "lango-labelled-dataset.jsonl".to_string(),
            jsonl.into_bytes(),
        ))
    } else {
        let csv = reports::build_labelled_dataset_csv(&rows);
        Ok(file_response("text/csv", "lango-labelled-dataset.csv".to_string(), csv.into_bytes()))
    }
}
