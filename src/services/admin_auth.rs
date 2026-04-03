use anyhow::Result;
use sqlx::PgPool;

use crate::ids::AdminUserId;
use crate::models::admin_user::AdminUser;
use crate::services::password;

pub async fn create_admin(db: &PgPool, email: &str, raw_password: &str) -> Result<AdminUser> {
    let password_hash = password::hash(raw_password)?;
    let id = AdminUserId::new();
    let row: AdminUser = sqlx::query_as(
        "INSERT INTO admin_user (id, email, password_hash) VALUES ($1, $2, $3)
         RETURNING id, email, password_hash, created_at, updated_at",
    )
    .bind(id)
    .bind(email)
    .bind(password_hash)
    .fetch_one(db)
    .await?;
    Ok(row)
}

pub async fn login_admin(db: &PgPool, email: &str, raw_password: &str) -> Result<Option<AdminUser>> {
    let row: Option<AdminUser> = sqlx::query_as(
        "SELECT id, email, password_hash, created_at, updated_at FROM admin_user WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(db)
    .await?;

    match row {
        Some(user) if password::verify(raw_password, &user.password_hash)? => Ok(Some(user)),
        _ => Ok(None),
    }
}
