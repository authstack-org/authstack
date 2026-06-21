use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::{delete, get},
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    error::{AppError, Result},
    ids::{MemberId, OrgRoleId, OrganizationId, UserId},
    middleware::app_auth::AppIdentity,
    services::{identity, roles},
};

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: UserId,
    pub org_role_id: Option<OrgRoleId>,
    /// Role slug when org_role_id is omitted. Defaults to `member`.
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MemberResponse {
    pub id: MemberId,
    pub organization_id: OrganizationId,
    pub user_id: UserId,
    pub org_role_id: OrgRoleId,
    pub role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/orgs/{org_id}/members", get(list_members).post(add_member))
        .route("/orgs/{org_id}/members/{user_id}", delete(remove_member))
}

async fn list_members(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(org_id): Path<String>,
) -> Result<Json<Vec<MemberResponse>>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;

    ensure_org_visible(&state, &app.ctx, org_id).await?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: MemberId,
        organization_id: OrganizationId,
        user_id: UserId,
        org_role_id: OrgRoleId,
        role: String,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    let members: Vec<Row> = sqlx::query_as(
        r#"SELECT m.id, m.organization_id, m.user_id, m.org_role_id, r.slug AS role,
                  m.created_at, m.updated_at
           FROM member m
           INNER JOIN org_role r ON r.id = m.org_role_id
           WHERE m.organization_id = $1"#,
    )
    .bind(org_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(
        members
            .into_iter()
            .map(|m| MemberResponse {
                id: m.id,
                organization_id: m.organization_id,
                user_id: m.user_id,
                org_role_id: m.org_role_id,
                role: m.role,
                created_at: m.created_at,
                updated_at: m.updated_at,
            })
            .collect(),
    ))
}

async fn add_member(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(org_id): Path<String>,
    Json(body): Json<AddMemberRequest>,
) -> Result<Json<MemberResponse>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;

    ensure_org_visible(&state, &app.ctx, org_id).await?;

    if !identity::user_visible_to_application(&state.db, body.user_id, app.app_id).await? {
        return Err(AppError::NotFound("user not found".to_string()));
    }

    let org_role = roles::resolve_org_role(
        &state.db,
        org_id,
        body.org_role_id,
        body.role.as_deref(),
    )
    .await
    .map_err(|e| AppError::Validation(e.to_string()))?;

    #[derive(sqlx::FromRow)]
    struct Row {
        id: MemberId,
        organization_id: OrganizationId,
        user_id: UserId,
        org_role_id: OrgRoleId,
        role: String,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
    }

    let member: Row = sqlx::query_as(
        r#"INSERT INTO member (id, organization_id, user_id, org_role_id)
           VALUES ($1, $2, $3, $4)
           RETURNING id, organization_id, user_id, org_role_id, $5::text AS role, created_at, updated_at"#,
    )
    .bind(MemberId::new())
    .bind(org_id)
    .bind(body.user_id)
    .bind(org_role.id)
    .bind(&org_role.slug)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db_err) if db_err.code().as_deref() == Some("23505") => {
            AppError::Conflict("user is already a member of this organization".to_string())
        }
        other => AppError::Internal(other.into()),
    })?;

    Ok(Json(MemberResponse {
        id: member.id,
        organization_id: member.organization_id,
        user_id: member.user_id,
        org_role_id: member.org_role_id,
        role: member.role,
        created_at: member.created_at,
        updated_at: member.updated_at,
    }))
}

async fn remove_member(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path((org_id, user_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;
    let user_id: UserId = user_id
        .parse()
        .map_err(|_| AppError::NotFound("user not found".to_string()))?;

    ensure_org_visible(&state, &app.ctx, org_id).await?;

    sqlx::query("DELETE FROM member WHERE organization_id = $1 AND user_id = $2")
        .bind(org_id)
        .bind(user_id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn ensure_org_visible(
    state: &AppState,
    ctx: &identity::AppContext,
    org_id: OrganizationId,
) -> Result<()> {
    if identity::organization_visible_to_app(&state.db, ctx, &org_id.to_string()).await? {
        Ok(())
    } else {
        Err(AppError::NotFound("organization not found".to_string()))
    }
}
