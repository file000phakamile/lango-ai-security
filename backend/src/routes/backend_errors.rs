//! Real observability ("response scanning + observability + hardening"
//! task, Part 2) — reads what `src/observability.rs`'s middleware writes.
//!
//! **Known v1 scope limitation, stated explicitly rather than left
//! implicit**: this endpoint is gated to `compliance_admin` (the most
//! privileged role that already exists in this codebase) but is
//! deliberately NOT organisation-scoped — `backend_errors` has no
//! `organisation_id` column at all (see migration 0016's own comment on
//! why: an error can happen before any organisation is known). That means
//! today, ANY compliance_admin, in ANY organisation, can see every
//! organisation's backend error log. That's a reasonable simplification
//! for a single/few-tenant pilot where this is really an internal
//! diagnostic tool, not a compliance record — but it would need genuine
//! operator-only access control, distinct from any tenant's own admin
//! role, before onboarding organisations that shouldn't see each other's
//! operational signals. Not built here — a real role/permission system
//! addition is a larger change than this task's scope.
use axum::{extract::State, Json};

use crate::{
    auth::{require_role, AuthUser},
    error::AppResult,
    models::{BackendErrorEntry, BackendErrorsResponse},
    state::AppState,
};

pub async fn get_backend_errors(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
) -> AppResult<Json<BackendErrorsResponse>> {
    require_role(&claims, &["compliance_admin"])?;

    let errors: Vec<BackendErrorEntry> = sqlx::query_as(
        "SELECT id, method, path, status_code, message, created_at \
         FROM backend_errors ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(&state.db)
    .await?;

    Ok(Json(BackendErrorsResponse { errors }))
}
