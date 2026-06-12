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
    ids::{ApplicationId, UserId},
    models::organization::{OrgType, Organization},
};

#[derive(Debug, Clone)]
pub struct UserIdentity {
    pub user_id: UserId,
    pub app_id: ApplicationId,
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

    Ok(UserIdentity { user_id, app_id })
}

async fn list_my_organizations(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<UserOrganization>>> {
    let user = authenticate_user(&state, &headers).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: crate::ids::OrganizationId,
        app_id: ApplicationId,
        name: String,
        slug: String,
        org_type: OrgType,
        logo: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
        role: String,
    }

    let orgs: Vec<Row> = sqlx::query_as(
        r#"SELECT o.id, o.app_id, o.name, o.slug, o.org_type, o.logo, o.created_at, o.updated_at, m.role
           FROM member m
           JOIN organization o ON o.id = m.organization_id
           WHERE m.user_id = $1 AND o.app_id = $2
           ORDER BY
             CASE WHEN o.org_type = 'personal' THEN 0 ELSE 1 END,
             o.created_at DESC"#,
    )
    .bind(user.user_id)
    .bind(user.app_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        orgs.into_iter()
            .map(|row| UserOrganization {
                organization: Organization {
                    id: row.id,
                    app_id: row.app_id,
                    name: row.name,
                    slug: row.slug,
                    org_type: row.org_type,
                    logo: row.logo,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                },
                role: row.role,
            })
            .collect(),
    ))
}

pub async fn membership_for_org(
    state: &AppState,
    app_id: ApplicationId,
    user_id: UserId,
    org_id: crate::ids::OrganizationId,
) -> Result<(OrgType, String)> {
    sqlx::query_as(
        r#"SELECT o.org_type, m.role
           FROM member m
           JOIN organization o ON o.id = m.organization_id
           WHERE m.user_id = $1 AND o.id = $2 AND o.app_id = $3"#,
    )
    .bind(user_id)
    .bind(org_id)
    .bind(app_id)
    .fetch_optional(&state.db)
    .await?
    .ok_or(AppError::Forbidden)
}
