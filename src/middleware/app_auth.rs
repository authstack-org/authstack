use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use base64::{engine::general_purpose::STANDARD, Engine};

use crate::{error::AppError, ids::ApplicationId, services::password, AppState};

#[derive(Clone, Debug)]
pub struct AppIdentity {
    pub app_id: ApplicationId,
}

pub async fn authenticate_app(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("missing Authorization header".to_string()))?;

    let encoded = auth_header
        .strip_prefix("Basic ")
        .ok_or_else(|| AppError::Unauthorized("expected Basic auth".to_string()))?;

    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| AppError::Unauthorized("invalid base64 in Authorization".to_string()))?;

    let credentials = String::from_utf8(decoded)
        .map_err(|_| AppError::Unauthorized("invalid utf8 in Authorization".to_string()))?;

    let (app_id_str, client_secret) = credentials
        .split_once(':')
        .ok_or_else(|| AppError::Unauthorized("malformed Basic credentials".to_string()))?;

    let app_id: ApplicationId = app_id_str
        .parse()
        .map_err(|_| AppError::Unauthorized("invalid application id".to_string()))?;

    let row: Option<(ApplicationId, String)> = sqlx::query_as(
        "SELECT id, client_secret_hash FROM application WHERE id = $1",
    )
    .bind(app_id)
    .fetch_optional(&state.db)
    .await?;

    let (app_id, secret_hash) = row
        .ok_or_else(|| AppError::Unauthorized("invalid client credentials".to_string()))?;

    let valid = password::verify(client_secret, &secret_hash).map_err(AppError::Internal)?;

    if !valid {
        return Err(AppError::Unauthorized("invalid client credentials".to_string()));
    }

    req.extensions_mut().insert(AppIdentity { app_id });
    Ok(next.run(req).await)
}
