use axum::{
    extract::{Path, State},
    routing::get,
    Extension, Json, Router,
};

use crate::{
    error::{AppError, Result},
    ids::UserId,
    middleware::app_auth::AppIdentity,
    models::user::User,
    AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/users", get(list_users))
        .route("/users/{id}", get(get_user))
}

async fn list_users(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
) -> Result<Json<Vec<User>>> {
    let users: Vec<User> = sqlx::query_as(
        r#"SELECT id, app_id, name, email, email_verified, image, created_at, updated_at
           FROM "user" WHERE app_id = $1 ORDER BY created_at DESC"#,
    )
    .bind(app.app_id)
    .fetch_all(&state.db)
    .await?;

    Ok(Json(users))
}

async fn get_user(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(id): Path<String>,
) -> Result<Json<User>> {
    let user_id: UserId = id
        .parse()
        .map_err(|_| AppError::NotFound("user not found".to_string()))?;

    let user: Option<User> = sqlx::query_as(
        r#"SELECT id, app_id, name, email, email_verified, image, created_at, updated_at
           FROM "user" WHERE id = $1 AND app_id = $2"#,
    )
    .bind(user_id)
    .bind(app.app_id)
    .fetch_optional(&state.db)
    .await?;

    user.map(Json).ok_or_else(|| AppError::NotFound("user not found".to_string()))
}
