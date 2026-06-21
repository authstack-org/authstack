use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::{delete, get, patch},
};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    AppState,
    error::{AppError, Result},
    ids::{AppPermissionId, OrgRoleId, OrganizationId},
    middleware::app_auth::AppIdentity,
    models::org_role::OrgRole,
    services::{identity, roles},
};

#[derive(Debug, Deserialize, Validate)]
pub struct CreateOrgRoleRequest {
    #[validate(length(min = 1))]
    pub slug: String,
    #[validate(length(min = 1))]
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub permission_ids: Vec<AppPermissionId>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateOrgRoleRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub permission_ids: Option<Vec<AppPermissionId>>,
}

#[derive(Debug, Serialize)]
pub struct OrgRoleDetail {
    #[serde(flatten)]
    pub role: OrgRole,
    pub permission_ids: Vec<AppPermissionId>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/orgs/{org_id}/roles",
            get(list_org_roles).post(create_org_role),
        )
        .route(
            "/orgs/{org_id}/roles/{role_id}",
            get(get_org_role)
                .patch(update_org_role)
                .delete(delete_org_role),
        )
}

async fn ensure_org_visible(
    state: &AppState,
    app: &AppIdentity,
    org_id: OrganizationId,
) -> Result<()> {
    if identity::organization_visible_to_app(&state.db, &app.ctx, &org_id.to_string()).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("organization not found".to_string()))
    }
}

async fn list_org_roles(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(org_id): Path<String>,
) -> Result<Json<Vec<OrgRoleDetail>>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;
    ensure_org_visible(&state, &app, org_id).await?;

    let org_roles = roles::list_org_roles(&state.db, org_id)
        .await
        .map_err(AppError::Internal)?;

    let mut out = Vec::with_capacity(org_roles.len());
    for role in org_roles {
        let permission_ids = roles::list_org_role_permission_ids(&state.db, role.id)
            .await
            .map_err(AppError::Internal)?;
        out.push(OrgRoleDetail {
            role,
            permission_ids,
        });
    }

    Ok(Json(out))
}

async fn get_org_role(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path((org_id, role_id)): Path<(String, String)>,
) -> Result<Json<OrgRoleDetail>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;
    let role_id: OrgRoleId = role_id
        .parse()
        .map_err(|_| AppError::NotFound("organization role not found".to_string()))?;
    ensure_org_visible(&state, &app, org_id).await?;

    let role = roles::get_org_role(&state.db, org_id, role_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("organization role not found".to_string()))?;

    let permission_ids = roles::list_org_role_permission_ids(&state.db, role.id)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(OrgRoleDetail {
        role,
        permission_ids,
    }))
}

async fn create_org_role(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(org_id): Path<String>,
    Json(body): Json<CreateOrgRoleRequest>,
) -> Result<Json<OrgRoleDetail>> {
    body.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;
    ensure_org_visible(&state, &app, org_id).await?;

    let role = roles::create_org_role(
        &state.db,
        org_id,
        &body.slug,
        &body.name,
        body.description.as_deref(),
        &body.permission_ids,
    )
    .await
    .map_err(|e| {
        if crate::error::is_unique_violation(e.as_ref()) {
            AppError::Conflict("role slug already exists in this organization".to_string())
        } else {
            AppError::Validation(e.to_string())
        }
    })?;

    Ok(Json(OrgRoleDetail {
        permission_ids: body.permission_ids,
        role,
    }))
}

async fn update_org_role(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path((org_id, role_id)): Path<(String, String)>,
    Json(body): Json<UpdateOrgRoleRequest>,
) -> Result<Json<OrgRoleDetail>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;
    let role_id: OrgRoleId = role_id
        .parse()
        .map_err(|_| AppError::NotFound("organization role not found".to_string()))?;
    ensure_org_visible(&state, &app, org_id).await?;

    let role = roles::update_org_role(
        &state.db,
        org_id,
        role_id,
        body.name.as_deref(),
        Some(body.description.as_deref()),
        body.permission_ids.as_deref(),
    )
    .await
    .map_err(|e| AppError::Validation(e.to_string()))?;

    let permission_ids = roles::list_org_role_permission_ids(&state.db, role.id)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(OrgRoleDetail {
        role,
        permission_ids,
    }))
}

async fn delete_org_role(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path((org_id, role_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;
    let role_id: OrgRoleId = role_id
        .parse()
        .map_err(|_| AppError::NotFound("organization role not found".to_string()))?;
    ensure_org_visible(&state, &app, org_id).await?;

    roles::delete_org_role(&state.db, org_id, role_id)
        .await
        .map_err(|e| AppError::Validation(e.to_string()))?;

    Ok(Json(serde_json::json!({ "ok": true })))
}
