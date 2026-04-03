use axum::{
    extract::{Path, State},
    routing::get,
    Extension, Json, Router,
};
use serde::Deserialize;
use validator::Validate;

use crate::{
    error::{AppError, Result},
    ids::OrganizationId,
    middleware::app_auth::AppIdentity,
    models::organization::Organization,
    AppState,
};

#[derive(Debug, Deserialize, Validate)]
pub struct CreateOrgRequest {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(length(min = 1))]
    pub slug: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs", get(list_orgs).post(create_org))
        .route("/orgs/{id}", get(get_org))
}

async fn list_orgs(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
) -> Result<Json<Vec<Organization>>> {
    let orgs: Vec<Organization> = sqlx::query_as(
        r#"SELECT id, app_id, name, slug, org_type, logo, created_at, updated_at
           FROM organization WHERE app_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(app.app_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(orgs))
}

async fn get_org(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(id): Path<String>,
) -> Result<Json<Organization>> {
    let org_id: OrganizationId = id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;

    let org: Option<Organization> = sqlx::query_as(
        "SELECT id, app_id, name, slug, org_type, logo, created_at, updated_at FROM organization WHERE id = $1 AND app_id = $2",
    )
    .bind(org_id)
    .bind(app.app_id)
    .fetch_optional(&state.db)
    .await?;

    org.map(Json).ok_or_else(|| AppError::NotFound("organization not found".to_string()))
}

async fn create_org(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Json(body): Json<CreateOrgRequest>,
) -> Result<Json<Organization>> {
    body.validate().map_err(|e| AppError::Validation(e.to_string()))?;

    let org: Organization = sqlx::query_as(
        r#"INSERT INTO organization (id, app_id, name, slug, org_type)
           VALUES ($1, $2, $3, $4, 'team')
           RETURNING id, app_id, name, slug, org_type, logo, created_at, updated_at"#,
    )
    .bind(OrganizationId::new())
    .bind(app.app_id)
    .bind(&body.name)
    .bind(&body.slug)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(org))
}
