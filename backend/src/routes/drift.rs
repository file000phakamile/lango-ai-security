use axum::{extract::State, Json};

use crate::{
    auth::{require_role, AuthUser},
    error::AppResult,
    models::{DriftResponse, DriftSnapshotRow, DriftWeek},
    state::AppState,
};

pub async fn get_drift(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<DriftResponse>> {
    require_role(&claims, &["compliance_admin"])?;

    let rows: Vec<DriftSnapshotRow> = sqlx::query_as::<_, DriftSnapshotRow>(
        "SELECT week_start, psi_score, kl_divergence_score FROM drift_snapshots \
         WHERE organisation_id = $1 ORDER BY week_start ASC",
    )
    .bind(claims.organisation_id)
    .fetch_all(&state.db)
    .await?;

    let weeks: Vec<DriftWeek> = rows
        .into_iter()
        .enumerate()
        .map(DriftWeek::from)
        .collect();

    Ok(Json(DriftResponse { weeks }))
}
