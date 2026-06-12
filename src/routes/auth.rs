use axum::{Extension, Json, Router, extract::State, http::HeaderMap, routing::post};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use validator::Validate;

use crate::{
    AppState,
    error::{AppError, Result},
    ids::{OrganizationId, RefreshSessionId, UserId},
    middleware::app_auth::AppIdentity,
    models::organization::OrgType,
    routes::me,
    services::auth as auth_service,
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
        app.app_id,
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

    let result = auth_service::login(&state.db, app.app_id, &body.email, &body.password).await?;

    let jwt = &state.jwt;

    let org_type_str = format!("{:?}", result.org_type).to_lowercase();
    let access_token = jwt
        .issue_access_token(
            result.user.id,
            app.app_id,
            result.org_id,
            &org_type_str,
            &result.role,
            &result.user.email,
        )
        .map_err(AppError::Internal)?;

    let (refresh_token, jti) = jwt
        .issue_refresh_token(result.user.id, app.app_id)
        .map_err(|e| AppError::Internal(e))?;

    sqlx::query(
        "INSERT INTO refresh_session (id, user_id, jti, expires_at) VALUES ($1, $2, $3, NOW() + ($4 * interval '1 second'))",
    )
    .bind(RefreshSessionId::new())
    .bind(result.user.id)
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
    let jwt = &state.jwt;

    let token_data = jwt
        .verify_refresh_token(&body.refresh_token)
        .map_err(|_| AppError::Unauthorized("invalid or expired refresh token".to_string()))?;

    let claims = token_data.claims;

    if claims.app_id != app.app_id.to_string() {
        return Err(AppError::Unauthorized("token app mismatch".to_string()));
    }

    let session_id: Option<RefreshSessionId> = sqlx::query_scalar(
        "SELECT id FROM refresh_session WHERE jti = $1 AND revoked_at IS NULL AND expires_at > NOW()",
    )
    .bind(&claims.jti)
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

    let row: (UserId, String) = sqlx::query_as(
        r#"SELECT u.id, u.email
           FROM "user" u
           WHERE u.id = $1 AND u.app_id = $2"#,
    )
    .bind(user_id)
    .bind(app.app_id)
    .fetch_one(&state.db)
    .await
    .map_err(|_| AppError::Unauthorized("user not found".to_string()))?;

    let (uid, email) = row;

    let (org_id, org_type, role) = match body.org_id {
        Some(org_id) => {
            let (org_type, role) = me::membership_for_org(&state, app.app_id, uid, org_id).await?;
            (org_id, org_type, role)
        }
        None => {
            let row: (OrganizationId, OrgType, String) = sqlx::query_as(
                r#"SELECT m.organization_id, o.org_type, m.role
                   FROM member m
                   JOIN organization o ON o.id = m.organization_id
                   WHERE m.user_id = $1 AND o.app_id = $2 AND o.org_type = 'personal'"#,
            )
            .bind(uid)
            .bind(app.app_id)
            .fetch_one(&state.db)
            .await?;
            row
        }
    };

    let org_type_str = format!("{:?}", org_type).to_lowercase();
    let access_token = jwt
        .issue_access_token(uid, app.app_id, org_id, &org_type_str, &role, &email)
        .map_err(AppError::Internal)?;

    let (new_refresh_token, new_jti) = jwt
        .issue_refresh_token(uid, app.app_id)
        .map_err(AppError::Internal)?;

    sqlx::query(
        "INSERT INTO refresh_session (id, user_id, jti, expires_at) VALUES ($1, $2, $3, NOW() + ($4 * interval '1 second'))",
    )
    .bind(RefreshSessionId::new())
    .bind(uid)
    .bind(&new_jti)
    .bind(state.config.refresh_token_expiry_secs as f64)
    .execute(&state.db)
    .await?;

    Ok(Json(TokenResponse {
        access_token,
        refresh_token: new_refresh_token,
        token_type: "Bearer".to_string(),
    }))
}

async fn switch_org(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<SwitchOrgRequest>,
) -> Result<Json<TokenResponse>> {
    let user = me::authenticate_user(&state, &headers).await?;
    let (org_type, role) =
        me::membership_for_org(&state, user.app_id, user.user_id, body.org_id).await?;

    let email: String =
        sqlx::query_scalar(r#"SELECT email FROM "user" WHERE id = $1 AND app_id = $2"#)
            .bind(user.user_id)
            .bind(user.app_id)
            .fetch_optional(&state.db)
            .await?
            .ok_or_else(|| AppError::Unauthorized("user not found".to_string()))?;

    let org_type_str = format!("{:?}", org_type).to_lowercase();
    let access_token = state
        .jwt
        .issue_access_token(
            user.user_id,
            user.app_id,
            body.org_id,
            &org_type_str,
            &role,
            &email,
        )
        .map_err(AppError::Internal)?;
    let (refresh_token, jti) = state
        .jwt
        .issue_refresh_token(user.user_id, user.app_id)
        .map_err(AppError::Internal)?;

    sqlx::query(
        "INSERT INTO refresh_session (id, user_id, jti, expires_at) VALUES ($1, $2, $3, NOW() + ($4 * interval '1 second'))",
    )
    .bind(RefreshSessionId::new())
    .bind(user.user_id)
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
