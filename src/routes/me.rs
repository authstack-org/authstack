use axum::{
    Json, Router,
    extract::State,
    http::{HeaderMap, header},
    routing::get,
};
use serde::Serialize;

use crate::{
    AppState,
    error::{AppError, Result},
    ids::{ApplicationId, DirectoryId, UserId},
    models::organization::Organization,
    services::identity::{self, AppContext},
};

#[derive(Debug, Clone)]
pub struct UserIdentity {
    pub user_id: UserId,
    pub app_id: ApplicationId,
    pub directory_id: DirectoryId,
    pub ctx: AppContext,
}

#[derive(Debug, Serialize)]
pub struct UserOrganization {
    pub organization: Organization,
    pub role: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/me/organizations", get(list_my_organizations))
}

pub async fn authenticate_user(state: &AppState, headers: &HeaderMap) -> Result<UserIdentity> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| AppError::Unauthorized("missing bearer token".to_string()))?;

    let claims = state
        .jwt
        .verify_access_token(token)
        .map_err(|_| AppError::Unauthorized("invalid or expired access token".to_string()))?
        .claims;

    let user_id = claims
        .sub
        .parse()
        .map_err(|_| AppError::Unauthorized("invalid user id in token".to_string()))?;
    let app_id = claims
        .app_id
        .parse()
        .map_err(|_| AppError::Unauthorized("invalid app id in token".to_string()))?;
    let directory_id = claims
        .directory_id
        .parse()
        .map_err(|_| AppError::Unauthorized("invalid directory id in token".to_string()))?;

    let ctx = identity::load_app_context(&state.db, app_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("invalid app id in token".to_string()))?;

    Ok(UserIdentity {
        user_id,
        app_id,
        directory_id,
        ctx,
    })
}

async fn list_my_organizations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserOrganization>>> {
    let user = authenticate_user(&state, &headers).await?;

    let orgs = identity::list_organizations_for_user(&state.db, &user.ctx, user.user_id).await?;

    Ok(Json(
        orgs.into_iter()
            .map(|(organization, role)| UserOrganization { organization, role })
            .collect(),
    ))
}

pub async fn membership_for_org(
    state: &AppState,
    app_id: ApplicationId,
    user_id: UserId,
    org_id: crate::ids::OrganizationId,
) -> Result<(crate::models::organization::OrgType, String)> {
    let ctx = identity::load_app_context(&state.db, app_id)
        .await?
        .ok_or_else(|| AppError::Unauthorized("invalid application".to_string()))?;

    identity::membership_for_org(&state.db, &ctx, user_id, org_id).await
}
