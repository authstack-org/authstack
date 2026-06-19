use sqlx::PgPool;

use crate::error::{AppError, Result};
use crate::ids::{AccountId, OrganizationId, UserId};
use crate::models::user::User;
use crate::services::identity::{self, AppContext};
use crate::services::password;

pub async fn signup(
    db: &PgPool,
    ctx: &AppContext,
    name: &str,
    email: &str,
    password: &str,
) -> Result<User> {
    if identity::email_taken_for_signup(db, ctx, email).await? {
        return Err(AppError::Conflict("email already registered".to_string()));
    }

    let password_hash = password::hash(password).map_err(AppError::Internal)?;

    let mut tx = db.begin().await?;

    let user: User = sqlx::query_as(
        r#"INSERT INTO "user" (id, directory_id, name, email, email_verified)
           VALUES ($1, $2, $3, $4, false)
           RETURNING id, directory_id, name, email, email_verified, image, created_at, updated_at"#,
    )
    .bind(UserId::new())
    .bind(ctx.directory_id)
    .bind(name)
    .bind(email)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO account (id, provider_id, user_id, password_hash) VALUES ($1, 'credential', $2, $3)",
    )
    .bind(AccountId::new())
    .bind(user.id)
    .bind(password_hash)
    .execute(&mut *tx)
    .await?;

    identity::grant_app_access(&mut tx, user.id, ctx.application_id).await?;

    tx.commit().await?;

    Ok(user)
}

pub struct LoginResult {
    pub user: User,
    pub org: Option<(OrganizationId, String)>,
}

pub async fn login(
    db: &PgPool,
    ctx: &AppContext,
    email: &str,
    password: &str,
) -> Result<LoginResult> {
    let user = identity::find_user_for_login(db, ctx, email)
        .await?
        .ok_or_else(|| AppError::Unauthorized("invalid credentials".to_string()))?;

    let hash: Option<String> = sqlx::query_scalar(
        "SELECT password_hash FROM account WHERE user_id = $1 AND provider_id = 'credential'",
    )
    .bind(user.id)
    .fetch_optional(db)
    .await?
    .flatten();

    let hash = hash.ok_or_else(|| AppError::Unauthorized("invalid credentials".to_string()))?;

    let valid = password::verify(password, &hash).map_err(AppError::Internal)?;
    if !valid {
        return Err(AppError::Unauthorized("invalid credentials".to_string()));
    }

    let org = identity::find_primary_org_membership(db, ctx, user.id).await?;

    Ok(LoginResult { user, org })
}
