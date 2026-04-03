use axum::{
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Redirect, Response},
};

use crate::{error::AppError, ids::AdminUserId, AppState};

#[derive(Clone, Debug)]
pub struct AdminIdentity {
    pub admin_id: AdminUserId,
    pub email: String,
}

pub async fn authenticate_admin(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Response {
    let token = req
        .headers()
        .get("Cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|cookies| {
            cookies
                .split(';')
                .find_map(|c| c.trim().strip_prefix("admin_token=").map(str::to_string))
        });

    let token = match token {
        Some(t) => t,
        None => return Redirect::to("/admin/login").into_response(),
    };

    match state.jwt.verify_admin_token(&token) {
        Ok(data) => {
            let admin_id = match data.claims.sub.parse::<AdminUserId>() {
                Ok(id) => id,
                Err(_) => {
                    return AppError::Unauthorized("invalid admin token subject".to_string())
                        .into_response()
                }
            };
            req.extensions_mut().insert(AdminIdentity {
                admin_id,
                email: data.claims.email,
            });
            next.run(req).await
        }
        Err(_) => Redirect::to("/admin/login").into_response(),
    }
}
