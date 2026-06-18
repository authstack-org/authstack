use sqlx::PgPool;

use crate::error::{AppError, Result};
use crate::ids::{AccountId, MemberId, OrganizationId, UserId};
use crate::models::organization::OrgType;
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

    let scoped_application_id = identity::org_application_scope(ctx);
    let org_application_id = scoped_application_id;

    let mut tx = db.begin().await?;

    let user: User = sqlx::query_as(
        r#"INSERT INTO "user" (id, directory_id, scoped_application_id, name, email, email_verified)
           VALUES ($1, $2, $3, $4, $5, false)
           RETURNING id, directory_id, scoped_application_id, name, email, email_verified, image, created_at, updated_at"#,
    )
    .bind(UserId::new())
    .bind(ctx.directory_id)
    .bind(scoped_application_id)
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

    let org_slug = format!("{}-personal", user.id);
    let org_id: OrganizationId = sqlx::query_scalar(
        r#"INSERT INTO organization (id, directory_id, application_id, name, slug, org_type)
           VALUES ($1, $2, $3, $4, $5, 'personal')
           RETURNING id"#,
    )
    .bind(OrganizationId::new())
    .bind(ctx.directory_id)
    .bind(org_application_id)
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

    let row: (OrganizationId, OrgType, String) =
        identity::find_personal_membership(db, ctx, user.id).await?;

    Ok(LoginResult {
        user,
        org_id: row.0,
        org_type: row.1,
        role: row.2,
    })
}
