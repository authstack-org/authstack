use axum::{
    extract::State,
    http::HeaderMap,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use crate::{
    error::{AppError, Result},
    services::password,
    AppState,
};

#[derive(Debug, Deserialize, Validate)]
pub struct CreateApplicationRequest {
    #[validate(length(min = 1))]
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct CreateApplicationResponse {
    pub id: Uuid,
    pub client_id: String,
    /// Plaintext secret — only returned at creation time, never stored.
    pub client_secret: String,
    pub name: String,
}

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/applications", post(create_application))
}

async fn create_application(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateApplicationRequest>,
) -> Result<Json<CreateApplicationResponse>> {
    body.validate().map_err(|e| AppError::Validation(e.to_string()))?;

    let provided_key = headers
        .get("X-Admin-Key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided_key != state.config.admin_key {
        return Err(AppError::Unauthorized("invalid admin key".to_string()));
    }

    let client_id = format!("app_{}", &Uuid::new_v4().to_string().replace('-', "")[..16]);
    let client_secret = format!("secret_{}", &Uuid::new_v4().to_string().replace('-', "")[..32]);

    let secret_hash = password::hash(&client_secret).map_err(AppError::Internal)?;

    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO application (id, client_id, client_secret_hash, name) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(&client_id)
    .bind(&secret_hash)
    .bind(&body.name)
    .execute(&state.db)
    .await?;

    Ok(Json(CreateApplicationResponse {
        id,
        client_id,
        client_secret,
        name: body.name,
    }))
}
