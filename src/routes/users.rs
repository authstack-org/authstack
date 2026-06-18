use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    routing::get,
};

use crate::{
    AppState,
    error::{AppError, Result},
    ids::UserId,
    middleware::app_auth::AppIdentity,
    models::user::User,
    services::identity,
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
        r#"SELECT u.id, u.directory_id, u.scoped_application_id, u.name, u.email, u.email_verified, u.image, u.created_at, u.updated_at
           FROM "user" u
           INNER JOIN user_app_grant g ON g.user_id = u.id
           WHERE g.application_id = $1
           ORDER BY u.created_at DESC"#,
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

    if !identity::user_visible_to_application(&state.db, user_id, app.app_id).await? {
        return Err(AppError::NotFound("user not found".to_string()));
    }

    let user: Option<User> = sqlx::query_as(
        r#"SELECT id, directory_id, scoped_application_id, name, email, email_verified, image, created_at, updated_at
           FROM "user" WHERE id = $1"#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;

    user.map(Json)
        .ok_or_else(|| AppError::NotFound("user not found".to_string()))
}
