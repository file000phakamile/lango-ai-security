use axum::{extract::State, Json};

use crate::{
    auth::AuthUser,
    error::{AppError, AppResult},
    models::{ConsentAcceptRequest, ConsentAcceptResponse},
    state::AppState,
};

/// Records that the calling user has accepted their organisation's current
/// data-use consent policy. Part 4 of the multi-tenancy task — see
/// docs/SECURITY_PRIVACY.md's Consent row and docs/DEPLOYMENT_PLAN.md's
/// pilot checklist, both of which described this conceptually before now.
///
/// No role restriction — every role (staff included) must be able to accept
/// consent for themselves; this isn't a dashboard-reading endpoint.
pub async fn accept_consent(
    State(state): State<AppState>,
    AuthUser(claims): AuthUser,
    Json(payload): Json<ConsentAcceptRequest>,
) -> AppResult<Json<ConsentAcceptResponse>> {
    let org_version: String =
        sqlx::query_scalar("SELECT consent_policy_version FROM organisations WHERE id = $1")
            .bind(claims.organisation_id)
            .fetch_one(&state.db)
            .await?;

    // Guards against accepting a version that's no longer current — e.g.
    // the consent screen was shown, then the organisation bumped its policy
    // version before the user clicked accept. Reject explicitly rather than
    // silently recording acceptance of a policy text the user never
    // actually saw.
    if payload.policy_version != org_version {
        return Err(AppError::BadRequest(format!(
            "The consent policy has changed since it was shown to you (was '{}', now '{}'). \
             Please reload and review the current version before accepting.",
            payload.policy_version, org_version
        )));
    }

    sqlx::query("UPDATE users SET consent_accepted_at = now(), consent_policy_version = $1 WHERE id = $2")
        .bind(&org_version)
        .bind(claims.sub)
        .execute(&state.db)
        .await?;

    Ok(Json(ConsentAcceptResponse {
        accepted: true,
        policy_version: org_version,
    }))
}
