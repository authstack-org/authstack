use axum::{extract::State, routing::get, Json, Router};

use crate::{error::Result, AppState};

pub fn router() -> Router<AppState> {
    Router::new().route("/.well-known/jwks.json", get(jwks))
}

/// Returns the public key set so consuming services can verify JWTs locally.
async fn jwks(State(state): State<AppState>) -> Result<Json<serde_json::Value>> {
    // For ES256 the public key is an EC key — expose it as a JWK.
    // In production, parse the PEM and build a proper JWK. For now, return the PEM
    // wrapped so clients can use it directly.
    Ok(Json(serde_json::json!({
        "keys": [
            {
                "kty": "EC",
                "use": "sig",
                "alg": "ES256",
                "pem": state.config.jwt_public_key,
            }
        ]
    })))
}
