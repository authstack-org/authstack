use anyhow::{Context, Result};
use sqlx::{PgPool, Postgres, Transaction};

use crate::ids::{
    AppPermissionId, ApplicationId, OrgRoleId, OrganizationId, UserId,
};
use crate::models::app_permission::AppPermission;
use crate::models::org_role::OrgRole;

pub const DEFAULT_OWNER_SLUG: &str = "owner";
pub const DEFAULT_MEMBER_SLUG: &str = "member";

#[derive(Debug, Clone)]
pub struct OrgMembershipContext {
    pub org_id: OrganizationId,
    pub org_role_id: OrgRoleId,
    pub role_slug: String,
    pub permissions: Vec<String>,
}

pub fn validate_permission_key(key: &str) -> Result<()> {
    let key = key.trim();
    if key.is_empty() {
        anyhow::bail!("permission key is required");
    }
    if key.len() > 128 {
        anyhow::bail!("permission key must be at most 128 characters");
    }
    let valid = key
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == ':' || c == '_' || c == '-');
    if !valid || !key.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        anyhow::bail!(
            "permission key must start with a lowercase letter and contain only lowercase letters, digits, colons, underscores, or hyphens"
        );
    }
    Ok(())
}

pub fn validate_role_slug(slug: &str) -> Result<()> {
    let slug = slug.trim();
    if slug.is_empty() {
        anyhow::bail!("role slug is required");
    }
    if slug.len() > 64 {
        anyhow::bail!("role slug must be at most 64 characters");
    }
    let valid = slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_');
    if !valid || !slug.chars().next().is_some_and(|c| c.is_ascii_lowercase()) {
        anyhow::bail!(
            "role slug must start with a lowercase letter and contain only lowercase letters, digits, hyphens, or underscores"
        );
    }
    Ok(())
}

pub async fn list_app_permissions(
    db: &PgPool,
    application_id: ApplicationId,
) -> Result<Vec<AppPermission>> {
    let rows: Vec<AppPermission> = sqlx::query_as(
        r#"SELECT id, application_id, key, name, description, created_at, updated_at
           FROM app_permission
           WHERE application_id = $1
           ORDER BY key ASC"#,
    )
    .bind(application_id)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn get_app_permission(
    db: &PgPool,
    application_id: ApplicationId,
    permission_id: AppPermissionId,
) -> Result<Option<AppPermission>> {
    let row: Option<AppPermission> = sqlx::query_as(
        r#"SELECT id, application_id, key, name, description, created_at, updated_at
           FROM app_permission
           WHERE id = $1 AND application_id = $2"#,
    )
    .bind(permission_id)
    .bind(application_id)
    .fetch_optional(db)
    .await?;
    Ok(row)
}

pub async fn create_app_permission(
    db: &PgPool,
    application_id: ApplicationId,
    key: &str,
    name: &str,
    description: Option<&str>,
) -> Result<AppPermission> {
    let key = key.trim();
    let name = name.trim();
    validate_permission_key(key)?;
    if name.is_empty() {
        anyhow::bail!("permission name is required");
    }

    let id = AppPermissionId::new();
    let description = description.map(str::trim).filter(|d| !d.is_empty());

    let mut tx = db.begin().await?;

    let permission: AppPermission = sqlx::query_as(
        r#"INSERT INTO app_permission (id, application_id, key, name, description)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, application_id, key, name, description, created_at, updated_at"#,
    )
    .bind(id)
    .bind(application_id)
    .bind(key)
    .bind(name)
    .bind(description)
    .fetch_one(&mut *tx)
    .await
    .context("failed to create app permission")?;

    sqlx::query(
        r#"INSERT INTO org_role_permission (org_role_id, app_permission_id)
           SELECT r.id, $1
           FROM org_role r
           INNER JOIN organization o ON o.id = r.organization_id
           WHERE o.application_id = $2 AND r.slug = $3
           ON CONFLICT DO NOTHING"#,
    )
    .bind(permission.id)
    .bind(application_id)
    .bind(DEFAULT_OWNER_SLUG)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(permission)
}

pub async fn delete_app_permission(
    db: &PgPool,
    application_id: ApplicationId,
    permission_id: AppPermissionId,
) -> Result<bool> {
    let result = sqlx::query("DELETE FROM app_permission WHERE id = $1 AND application_id = $2")
        .bind(permission_id)
        .bind(application_id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn seed_default_org_roles(
    tx: &mut Transaction<'_, Postgres>,
    organization_id: OrganizationId,
) -> Result<()> {
    for (slug, name) in [
        (DEFAULT_OWNER_SLUG, "Owner"),
        (DEFAULT_MEMBER_SLUG, "Member"),
    ] {
        sqlx::query(
            r#"INSERT INTO org_role (id, organization_id, slug, name)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(OrgRoleId::new())
        .bind(organization_id)
        .bind(slug)
        .bind(name)
        .execute(&mut **tx)
        .await?;
    }
    Ok(())
}

pub async fn list_org_roles(db: &PgPool, organization_id: OrganizationId) -> Result<Vec<OrgRole>> {
    let rows: Vec<OrgRole> = sqlx::query_as(
        r#"SELECT id, organization_id, slug, name, description, created_at, updated_at
           FROM org_role
           WHERE organization_id = $1
           ORDER BY created_at ASC"#,
    )
    .bind(organization_id)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn get_org_role(
    db: &PgPool,
    organization_id: OrganizationId,
    org_role_id: OrgRoleId,
) -> Result<Option<OrgRole>> {
    let row: Option<OrgRole> = sqlx::query_as(
        r#"SELECT id, organization_id, slug, name, description, created_at, updated_at
           FROM org_role
           WHERE id = $1 AND organization_id = $2"#,
    )
    .bind(org_role_id)
    .bind(organization_id)
    .fetch_optional(db)
    .await?;
    Ok(row)
}

pub async fn resolve_org_role(
    db: &PgPool,
    organization_id: OrganizationId,
    org_role_id: Option<OrgRoleId>,
    role_slug: Option<&str>,
) -> Result<OrgRole> {
    if let Some(id) = org_role_id {
        return get_org_role(db, organization_id, id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("organization role not found"));
    }

    let slug = role_slug.unwrap_or(DEFAULT_MEMBER_SLUG);
    validate_role_slug(slug)?;

    let row: Option<OrgRole> = sqlx::query_as(
        r#"SELECT id, organization_id, slug, name, description, created_at, updated_at
           FROM org_role
           WHERE organization_id = $1 AND slug = $2"#,
    )
    .bind(organization_id)
    .bind(slug)
    .fetch_optional(db)
    .await?;

    row.ok_or_else(|| anyhow::anyhow!("organization role not found"))
}

pub async fn create_org_role(
    db: &PgPool,
    organization_id: OrganizationId,
    slug: &str,
    name: &str,
    description: Option<&str>,
    permission_ids: &[AppPermissionId],
) -> Result<OrgRole> {
    let slug = slug.trim();
    let name = name.trim();
    validate_role_slug(slug)?;
    if name.is_empty() {
        anyhow::bail!("role name is required");
    }

    let description = description.map(str::trim).filter(|d| !d.is_empty());
    let id = OrgRoleId::new();

    let mut tx = db.begin().await?;

    let role: OrgRole = sqlx::query_as(
        r#"INSERT INTO org_role (id, organization_id, slug, name, description)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, organization_id, slug, name, description, created_at, updated_at"#,
    )
    .bind(id)
    .bind(organization_id)
    .bind(slug)
    .bind(name)
    .bind(description)
    .fetch_one(&mut *tx)
    .await
    .context("failed to create org role")?;

    set_org_role_permissions(&mut tx, organization_id, role.id, permission_ids).await?;
    tx.commit().await?;
    Ok(role)
}

pub async fn update_org_role(
    db: &PgPool,
    organization_id: OrganizationId,
    org_role_id: OrgRoleId,
    name: Option<&str>,
    description: Option<Option<&str>>,
    permission_ids: Option<&[AppPermissionId]>,
) -> Result<OrgRole> {
    let existing = get_org_role(db, organization_id, org_role_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("organization role not found"))?;

    let name = name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .unwrap_or(&existing.name);

    let mut tx = db.begin().await?;

    let role: OrgRole = sqlx::query_as(
        r#"UPDATE org_role
           SET name = $1,
               description = COALESCE($2, description),
               updated_at = NOW()
           WHERE id = $3 AND organization_id = $4
           RETURNING id, organization_id, slug, name, description, created_at, updated_at"#,
    )
    .bind(name)
    .bind(description.flatten())
    .bind(org_role_id)
    .bind(organization_id)
    .fetch_one(&mut *tx)
    .await?;

    if let Some(ids) = permission_ids {
        set_org_role_permissions(&mut tx, organization_id, org_role_id, ids).await?;
    }

    tx.commit().await?;
    Ok(role)
}

async fn set_org_role_permissions(
    tx: &mut Transaction<'_, Postgres>,
    organization_id: OrganizationId,
    org_role_id: OrgRoleId,
    permission_ids: &[AppPermissionId],
) -> Result<()> {
    if permission_ids.is_empty() {
        sqlx::query("DELETE FROM org_role_permission WHERE org_role_id = $1")
            .bind(org_role_id)
            .execute(&mut **tx)
            .await?;
        return Ok(());
    }

    let app_id: ApplicationId = sqlx::query_scalar(
        "SELECT application_id FROM organization WHERE id = $1",
    )
    .bind(organization_id)
    .fetch_one(&mut **tx)
    .await?;

    for permission_id in permission_ids {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM app_permission WHERE id = $1 AND application_id = $2)",
        )
        .bind(permission_id)
        .bind(app_id)
        .fetch_one(&mut **tx)
        .await?;
        if !valid {
            anyhow::bail!("permission {permission_id} does not belong to this application");
        }
    }

    sqlx::query("DELETE FROM org_role_permission WHERE org_role_id = $1")
        .bind(org_role_id)
        .execute(&mut **tx)
        .await?;

    for permission_id in permission_ids {
        sqlx::query(
            "INSERT INTO org_role_permission (org_role_id, app_permission_id) VALUES ($1, $2)",
        )
        .bind(org_role_id)
        .bind(permission_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub async fn list_org_role_permission_ids(
    db: &PgPool,
    org_role_id: OrgRoleId,
) -> Result<Vec<AppPermissionId>> {
    let rows: Vec<AppPermissionId> = sqlx::query_scalar(
        "SELECT app_permission_id FROM org_role_permission WHERE org_role_id = $1 ORDER BY app_permission_id",
    )
    .bind(org_role_id)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn list_permission_keys_for_org_role(
    db: &PgPool,
    org_role_id: OrgRoleId,
) -> Result<Vec<String>> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"SELECT p.key
           FROM org_role_permission rp
           INNER JOIN app_permission p ON p.id = rp.app_permission_id
           WHERE rp.org_role_id = $1
           ORDER BY p.key ASC"#,
    )
    .bind(org_role_id)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn delete_org_role(
    db: &PgPool,
    organization_id: OrganizationId,
    org_role_id: OrgRoleId,
) -> Result<()> {
    let member_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM member WHERE org_role_id = $1",
    )
    .bind(org_role_id)
    .fetch_one(db)
    .await?;

    if member_count > 0 {
        anyhow::bail!("cannot delete a role that is assigned to members");
    }

    let invite_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::bigint FROM app_invite WHERE org_role_id = $1 AND accepted_at IS NULL",
    )
    .bind(org_role_id)
    .fetch_one(db)
    .await?;

    if invite_count > 0 {
        anyhow::bail!("cannot delete a role that is used by pending invites");
    }

    let result = sqlx::query("DELETE FROM org_role WHERE id = $1 AND organization_id = $2")
        .bind(org_role_id)
        .bind(organization_id)
        .execute(db)
        .await?;

    if result.rows_affected() == 0 {
        anyhow::bail!("organization role not found");
    }

    Ok(())
}

pub async fn find_primary_org_membership(
    db: &PgPool,
    application_id: ApplicationId,
    user_id: UserId,
) -> Result<Option<OrgMembershipContext>> {
    let row: Option<(OrganizationId, OrgRoleId, String)> = sqlx::query_as(
        r#"SELECT m.organization_id, m.org_role_id, r.slug
           FROM member m
           INNER JOIN organization o ON o.id = m.organization_id
           INNER JOIN org_role r ON r.id = m.org_role_id
           WHERE m.user_id = $1 AND o.application_id = $2
           ORDER BY m.created_at ASC
           LIMIT 1"#,
    )
    .bind(user_id)
    .bind(application_id)
    .fetch_optional(db)
    .await?;

    let Some((org_id, org_role_id, role_slug)) = row else {
        return Ok(None);
    };

    let permissions = list_permission_keys_for_org_role(db, org_role_id).await?;

    Ok(Some(OrgMembershipContext {
        org_id,
        org_role_id,
        role_slug,
        permissions,
    }))
}

pub async fn membership_for_org(
    db: &PgPool,
    application_id: ApplicationId,
    user_id: UserId,
    org_id: OrganizationId,
) -> Result<OrgMembershipContext> {
    let row: Option<(OrgRoleId, String)> = sqlx::query_as(
        r#"SELECT m.org_role_id, r.slug
           FROM member m
           INNER JOIN organization o ON o.id = m.organization_id
           INNER JOIN org_role r ON r.id = m.org_role_id
           WHERE m.user_id = $1
             AND o.id = $2
             AND o.application_id = $3"#,
    )
    .bind(user_id)
    .bind(org_id)
    .bind(application_id)
    .fetch_optional(db)
    .await?;

    let Some((org_role_id, role_slug)) = row else {
        anyhow::bail!("not a member of this organization");
    };

    let permissions = list_permission_keys_for_org_role(db, org_role_id).await?;

    Ok(OrgMembershipContext {
        org_id,
        org_role_id,
        role_slug,
        permissions,
    })
}
