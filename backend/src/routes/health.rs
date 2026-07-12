use axum::{extract::State, Json};

use crate::{
    auth::{require_role, AuthUser},
    error::AppResult,
    models::{HealthSummaryResponse, ParityEntry},
    state::AppState,
};

use super::fairness::{compute_dir_spd, DIR_THRESHOLD};

/// Health Data Guard summary — the aggregate-reporting endpoint backing the
/// dashboard's sixth view. Built for the Cimas Healthathon 3.0 submission
/// (see docs/HEALTH_MODULE.md).
///
/// *** STIGMA-AWARE AGGREGATE REPORTING — READ BEFORE ADDING A FIELD HERE ***
/// This handler deliberately returns ONLY: a total special-category-health
/// count, a redaction rate, the standard/special-category count split, and a
/// facility-type DIR/SPD comparison. It does NOT — and must never — group or
/// filter by `entities_detected`'s individual entity types (diagnosis_code
/// vs. medication_name vs. medical_aid_number vs. lab_result_value vs.
/// next_of_kin), let alone by the specific ICD-10 code or medication name
/// decoded from a match. A per-department "N diagnosis_code detections this
/// week" count sounds harmless in isolation, but combined with a small
/// department headcount it becomes a de-anonymisation vector: in the
/// Zimbabwean context this module was built for, that is directly an HIV-
/// status-stigma risk, not an abstract privacy concern — a handful of
/// health-related detections attributed to one small team over a short
/// window can effectively identify who they came from, and the harm of that
/// (discrimination, social stigma) is categorically worse than an ordinary
/// PII leak. The existing per-entry Audit Log detail view (one specific,
/// already-flagged session, reviewed by an authorized compliance officer) is
/// the correct, legitimate place for that level of detail — see
/// `models::AuditLogRow`'s `sensitivity_class` field and
/// `routes::audit_log::get_audit_log`, both unrestricted, both scoped to one
/// row at a time. This restriction applies ONLY to aggregate/trend reporting
/// like this endpoint. See docs/SECURITY_PRIVACY.md's matching note.
pub async fn get_health_summary(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<HealthSummaryResponse>> {
    require_role(&claims, &["compliance", "admin"])?;

    let standard_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM audit_log WHERE sensitivity_class = 'standard'")
            .fetch_one(&state.db)
            .await?;

    let special_category_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE sensitivity_class = 'special_category_health'",
    )
    .fetch_one(&state.db)
    .await?;

    // Redaction rate: of every special_category_health row (including
    // blocked ones), what fraction actually made it through as
    // redacted_and_forwarded. Per Part 2's hard rule, `decision` can never
    // be 'redacted_low_confidence_review' for a special_category_health row
    // — that decision value is structurally unreachable here, not merely
    // absent from this query by choice (see
    // detection::scan::scan_prompt's `is_leniency_eligible`).
    let redaction_rate: f64 = sqlx::query_scalar(
        r#"
        SELECT COALESCE(
            ROUND(
                100.0 * SUM(CASE WHEN decision = 'redacted_and_forwarded' THEN 1 ELSE 0 END)
                / NULLIF(COUNT(*), 0)::numeric,
                1
            )::float8,
            0.0
        )
        FROM audit_log
        WHERE sensitivity_class = 'special_category_health'
        "#,
    )
    .fetch_one(&state.db)
    .await?;

    // Facility-type parity — the SAME Disparate Impact Ratio / Statistical
    // Parity Difference math already built for the Fairness Audit view's
    // language/department comparison (`routes::fairness::compute_dir_spd`,
    // reused here, not reimplemented), applied to a new grouping dimension
    // (facility_type) and scoped to special_category_health rows only: this
    // checks whether special-category detection accuracy is equitable
    // across facility types (e.g. a rural clinic vs. an urban hospital), the
    // same way the existing view checks it across departments and
    // languages. `facility_type` is optional and caller-declared (see
    // models::ScanRequest) — only rows that supplied it are included.
    let facility_parity: Vec<ParityEntry> = sqlx::query_as::<_, ParityEntry>(
        r#"
        SELECT facility_type AS "group",
               ROUND(
                   100.0 * SUM(CASE WHEN decision <> 'cleared_no_entities' THEN 1 ELSE 0 END)
                   / COUNT(*)::numeric,
                   1
               )::float8 AS flag_rate
        FROM audit_log
        WHERE facility_type IS NOT NULL AND sensitivity_class = 'special_category_health'
        GROUP BY facility_type
        ORDER BY facility_type
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    let (dir_facility, spd_facility) = compute_dir_spd(&facility_parity);

    Ok(Json(HealthSummaryResponse {
        special_category_total: special_category_count,
        standard_count,
        special_category_count,
        redaction_rate,
        facility_parity,
        dir_facility,
        spd_facility,
        threshold: DIR_THRESHOLD,
    }))
}
