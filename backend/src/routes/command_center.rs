use axum::{extract::State, Json};

use crate::{
    auth::{require_role, AuthUser},
    error::AppResult,
    models::CommandCenterSummary,
    state::AppState,
};

pub async fn get_summary(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<CommandCenterSummary>> {
    require_role(&claims, &["compliance", "admin"])?;

    let sessions_scanned_today: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM audit_log WHERE \"timestamp\" >= date_trunc('day', now())",
    )
    .fetch_one(&state.db)
    .await?;

    let blocked_today: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM audit_log
        WHERE "timestamp" >= date_trunc('day', now())
          AND decision <> 'cleared_no_entities'
        "#,
    )
    .fetch_one(&state.db)
    .await?;

    let avg_risk_score: Option<f64> = sqlx::query_scalar(
        "SELECT AVG(risk_score)::float8 FROM audit_log WHERE \"timestamp\" >= date_trunc('day', now())",
    )
    .fetch_one(&state.db)
    .await?;

    // "Active alerts" = weeks currently over the PSI drift threshold, plus
    // any fairness group currently below the DIR threshold. Recomputed live
    // rather than stored, so it can't drift out of sync with the views that
    // show the underlying detail.
    let drift_alerts: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM drift_snapshots WHERE psi_score >= 0.20",
    )
    .fetch_one(&state.db)
    .await?;

    let fairness_alerts: i64 = sqlx::query_scalar(
        r#"
        WITH parity AS (
            SELECT department AS grp,
                   100.0 * SUM(CASE WHEN decision <> 'cleared_no_entities' THEN 1 ELSE 0 END) / COUNT(*) AS flag_rate
            FROM audit_log
            GROUP BY department
        )
        SELECT (CASE
            WHEN MAX(flag_rate) > 0 AND MIN(flag_rate) / MAX(flag_rate) < 0.80 THEN 1
            ELSE 0
        END)::bigint
        FROM parity
        "#,
    )
    .fetch_one(&state.db)
    .await?;

    Ok(Json(CommandCenterSummary {
        sessions_scanned_today,
        blocked_today,
        avg_risk_score: avg_risk_score.unwrap_or(0.0),
        active_alerts: drift_alerts + fairness_alerts,
    }))
}
