use axum::{Extension, Json, Router, extract::State, http::HeaderMap, routing::post};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    AppState,
    error::{AppError, Result},
    ids::{OrganizationId, RefreshSessionId, UserId},
    middleware::app_auth::AppIdentity,
    routes::me,
    services::{auth as auth_service, identity},
};

#[derive(Debug, Deserialize, Validate)]
pub struct SignupRequest {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 8))]
    pub password: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoginRequest {
    #[validate(email)]
    pub email: String,
    #[validate(length(min = 1))]
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
    pub org_id: Option<OrganizationId>,
}

#[derive(Debug, Deserialize)]
pub struct SwitchOrgRequest {
    pub org_id: OrganizationId,
}

#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/signup", post(signup))
        .route("/auth/login", post(login))
        .route("/auth/refresh", post(refresh))
        .route("/auth/logout", post(logout))
}

pub fn bearer_router() -> Router<AppState> {
    Router::new().route("/auth/switch-org", post(switch_org))
}

async fn signup(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Json(body): Json<SignupRequest>,
) -> Result<Json<serde_json::Value>> {
    body.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let user = auth_service::signup(
        &state.db,
        &app.ctx,
        &body.name,
        &body.email,
        &body.password,
    )
    .await?;

    Ok(Json(
        serde_json::json!({ "id": user.id, "email": user.email, "name": user.name }),
    ))
}

async fn login(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<TokenResponse>> {
    body.validate()
        .map_err(|e| AppError::Validation(e.to_string()))?;

    let result = auth_service::login(&state.db, &app.ctx, &body.email, &body.password).await?;

    issue_tokens(
        &state,
        &app.ctx,
        result.user.id,
        result.org,
        &result.user.email,
    )
    .await
}

async fn issue_tokens(
    state: &AppState,
    ctx: &identity::AppContext,
    user_id: UserId,
    org: Option<(OrganizationId, String)>,
    email: &str,
) -> Result<Json<TokenResponse>> {
    let (org_id, role) = match org {
        Some((org_id, role)) => (Some(org_id), Some(role)),
        None => (None, None),
    };

    let access_token = state
        .jwt
        .issue_access_token(
            user_id,
            ctx.directory_id,
            ctx.application_id,
            org_id,
            role.as_deref(),
            email,
        )
        .map_err(AppError::Internal)?;

    let (refresh_token, jti) = state
        .jwt
        .issue_refresh_token(user_id, ctx.application_id)
        .map_err(AppError::Internal)?;

    sqlx::query(
        r#"INSERT INTO refresh_session (id, user_id, application_id, jti, expires_at)
           VALUES ($1, $2, $3, $4, NOW() + ($5 * interval '1 second'))"#,
    )
    .bind(RefreshSessionId::new())
    .bind(user_id)
    .bind(ctx.application_id)
    .bind(&jti)
    .bind(state.config.refresh_token_expiry_secs as f64)
    .execute(&state.db)
    .await?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
    }))
}

async fn refresh(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<TokenResponse>> {
    let claims = state
        .jwt
        .verify_refresh_token(&body.refresh_token)
        .map_err(|_| AppError::Unauthorized("invalid or expired refresh token".to_string()))?
        .claims;

    if claims.app_id != app.app_id.to_string() {
        return Err(AppError::Unauthorized("token app mismatch".to_string()));
    }

    let session_id: Option<RefreshSessionId> = sqlx::query_scalar(
        r#"SELECT id FROM refresh_session
           WHERE jti = $1 AND application_id = $2 AND revoked_at IS NULL AND expires_at > NOW()"#,
    )
    .bind(&claims.jti)
    .bind(app.app_id)
    .fetch_optional(&state.db)
    .await?;

    let session_id = session_id
        .ok_or_else(|| AppError::Unauthorized("refresh token revoked or expired".to_string()))?;

    sqlx::query("UPDATE refresh_session SET revoked_at = NOW() WHERE id = $1")
        .bind(session_id)
        .execute(&state.db)
        .await?;

    let user_id: UserId = claims
        .sub
        .parse()
        .map_err(|_| AppError::Unauthorized("invalid token".to_string()))?;

    if !identity::user_has_app_access(&state.db, user_id, app.app_id).await? {
        return Err(AppError::Unauthorized("user not found".to_string()));
    }

    let email: String = sqlx::query_scalar(r#"SELECT email FROM "user" WHERE id = $1"#)
        .bind(user_id)
        .fetch_one(&state.db)
        .await
        .map_err(|_| AppError::Unauthorized("user not found".to_string()))?;

    let org = match body.org_id {
        Some(org_id) => {
            let role = me::membership_for_org(&state, app.app_id, user_id, org_id).await?;
            Some((org_id, role))
        }
        None => identity::find_primary_org_membership(&state.db, &app.ctx, user_id).await?,
    };

    issue_tokens(&state, &app.ctx, user_id, org, &email).await
}

async fn switch_org(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SwitchOrgRequest>,
) -> Result<Json<TokenResponse>> {
    let user = me::authenticate_user(&state, &headers).await?;
    let role = me::membership_for_org(&state, user.app_id, user.user_id, body.org_id).await?;

    let email: String = sqlx::query_scalar(r#"SELECT email FROM "user" WHERE id = $1"#)
        .bind(user.user_id)
        .fetch_optional(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("user not found".to_string()))?;

    issue_tokens(
        &state,
        &user.ctx,
        user.user_id,
        Some((body.org_id, role)),
        &email,
    )
    .await
}

async fn logout(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<serde_json::Value>> {
    if let Some(jti) = extract_jti_unverified(&body.refresh_token) {
        sqlx::query("UPDATE refresh_session SET revoked_at = NOW() WHERE jti = $1")
            .bind(jti)
            .execute(&state.db)
            .await?;
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

fn extract_jti_unverified(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let decoded = URL_SAFE_NO_PAD.decode(payload).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&decoded).ok()?;
    json.get("jti")?.as_str().map(|s| s.to_string())
}
