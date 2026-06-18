use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::ids::{AdminUserId, ApplicationId, OrganizationId};
use crate::models::admin_role::AdminRole;
use crate::models::admin_user::AdminUser;
use crate::services::password;

pub struct ApplicationSummary {
    pub id: ApplicationId,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub user_count: i64,
}

pub struct OperatorSummary {
    pub id: AdminUserId,
    pub email: String,
    pub role: AdminRole,
    pub granted_app_ids: Vec<ApplicationId>,
    pub created_at: DateTime<Utc>,
}

pub struct TenantUserRow {
    pub id: String,
    pub name: String,
    pub email: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
}

pub async fn load_granted_app_ids(
    db: &PgPool,
    admin_id: AdminUserId,
) -> Result<Vec<ApplicationId>> {
    let ids: Vec<ApplicationId> = sqlx::query_scalar(
        "SELECT app_id FROM admin_app_grant WHERE admin_user_id = $1 ORDER BY app_id",
    )
    .bind(admin_id)
    .fetch_all(db)
    .await?;
    Ok(ids)
}

pub async fn list_applications_for_admin(
    db: &PgPool,
    role: AdminRole,
    admin_id: AdminUserId,
) -> Result<Vec<ApplicationSummary>> {
    let rows: Vec<(ApplicationId, String, DateTime<Utc>, i64)> = match role {
        AdminRole::InstanceAdmin => {
            sqlx::query_as(
                r#"SELECT a.id, a.name, a.created_at,
                          (SELECT COUNT(*)::bigint FROM "user" u WHERE u.app_id = a.id) AS user_count
                   FROM application a
                   ORDER BY a.created_at DESC"#,
            )
            .fetch_all(db)
            .await?
        }
        AdminRole::AppAdmin => {
            sqlx::query_as(
                r#"SELECT a.id, a.name, a.created_at,
                          (SELECT COUNT(*)::bigint FROM "user" u WHERE u.app_id = a.id) AS user_count
                   FROM application a
                   INNER JOIN admin_app_grant g ON g.app_id = a.id
                   WHERE g.admin_user_id = $1
                   ORDER BY a.created_at DESC"#,
            )
            .bind(admin_id)
            .fetch_all(db)
            .await?
        }
    };

    Ok(rows
        .into_iter()
        .map(|(id, name, created_at, user_count)| ApplicationSummary {
            id,
            name,
            created_at,
            user_count,
        })
        .collect())
}

pub async fn get_application_summary(
    db: &PgPool,
    app_id: ApplicationId,
) -> Result<Option<ApplicationSummary>> {
    let row: Option<(ApplicationId, String, DateTime<Utc>, i64)> = sqlx::query_as(
        r#"SELECT a.id, a.name, a.created_at,
                  (SELECT COUNT(*)::bigint FROM "user" u WHERE u.app_id = a.id) AS user_count
           FROM application a
           WHERE a.id = $1"#,
    )
    .bind(app_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|(id, name, created_at, user_count)| ApplicationSummary {
        id,
        name,
        created_at,
        user_count,
    }))
}

pub async fn list_tenant_users(db: &PgPool, app_id: ApplicationId) -> Result<Vec<TenantUserRow>> {
    search_tenant_users(db, app_id, "").await
}

pub async fn search_tenant_users(
    db: &PgPool,
    app_id: ApplicationId,
    query: &str,
) -> Result<Vec<TenantUserRow>> {
    let q = query.trim();
    let rows: Vec<(String, String, String, bool, DateTime<Utc>)> = sqlx::query_as(
        r#"SELECT id, name, email, email_verified, created_at
           FROM "user"
           WHERE app_id = $1
             AND ($2 = '' OR name ILIKE '%' || $2 || '%' OR email ILIKE '%' || $2 || '%')
           ORDER BY created_at DESC
           LIMIT 50"#,
    )
    .bind(app_id)
    .bind(q)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, name, email, email_verified, created_at)| TenantUserRow {
            id,
            name,
            email,
            email_verified,
            created_at,
        })
        .collect())
}

pub struct OrgSummary {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub org_type: String,
    pub member_count: i64,
    pub created_at: DateTime<Utc>,
}

pub struct OrgDetail {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub org_type: String,
    pub created_at: DateTime<Utc>,
}

pub struct OrgMemberRow {
    pub id: String,
    pub user_id: String,
    pub user_name: String,
    pub user_email: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
}

pub async fn list_orgs_for_app(db: &PgPool, app_id: ApplicationId) -> Result<Vec<OrgSummary>> {
    let rows: Vec<(String, String, String, String, i64, DateTime<Utc>)> = sqlx::query_as(
        r#"SELECT o.id, o.name, o.slug, o.org_type::text, COUNT(m.id)::bigint, o.created_at
           FROM organization o
           LEFT JOIN member m ON m.organization_id = o.id
           WHERE o.app_id = $1
           GROUP BY o.id
           ORDER BY o.created_at DESC"#,
    )
    .bind(app_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, name, slug, org_type, member_count, created_at)| OrgSummary {
            id,
            name,
            slug,
            org_type,
            member_count,
            created_at,
        })
        .collect())
}

pub async fn create_team_org(
    db: &PgPool,
    app_id: ApplicationId,
    name: &str,
    slug: &str,
) -> Result<OrgDetail> {
    let name = name.trim();
    let slug = slug.trim();
    if name.is_empty() {
        anyhow::bail!("organization name is required");
    }
    if slug.is_empty() {
        anyhow::bail!("organization slug is required");
    }

    let row: (String, String, String, String, DateTime<Utc>) = sqlx::query_as(
        r#"INSERT INTO organization (id, app_id, name, slug, org_type)
           VALUES ($1, $2, $3, $4, 'team')
           RETURNING id, name, slug, org_type::text, created_at"#,
    )
    .bind(OrganizationId::new())
    .bind(app_id)
    .bind(name)
    .bind(slug)
    .fetch_one(db)
    .await?;

    Ok(OrgDetail {
        id: row.0,
        name: row.1,
        slug: row.2,
        org_type: row.3,
        created_at: row.4,
    })
}

pub async fn get_org_for_app(
    db: &PgPool,
    app_id: ApplicationId,
    org_id: &str,
) -> Result<Option<OrgDetail>> {
    let row: Option<(String, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        r#"SELECT id, name, slug, org_type::text, created_at
           FROM organization
           WHERE id = $1 AND app_id = $2"#,
    )
    .bind(org_id)
    .bind(app_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|(id, name, slug, org_type, created_at)| OrgDetail {
        id,
        name,
        slug,
        org_type,
        created_at,
    }))
}

pub async fn list_org_members(db: &PgPool, org_id: &str) -> Result<Vec<OrgMemberRow>> {
    let rows: Vec<(String, String, String, String, String, DateTime<Utc>)> = sqlx::query_as(
        r#"SELECT m.id, m.user_id, u.name, u.email, m.role, m.created_at
           FROM member m
           JOIN "user" u ON u.id = m.user_id
           WHERE m.organization_id = $1
           ORDER BY m.created_at ASC"#,
    )
    .bind(org_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, user_id, user_name, user_email, role, created_at)| OrgMemberRow {
            id,
            user_id,
            user_name,
            user_email,
            role,
            created_at,
        })
        .collect())
}

pub async fn add_org_member(
    db: &PgPool,
    app_id: ApplicationId,
    org_id: &str,
    user_id: &str,
    role: &str,
) -> Result<()> {
    let org = get_org_for_app(db, app_id, org_id).await?;
    let Some(org) = org else {
        anyhow::bail!("organization not found");
    };

    if org.org_type != "team" {
        anyhow::bail!("members can only be added to team organizations");
    }

    let user_exists: Option<String> = sqlx::query_scalar(
        r#"SELECT id FROM "user" WHERE id = $1 AND app_id = $2"#,
    )
    .bind(user_id)
    .bind(app_id)
    .fetch_optional(db)
    .await?;

    if user_exists.is_none() {
        anyhow::bail!("user not found in this application");
    }

    let role = if role.trim().is_empty() { "member" } else { role.trim() };
    let member_id = crate::ids::MemberId::new();

    sqlx::query(
        "INSERT INTO member (id, organization_id, user_id, role) VALUES ($1, $2, $3, $4)",
    )
    .bind(member_id)
    .bind(org_id)
    .bind(user_id)
    .bind(role)
    .execute(db)
    .await?;

    Ok(())
}

pub async fn remove_org_member(db: &PgPool, app_id: ApplicationId, org_id: &str, user_id: &str) -> Result<bool> {
    let org = get_org_for_app(db, app_id, org_id).await?;
    if org.is_none() {
        anyhow::bail!("organization not found");
    }

    let result = sqlx::query("DELETE FROM member WHERE organization_id = $1 AND user_id = $2")
        .bind(org_id)
        .bind(user_id)
        .execute(db)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn list_operators(db: &PgPool) -> Result<Vec<OperatorSummary>> {
    let admins: Vec<AdminUser> = sqlx::query_as(
        r#"SELECT id, email, password_hash, role, created_at, updated_at
           FROM admin_user
           ORDER BY created_at ASC"#,
    )
    .fetch_all(db)
    .await?;

    let mut out = Vec::with_capacity(admins.len());
    for admin in admins {
        let role = admin
            .admin_role()
            .ok_or_else(|| anyhow::anyhow!("invalid admin role: {}", admin.role))?;
        let granted_app_ids = if role == AdminRole::AppAdmin {
            load_granted_app_ids(db, admin.id).await?
        } else {
            Vec::new()
        };
        out.push(OperatorSummary {
            id: admin.id,
            email: admin.email,
            role,
            granted_app_ids,
            created_at: admin.created_at,
        });
    }
    Ok(out)
}

pub async fn list_all_applications_for_picker(db: &PgPool) -> Result<Vec<(ApplicationId, String)>> {
    let rows: Vec<(ApplicationId, String)> = sqlx::query_as(
        "SELECT id, name FROM application ORDER BY name ASC",
    )
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub async fn create_operator(
    db: &PgPool,
    email: &str,
    raw_password: &str,
    role: AdminRole,
    app_ids: &[ApplicationId],
) -> Result<AdminUser> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        anyhow::bail!("valid email is required");
    }
    if raw_password.len() < 8 {
        anyhow::bail!("password must be at least 8 characters");
    }
    if role == AdminRole::AppAdmin && app_ids.is_empty() {
        anyhow::bail!("app admin must be assigned at least one application");
    }

    let password_hash = password::hash(raw_password)?;
    let id = AdminUserId::new();

    let mut tx = db.begin().await?;

    let user: AdminUser = sqlx::query_as(
        "INSERT INTO admin_user (id, email, password_hash, role) VALUES ($1, $2, $3, $4)
         RETURNING id, email, password_hash, role, created_at, updated_at",
    )
    .bind(id)
    .bind(email)
    .bind(password_hash)
    .bind(role.as_str())
    .fetch_one(&mut *tx)
    .await?;

    for app_id in app_ids {
        sqlx::query(
            "INSERT INTO admin_app_grant (admin_user_id, app_id) VALUES ($1, $2)",
        )
        .bind(user.id)
        .bind(app_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(user)
}

pub async fn delete_application(db: &PgPool, app_id: ApplicationId) -> Result<bool> {
    let result = sqlx::query("DELETE FROM application WHERE id = $1")
        .bind(app_id)
        .execute(db)
        .await?;
    Ok(result.rows_affected() > 0)
}
