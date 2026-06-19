use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::get,
};
use serde::Deserialize;
use validator::Validate;

use crate::{
    AppState,
    error::{AppError, Result},
    ids::OrganizationId,
    middleware::app_auth::AppIdentity,
    models::organization::Organization,
    services::identity,
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
        r#"SELECT o.id, o.directory_id, o.application_id, o.name, o.slug, o.logo, o.created_at, o.updated_at
           FROM organization o
           WHERE o.application_id = $1
           ORDER BY o.created_at DESC"#,
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

    if !identity::organization_visible_to_app(&state.db, &app.ctx, &org_id.to_string()).await? {
        return Err(AppError::NotFound("organization not found".to_string()));
    }

    let org: Option<Organization> = sqlx::query_as(
        r#"SELECT id, directory_id, application_id, name, slug, logo, created_at, updated_at
           FROM organization WHERE id = $1"#,
    )
    .bind(org_id)
    .fetch_optional(&state.db)
    .await?;

    org.map(Json)
        .ok_or_else(|| AppError::NotFound("organization not found".to_string()))
}

async fn create_org(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Json(body): Json<CreateOrgRequest>,
) -> Result<Json<Organization>> {
    body.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let org: Organization = sqlx::query_as(
        r#"INSERT INTO organization (id, directory_id, application_id, name, slug)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, directory_id, application_id, name, slug, logo, created_at, updated_at"#,
    )
    .bind(OrganizationId::new())
    .bind(app.ctx.directory_id)
    .bind(app.app_id)
    .bind(&body.name)
    .bind(&body.slug)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.code().as_deref() == Some("23505") => {
            AppError::Conflict("organization slug already taken".to_string())
        }
        other => AppError::Internal(other.into()),
    })?;

    Ok(Json(org))
}
