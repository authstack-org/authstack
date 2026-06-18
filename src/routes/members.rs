use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::{delete, get},
};
use serde::Deserialize;

use crate::{
    AppState,
    error::{AppError, Result},
    ids::{ApplicationId, MemberId, OrganizationId, UserId},
    middleware::app_auth::AppIdentity,
    models::member::Member,
};

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: UserId,
    pub role: Option<String>,
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
) -> Result<Json<Vec<Member>>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;

    ensure_org_belongs_to_app(&state, org_id, app.app_id).await?;

    let members: Vec<Member> = sqlx::query_as(
        "SELECT id, organization_id, user_id, role, created_at, updated_at FROM member WHERE organization_id = $1",
    )
    .bind(org_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(members))
}

async fn add_member(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(org_id): Path<String>,
    Json(body): Json<AddMemberRequest>,
) -> Result<Json<Member>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;

    ensure_org_belongs_to_app(&state, org_id, app.app_id).await?;
    ensure_team_org(&state, org_id, app.app_id).await?;

    let user_exists: Option<UserId> =
        sqlx::query_scalar(r#"SELECT id FROM "user" WHERE id = $1 AND app_id = $2"#)
            .bind(body.user_id)
            .bind(app.app_id)
            .fetch_optional(&state.db)
            .await?;

    if user_exists.is_none() {
        return Err(AppError::NotFound("user not found".to_string()));
    }

    let role = body.role.unwrap_or_else(|| "member".to_string());

    let member: Member = sqlx::query_as(
        "INSERT INTO member (id, organization_id, user_id, role) VALUES ($1, $2, $3, $4) RETURNING id, organization_id, user_id, role, created_at, updated_at",
    )
    .bind(MemberId::new())
    .bind(org_id)
    .bind(body.user_id)
    .bind(role)
    .fetch_one(&state.db)
    .await?;

    Ok(Json(member))
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

    ensure_org_belongs_to_app(&state, org_id, app.app_id).await?;

    sqlx::query("DELETE FROM member WHERE organization_id = $1 AND user_id = $2")
        .bind(org_id)
        .bind(user_id)
        .execute(&state.db)
        .await?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn ensure_org_belongs_to_app(
    state: &AppState,
    org_id: OrganizationId,
    app_id: ApplicationId,
) -> Result<()> {
    let exists: Option<OrganizationId> =
        sqlx::query_scalar("SELECT id FROM organization WHERE id = $1 AND app_id = $2")
            .bind(org_id)
            .bind(app_id)
            .fetch_optional(&state.db)
            .await?;

    exists
        .map(|_| ())
        .ok_or_else(|| AppError::NotFound("organization not found".to_string()))
}

async fn ensure_team_org(
    state: &AppState,
    org_id: OrganizationId,
    app_id: ApplicationId,
) -> Result<()> {
    let org_type: Option<String> = sqlx::query_scalar(
        "SELECT org_type::text FROM organization WHERE id = $1 AND app_id = $2",
    )
    .bind(org_id)
    .bind(app_id)
    .fetch_optional(&state.db)
    .await?;

    match org_type.as_deref() {
        Some("team") => Ok(()),
        Some(_) => Err(AppError::Validation(
            "members can only be added to team organizations".to_string(),
        )),
        None => Err(AppError::NotFound("organization not found".to_string())),
    }
}
