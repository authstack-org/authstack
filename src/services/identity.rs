use sqlx::PgPool;

use crate::error::{AppError, Result as AppResult};
use crate::ids::{ApplicationId, DirectoryId, OrganizationId, UserId};
use crate::models::identity_policy::IdentityPolicy;
use crate::models::organization::OrgType;
use crate::models::user::User;


#[derive(Debug, Clone)]
pub struct AppContext {
    pub application_id: ApplicationId,
    pub directory_id: DirectoryId,
    pub identity_policy: IdentityPolicy,
}

pub async fn load_app_context(db: &PgPool, application_id: ApplicationId) -> AppResult<Option<AppContext>> {
    let row: Option<(DirectoryId, IdentityPolicy)> = sqlx::query_as(
        r#"SELECT a.directory_id, d.identity_policy
           FROM application a
           INNER JOIN directory d ON d.id = a.directory_id
           WHERE a.id = $1"#,
    )
    .bind(application_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|(directory_id, identity_policy)| AppContext {
        application_id,
        directory_id,
        identity_policy,
    }))
}

pub fn org_application_scope(ctx: &AppContext) -> Option<ApplicationId> {
    if ctx.identity_policy.is_shared() {
        None
    } else {
        Some(ctx.application_id)
    }
}

pub async fn email_taken_for_signup(
    db: &PgPool,
    ctx: &AppContext,
    email: &str,
) -> AppResult<bool> {
    let exists: bool = if ctx.identity_policy.is_shared() {
        sqlx::query_scalar(
            r#"SELECT EXISTS(
                   SELECT 1 FROM "user"
                   WHERE directory_id = $1
                     AND scoped_application_id IS NULL
                     AND email = $2
               )"#,
        )
        .bind(ctx.directory_id)
        .bind(email)
        .fetch_one(db)
        .await?
    } else {
        sqlx::query_scalar(
            r#"SELECT EXISTS(
                   SELECT 1 FROM "user"
                   WHERE directory_id = $1
                     AND scoped_application_id = $2
                     AND email = $3
               )"#,
        )
        .bind(ctx.directory_id)
        .bind(ctx.application_id)
        .bind(email)
        .fetch_one(db)
        .await?
    };

    Ok(exists)
}

pub async fn find_user_for_login(
    db: &PgPool,
    ctx: &AppContext,
    email: &str,
) -> AppResult<Option<User>> {
    let user: Option<User> = if ctx.identity_policy.is_shared() {
        sqlx::query_as(
            r#"SELECT id, directory_id, scoped_application_id, name, email, email_verified, image, created_at, updated_at
               FROM "user"
               WHERE directory_id = $1
                 AND scoped_application_id IS NULL
                 AND email = $2"#,
        )
        .bind(ctx.directory_id)
        .bind(email)
        .fetch_optional(db)
        .await?
    } else {
        sqlx::query_as(
            r#"SELECT id, directory_id, scoped_application_id, name, email, email_verified, image, created_at, updated_at
               FROM "user"
               WHERE directory_id = $1
                 AND scoped_application_id = $2
                 AND email = $3"#,
        )
        .bind(ctx.directory_id)
        .bind(ctx.application_id)
        .bind(email)
        .fetch_optional(db)
        .await?
    };

    let Some(user) = user else {
        return Ok(None);
    };

    if !user_has_app_access(db, user.id, ctx.application_id).await? {
        return Ok(None);
    }

    Ok(Some(user))
}

pub async fn user_has_app_access(
    db: &PgPool,
    user_id: UserId,
    application_id: ApplicationId,
) -> AppResult<bool> {
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1 FROM user_app_grant
               WHERE user_id = $1 AND application_id = $2
           )"#,
    )
    .bind(user_id)
    .bind(application_id)
    .fetch_one(db)
    .await?;

    Ok(exists)
}

pub async fn grant_app_access(
    db: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: UserId,
    application_id: ApplicationId,
) -> AppResult<()> {
    sqlx::query(
        r#"INSERT INTO user_app_grant (user_id, application_id)
           VALUES ($1, $2)
           ON CONFLICT DO NOTHING"#,
    )
    .bind(user_id)
    .bind(application_id)
    .execute(&mut **db)
    .await?;

    Ok(())
}

pub async fn user_visible_to_application(
    db: &PgPool,
    user_id: UserId,
    application_id: ApplicationId,
) -> AppResult<bool> {
    user_has_app_access(db, user_id, application_id).await
}

pub async fn get_default_directory_id(db: &PgPool) -> AppResult<DirectoryId> {
    let id: DirectoryId = sqlx::query_scalar("SELECT id FROM directory WHERE slug = 'default'")
        .fetch_optional(db)
        .await?
        .ok_or_else(|| AppError::Internal(anyhow::anyhow!("default directory is missing")))?;
    Ok(id)
}

pub async fn find_personal_membership(
    db: &PgPool,
    ctx: &AppContext,
    user_id: UserId,
) -> AppResult<(OrganizationId, OrgType, String)> {
    let row: (OrganizationId, OrgType, String) = if ctx.identity_policy.is_shared() {
        sqlx::query_as(
            r#"SELECT m.organization_id, o.org_type, m.role
               FROM member m
               JOIN organization o ON o.id = m.organization_id
               WHERE m.user_id = $1
                 AND o.org_type = 'personal'
                 AND o.application_id IS NULL
                 AND o.directory_id = $2"#,
        )
        .bind(user_id)
        .bind(ctx.directory_id)
        .fetch_one(db)
        .await?
    } else {
        sqlx::query_as(
            r#"SELECT m.organization_id, o.org_type, m.role
               FROM member m
               JOIN organization o ON o.id = m.organization_id
               WHERE m.user_id = $1
                 AND o.org_type = 'personal'
                 AND o.application_id = $2"#,
        )
        .bind(user_id)
        .bind(ctx.application_id)
        .fetch_one(db)
        .await?
    };

    Ok(row)
}

pub async fn list_organizations_for_user(
    db: &PgPool,
    ctx: &AppContext,
    user_id: UserId,
) -> AppResult<Vec<(crate::models::organization::Organization, String)>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: OrganizationId,
        directory_id: DirectoryId,
        application_id: Option<ApplicationId>,
        name: String,
        slug: String,
        org_type: OrgType,
        logo: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
        role: String,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"SELECT o.id, o.directory_id, o.application_id, o.name, o.slug, o.org_type, o.logo,
                  o.created_at, o.updated_at, m.role
           FROM member m
           JOIN organization o ON o.id = m.organization_id
           INNER JOIN directory d ON d.id = o.directory_id
           WHERE m.user_id = $3
             AND (
               (d.identity_policy = 'application_silo' AND o.application_id = $1)
               OR (d.identity_policy = 'shared_directory' AND o.application_id IS NULL AND o.directory_id = $2)
             )
           ORDER BY CASE WHEN o.org_type = 'personal' THEN 0 ELSE 1 END, o.created_at DESC"#,
    )
    .bind(ctx.application_id)
    .bind(ctx.directory_id)
    .bind(user_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                crate::models::organization::Organization {
                    id: row.id,
                    directory_id: row.directory_id,
                    application_id: row.application_id,
                    name: row.name,
                    slug: row.slug,
                    org_type: row.org_type,
                    logo: row.logo,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                },
                row.role,
            )
        })
        .collect())
}

pub async fn membership_for_org(
    db: &PgPool,
    ctx: &AppContext,
    user_id: UserId,
    org_id: OrganizationId,
) -> AppResult<(OrgType, String)> {
    let row: Option<(OrgType, String)> = sqlx::query_as(
        r#"SELECT o.org_type, m.role
           FROM member m
           JOIN organization o ON o.id = m.organization_id
           INNER JOIN directory d ON d.id = o.directory_id
           WHERE m.user_id = $3
             AND o.id = $4
             AND (
               (d.identity_policy = 'application_silo' AND o.application_id = $1)
               OR (d.identity_policy = 'shared_directory' AND o.application_id IS NULL AND o.directory_id = $2)
             )"#,
    )
    .bind(ctx.application_id)
    .bind(ctx.directory_id)
    .bind(user_id)
    .bind(org_id)
    .fetch_optional(db)
    .await?;

    row.ok_or(AppError::Forbidden)
}

/// SQL fragment: organizations visible within an application context.
pub fn organization_visibility_clause() -> &'static str {
    r#"(
        (d.identity_policy = 'application_silo' AND o.application_id = $1)
        OR (d.identity_policy = 'shared_directory' AND o.application_id IS NULL AND o.directory_id = $2)
    )"#
}

pub async fn organization_visible_to_app(
    db: &PgPool,
    ctx: &AppContext,
    organization_id: &str,
) -> AppResult<bool> {
    let exists: bool = sqlx::query_scalar(&format!(
        r#"SELECT EXISTS(
               SELECT 1
               FROM organization o
               INNER JOIN directory d ON d.id = o.directory_id
               WHERE o.id = $3 AND {}
           )"#,
        organization_visibility_clause()
    ))
    .bind(ctx.application_id)
    .bind(ctx.directory_id)
    .bind(organization_id)
    .fetch_one(db)
    .await?;

    Ok(exists)
}
