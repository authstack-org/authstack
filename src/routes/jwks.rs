use axum::{Json, Router, extract::State, routing::get};

use crate::{AppState, error::Result};

pub fn router() -> Router<AppState> {
    Router::new().route("/.well-known/jwks.json", get(jwks))
}

/// RFC 7517 JWKS for the ES256 signing key (P-256 `x`/`y`, `kid` aligned with JWS headers).
async fn jwks(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    Ok(Json(state.jwt.jwks().clone()))
}
