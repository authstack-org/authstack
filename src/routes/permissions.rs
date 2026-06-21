use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::{delete, get},
};
use serde::Deserialize;
use validator::Validate;

use crate::{
    AppState,
    error::{AppError, Result},
    ids::AppPermissionId,
    middleware::app_auth::AppIdentity,
    models::app_permission::AppPermission,
    services::roles,
};

#[derive(Debug, Deserialize, Validate)]
pub struct CreateAppPermissionRequest {
    #[validate(length(min = 1))]
    pub key: String,
    #[validate(length(min = 1))]
    pub name: String,
    pub description: Option<String>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/permissions", get(list_permissions).post(create_permission))
        .route("/permissions/{id}", get(get_permission).delete(delete_permission))
}

async fn list_permissions(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
) -> Result<Json<Vec<AppPermission>>> {
    let permissions = roles::list_app_permissions(&state.db, app.app_id)
        .await
        .map_err(AppError::Internal)?;
    Ok(Json(permissions))
}

async fn get_permission(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(id): Path<String>,
) -> Result<Json<AppPermission>> {
    let permission_id: AppPermissionId = id
        .parse()
        .map_err(|_| AppError::NotFound("permission not found".to_string()))?;

    let permission = roles::get_app_permission(&state.db, app.app_id, permission_id)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("permission not found".to_string()))?;

    Ok(Json(permission))
}

async fn create_permission(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Json(body): Json<CreateAppPermissionRequest>,
) -> Result<Json<AppPermission>> {
    body.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let permission = roles::create_app_permission(
        &state.db,
        app.app_id,
        &body.key,
        &body.name,
        body.description.as_deref(),
    )
    .await
    .map_err(|e| {
        if crate::error::is_unique_violation(e.as_ref()) {
            AppError::Conflict("permission key already exists".to_string())
        } else {
            AppError::Validation(e.to_string())
        }
    })?;

    Ok(Json(permission))
}

async fn delete_permission(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let permission_id: AppPermissionId = id
        .parse()
        .map_err(|_| AppError::NotFound("permission not found".to_string()))?;

    let deleted = roles::delete_app_permission(&state.db, app.app_id, permission_id)
        .await
        .map_err(AppError::Internal)?;

    if deleted {
        Ok(Json(serde_json::json!({ "ok": true })))
    } else {
        Err(AppError::NotFound("permission not found".to_string()))
    }
}
