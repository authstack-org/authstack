use sqlx::PgPool;

use crate::error::{AppError, Result as AppResult};
use crate::ids::{ApplicationId, DirectoryId, OrganizationId, UserId};
use crate::models::user::User;

#[derive(Debug, Clone)]
pub struct AppContext {
    pub application_id: ApplicationId,
    pub directory_id: DirectoryId,
}

pub async fn load_app_context(
    db: &PgPool,
    application_id: ApplicationId,
) -> AppResult<Option<AppContext>> {
    let directory_id: Option<DirectoryId> =
        sqlx::query_scalar("SELECT directory_id FROM application WHERE id = $1")
            .bind(application_id)
            .fetch_optional(db)
            .await?;

    Ok(directory_id.map(|directory_id| AppContext {
        application_id,
        directory_id,
    }))
}

pub async fn email_taken_for_signup(
    db: &PgPool,
    ctx: &AppContext,
    email: &str,
) -> AppResult<bool> {
    let exists: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1 FROM "user"
               WHERE directory_id = $1 AND email = $2
           )"#,
    )
    .bind(ctx.directory_id)
    .bind(email)
    .fetch_one(db)
    .await?;

    Ok(exists)
}

pub async fn find_user_for_login(
    db: &PgPool,
    ctx: &AppContext,
    email: &str,
) -> AppResult<Option<User>> {
    let user: Option<User> = sqlx::query_as(
        r#"SELECT id, directory_id, name, email, email_verified, image, created_at, updated_at
           FROM "user"
           WHERE directory_id = $1 AND email = $2"#,
    )
    .bind(ctx.directory_id)
    .bind(email)
    .fetch_optional(db)
    .await?;

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

/// First organization membership for the user in this application, if any.
pub async fn list_organizations_for_user(
    db: &PgPool,
    ctx: &AppContext,
    user_id: UserId,
) -> AppResult<Vec<(crate::models::organization::Organization, String, Vec<String>)>> {
    #[derive(sqlx::FromRow)]
    struct Row {
        id: OrganizationId,
        directory_id: DirectoryId,
        application_id: ApplicationId,
        name: String,
        slug: String,
        logo: Option<String>,
        created_at: chrono::DateTime<chrono::Utc>,
        updated_at: chrono::DateTime<chrono::Utc>,
        org_role_id: crate::ids::OrgRoleId,
        role: String,
    }

    let rows: Vec<Row> = sqlx::query_as(
        r#"SELECT o.id, o.directory_id, o.application_id, o.name, o.slug, o.logo,
                  o.created_at, o.updated_at, m.org_role_id, r.slug AS role
           FROM member m
           JOIN organization o ON o.id = m.organization_id
           JOIN org_role r ON r.id = m.org_role_id
           WHERE m.user_id = $1 AND o.application_id = $2
           ORDER BY o.created_at ASC"#,
    )
    .bind(user_id)
    .bind(ctx.application_id)
    .fetch_all(db)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let permissions =
            crate::services::roles::list_permission_keys_for_org_role(db, row.org_role_id).await?;
        out.push((
            crate::models::organization::Organization {
                id: row.id,
                directory_id: row.directory_id,
                application_id: row.application_id,
                name: row.name,
                slug: row.slug,
                logo: row.logo,
                created_at: row.created_at,
                updated_at: row.updated_at,
            },
            row.role,
            permissions,
        ));
    }

    Ok(out)
}

/// SQL fragment: organizations visible within an application context.
pub fn organization_visibility_clause() -> &'static str {
    "o.application_id = a.id"
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
               INNER JOIN application a ON a.id = $1
               WHERE o.id = $2 AND {}
           )"#,
        organization_visibility_clause()
    ))
    .bind(ctx.application_id)
    .bind(organization_id)
    .fetch_one(db)
    .await?;

    Ok(exists)
}
