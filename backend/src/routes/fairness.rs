use axum::{extract::State, Json};

use crate::{
    auth::{require_role, AuthUser},
    error::AppResult,
    models::{FairnessResponse, ParityEntry},
    state::AppState,
};

/// Same threshold and calculation method as the proposal: Disparate Impact
/// Ratio = lowest group flag rate / highest group flag rate, with a 0.80
/// pass threshold (the "80% rule" commonly used in fairness auditing).
/// Statistical Parity Difference = highest - lowest, in percentage points.
pub(crate) const DIR_THRESHOLD: f64 = 0.80;

/// `pub(crate)` (not private) so `routes::health` can reuse the exact same
/// DIR/SPD math for its facility-type comparison instead of re-implementing
/// it — see routes/health.rs's own comment.
pub(crate) fn compute_dir_spd(groups: &[ParityEntry]) -> (Option<f64>, Option<f64>) {
    let rates: Vec<f64> = groups.iter().map(|g| g.flag_rate).collect();
    let max = rates.iter().cloned().fold(f64::MIN, f64::max);
    let min = rates.iter().cloned().fold(f64::MAX, f64::min);
    if rates.is_empty() || max <= 0.0 {
        return (None, None);
    }
    let dir = (min / max * 100.0).round() / 100.0;
    let spd = ((max - min) * 10.0).round() / 10.0;
    (Some(dir), Some(spd))
}

pub async fn get_fairness(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<FairnessResponse>> {
    require_role(&claims, &["compliance_admin"])?;
    let org_id = claims.organisation_id;

    let department_parity: Vec<ParityEntry> = sqlx::query_as::<_, ParityEntry>(
        r#"
        SELECT department AS "group",
               ROUND(
                   100.0 * SUM(CASE WHEN decision <> 'cleared_no_entities' THEN 1 ELSE 0 END)
                   / COUNT(*)::numeric,
                   1
               )::float8 AS flag_rate
        FROM audit_log
        WHERE organisation_id = $1
        GROUP BY department
        ORDER BY department
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.db)
    .await?;

    let language_parity: Vec<ParityEntry> = sqlx::query_as::<_, ParityEntry>(
        r#"
        SELECT language AS "group",
               ROUND(
                   100.0 * SUM(CASE WHEN decision <> 'cleared_no_entities' THEN 1 ELSE 0 END)
                   / COUNT(*)::numeric,
                   1
               )::float8 AS flag_rate
        FROM audit_log
        WHERE organisation_id = $1 AND language IS NOT NULL
        GROUP BY language
        ORDER BY language
        "#,
    )
    .bind(org_id)
    .fetch_all(&state.db)
    .await?;

    let (dir_department, spd_department) = compute_dir_spd(&department_parity);
    let (dir_language, spd_language) = compute_dir_spd(&language_parity);

    Ok(Json(FairnessResponse {
        language_parity,
        department_parity,
        dir_language,
        spd_language,
        dir_department,
        spd_department,
        threshold: DIR_THRESHOLD,
    }))
}
