use axum::{extract::State, Json};

use crate::{
    auth::{require_role, AuthUser},
    error::AppResult,
    models::{SecurityEvent, SecurityEventRow, SecurityEventsResponse},
    state::AppState,
};

pub async fn get_security_events(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<SecurityEventsResponse>> {
    require_role(&claims, &["compliance_admin"])?;

    let rows: Vec<SecurityEventRow> = sqlx::query_as::<_, SecurityEventRow>(
        "SELECT event_type, detail, created_at FROM security_events \
         WHERE organisation_id = $1 ORDER BY created_at DESC LIMIT 20",
    )
    .bind(claims.organisation_id)
    .fetch_all(&state.db)
    .await?;

    let events: Vec<SecurityEvent> = rows.into_iter().map(SecurityEvent::from).collect();

    Ok(Json(SecurityEventsResponse { events }))
}
