use sqlx::PgPool;

use crate::error::{AppError, Result};
use crate::ids::{AccountId, ApplicationId, MemberId, OrganizationId, UserId};
use crate::models::{organization::OrgType, user::User};
use crate::services::password;

pub async fn signup(
    db: &PgPool,
    app_id: ApplicationId,
    name: &str,
    email: &str,
    password: &str,
) -> Result<User> {
    let existing: Option<UserId> = sqlx::query_scalar(
        r#"SELECT id FROM "user" WHERE app_id = $1 AND email = $2"#,
    )
    .bind(app_id)
    .bind(email)
    .fetch_optional(db)
    .await?;

    if existing.is_some() {
        return Err(AppError::Conflict("email already registered".to_string()));
    }

    let password_hash = password::hash(password).map_err(AppError::Internal)?;

    let mut tx = db.begin().await?;

    let user: User = sqlx::query_as(
        r#"INSERT INTO "user" (id, app_id, name, email, email_verified)
           VALUES ($1, $2, $3, $4, false)
           RETURNING id, app_id, name, email, email_verified, image, created_at, updated_at"#,
    )
    .bind(UserId::new())
    .bind(app_id)
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

    let org_slug = format!("{}-personal", user.id);
    let org_id: OrganizationId = sqlx::query_scalar(
        "INSERT INTO organization (id, app_id, name, slug, org_type) VALUES ($1, $2, $3, $4, 'personal') RETURNING id",
    )
    .bind(OrganizationId::new())
    .bind(app_id)
    .bind(format!("{}'s workspace", name))
    .bind(org_slug)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query(
        "INSERT INTO member (id, organization_id, user_id, role) VALUES ($1, $2, $3, 'owner')",
    )
    .bind(MemberId::new())
    .bind(org_id)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(user)
}

pub struct LoginResult {
    pub user: User,
    pub org_id: OrganizationId,
    pub org_type: OrgType,
    pub role: String,
}

pub async fn login(
    db: &PgPool,
    app_id: ApplicationId,
    email: &str,
    password: &str,
) -> Result<LoginResult> {
    let user: User = sqlx::query_as(
        r#"SELECT id, app_id, name, email, email_verified, image, created_at, updated_at
           FROM "user" WHERE app_id = $1 AND email = $2"#,
    )
    .bind(app_id)
    .bind(email)
    .fetch_optional(db)
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

    let row: (OrganizationId, OrgType, String) = sqlx::query_as(
        r#"SELECT m.organization_id, o.org_type, m.role
           FROM member m
           JOIN organization o ON o.id = m.organization_id
           WHERE m.user_id = $1 AND o.org_type = 'personal'"#,
    )
    .bind(user.id)
    .fetch_one(db)
    .await?;

    Ok(LoginResult {
        user,
        org_id: row.0,
        org_type: row.1,
        role: row.2,
    })
}
