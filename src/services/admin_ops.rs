use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::ids::{AdminUserId, ApplicationId, DirectoryId, OrganizationId};
use crate::models::admin_role::AdminRole;
use crate::models::admin_user::AdminUser;
use crate::services::{admin_access, admin_auth::AdminSession, identity, password};

pub struct DirectorySummary {
    pub id: DirectoryId,
    pub name: String,
    pub slug: String,
    pub application_count: i64,
    pub admin_count: i64,
    pub created_at: DateTime<Utc>,
}

const DIRECTORY_ADMIN_COUNT_SUBQUERY: &str = r#"
(SELECT COUNT(*)::bigint
 FROM admin_directory_grant g
 INNER JOIN admin_user u ON u.id = g.admin_user_id
 WHERE g.directory_id = d.id AND u.role = 'directory_admin')"#;

pub struct ApplicationSummary {
    pub id: ApplicationId,
    pub name: String,
    pub directory_id: DirectoryId,
    pub created_at: DateTime<Utc>,
    pub user_count: i64,
}

pub struct OperatorSummary {
    pub id: AdminUserId,
    pub email: String,
    pub role: AdminRole,
    pub granted_app_ids: Vec<ApplicationId>,
    pub granted_directory_ids: Vec<DirectoryId>,
    pub created_at: DateTime<Utc>,
}

pub struct TenantUserRow {
    pub id: String,
    pub name: String,
    pub email: String,
    pub email_verified: bool,
    pub created_at: DateTime<Utc>,
}

const USER_COUNT_SUBQUERY: &str =
    "(SELECT COUNT(DISTINCT g.user_id)::bigint FROM user_app_grant g WHERE g.application_id = a.id)";

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

pub async fn load_granted_directory_ids(
    db: &PgPool,
    admin_id: AdminUserId,
) -> Result<Vec<DirectoryId>> {
    let ids: Vec<DirectoryId> = sqlx::query_scalar(
        "SELECT directory_id FROM admin_directory_grant WHERE admin_user_id = $1 ORDER BY directory_id",
    )
    .bind(admin_id)
    .fetch_all(db)
    .await?;
    Ok(ids)
}

pub fn validate_directory_slug(slug: &str) -> Result<()> {
    let slug = slug.trim();
    if slug.is_empty() {
        anyhow::bail!("directory slug is required");
    }
    if slug == "default" {
        anyhow::bail!("slug 'default' is reserved for the built-in directory");
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        anyhow::bail!("slug must contain only lowercase letters, numbers, and hyphens");
    }
    if slug.starts_with('-') || slug.ends_with('-') {
        anyhow::bail!("slug cannot start or end with a hyphen");
    }
    Ok(())
}

pub async fn list_directories(db: &PgPool) -> Result<Vec<DirectorySummary>> {
    let rows: Vec<(DirectoryId, String, String, DateTime<Utc>, i64, i64)> =
        sqlx::query_as(&format!(
            r#"SELECT d.id, d.name, d.slug, d.created_at,
                      COUNT(a.id)::bigint AS application_count,
                      {DIRECTORY_ADMIN_COUNT_SUBQUERY} AS admin_count
               FROM directory d
               LEFT JOIN application a ON a.directory_id = d.id
               GROUP BY d.id
               ORDER BY d.created_at ASC"#
        ))
        .fetch_all(db)
        .await?;

    Ok(rows
        .into_iter()
        .map(|(id, name, slug, created_at, application_count, admin_count)| {
            DirectorySummary {
                id,
                name,
                slug,
                application_count,
                admin_count,
                created_at,
            }
        })
        .collect())
}

pub async fn get_directory(
    db: &PgPool,
    directory_id: DirectoryId,
) -> Result<Option<DirectorySummary>> {
    let row: Option<(DirectoryId, String, String, DateTime<Utc>, i64, i64)> =
        sqlx::query_as(&format!(
            r#"SELECT d.id, d.name, d.slug, d.created_at,
                      COUNT(a.id)::bigint AS application_count,
                      {DIRECTORY_ADMIN_COUNT_SUBQUERY} AS admin_count
               FROM directory d
               LEFT JOIN application a ON a.directory_id = d.id
               WHERE d.id = $1
               GROUP BY d.id"#
        ))
        .bind(directory_id)
        .fetch_optional(db)
        .await?;

    Ok(row.map(|(id, name, slug, created_at, application_count, admin_count)| {
        DirectorySummary {
            id,
            name,
            slug,
            application_count,
            admin_count,
            created_at,
        }
    }))
}

pub async fn list_directories_for_session(
    db: &PgPool,
    session: &AdminSession,
) -> Result<Vec<DirectorySummary>> {
    if session.is_instance_admin() {
        return list_directories(db).await;
    }
    if session.role == AdminRole::DirectoryAdmin {
        let mut out = Vec::new();
        for directory_id in &session.granted_directory_ids {
            if let Some(directory) = get_directory(db, *directory_id).await? {
                out.push(directory);
            }
        }
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        return Ok(out);
    }
    Ok(Vec::new())
}

pub struct DirectoryAdminRow {
    pub id: AdminUserId,
    pub email: String,
    pub created_at: DateTime<Utc>,
}

pub async fn list_directory_admins(
    db: &PgPool,
    directory_id: DirectoryId,
) -> Result<Vec<DirectoryAdminRow>> {
    let rows: Vec<(AdminUserId, String, DateTime<Utc>)> = sqlx::query_as(
        r#"SELECT u.id, u.email, u.created_at
           FROM admin_user u
           INNER JOIN admin_directory_grant g ON g.admin_user_id = u.id
           WHERE g.directory_id = $1 AND u.role = 'directory_admin'
           ORDER BY u.created_at ASC"#,
    )
    .bind(directory_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, email, created_at)| DirectoryAdminRow {
            id,
            email,
            created_at,
        })
        .collect())
}

pub async fn create_directory(db: &PgPool, name: &str, slug: &str) -> Result<DirectorySummary> {
    let name = name.trim();
    let slug = slug.trim();
    if name.is_empty() {
        anyhow::bail!("directory name is required");
    }
    validate_directory_slug(slug)?;

    let id = DirectoryId::new();
    let row: (DirectoryId, String, String, DateTime<Utc>) = sqlx::query_as(
        r#"INSERT INTO directory (id, name, slug)
           VALUES ($1, $2, $3)
           RETURNING id, name, slug, created_at"#,
    )
    .bind(id)
    .bind(name)
    .bind(slug)
    .fetch_one(db)
    .await?;

    Ok(DirectorySummary {
        id: row.0,
        name: row.1,
        slug: row.2,
        application_count: 0,
        admin_count: 0,
        created_at: row.3,
    })
}

pub async fn list_applications_for_admin(
    db: &PgPool,
    session: &AdminSession,
) -> Result<Vec<ApplicationSummary>> {
    let rows: Vec<(ApplicationId, String, DirectoryId, DateTime<Utc>, i64)> = match session.role {
        AdminRole::InstanceAdmin => {
            sqlx::query_as(&format!(
                r#"SELECT a.id, a.name, a.directory_id, a.created_at, {USER_COUNT_SUBQUERY} AS user_count
                   FROM application a
                   ORDER BY a.created_at DESC"#
            ))
            .fetch_all(db)
            .await?
        }
        AdminRole::AppAdmin => {
            sqlx::query_as(&format!(
                r#"SELECT a.id, a.name, a.directory_id, a.created_at, {USER_COUNT_SUBQUERY} AS user_count
                   FROM application a
                   INNER JOIN admin_app_grant g ON g.app_id = a.id
                   WHERE g.admin_user_id = $1
                   ORDER BY a.created_at DESC"#
            ))
            .bind(session.admin_id)
            .fetch_all(db)
            .await?
        }
        AdminRole::DirectoryAdmin => {
            sqlx::query_as(&format!(
                r#"SELECT a.id, a.name, a.directory_id, a.created_at, {USER_COUNT_SUBQUERY} AS user_count
                   FROM application a
                   INNER JOIN admin_directory_grant g ON g.directory_id = a.directory_id
                   WHERE g.admin_user_id = $1
                   ORDER BY a.created_at DESC"#
            ))
            .bind(session.admin_id)
            .fetch_all(db)
            .await?
        }
    };

    Ok(rows
        .into_iter()
        .map(|(id, name, directory_id, created_at, user_count)| ApplicationSummary {
            id,
            name,
            directory_id,
            created_at,
            user_count,
        })
        .collect())
}

pub async fn get_application_summary(
    db: &PgPool,
    app_id: ApplicationId,
) -> Result<Option<ApplicationSummary>> {
    let row: Option<(ApplicationId, String, DirectoryId, DateTime<Utc>, i64)> = sqlx::query_as(
        &format!(
            r#"SELECT a.id, a.name, a.directory_id, a.created_at, {USER_COUNT_SUBQUERY} AS user_count
               FROM application a
               WHERE a.id = $1"#
        ),
    )
    .bind(app_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|(id, name, directory_id, created_at, user_count)| ApplicationSummary {
        id,
        name,
        directory_id,
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
        r#"SELECT u.id, u.name, u.email, u.email_verified, u.created_at
           FROM "user" u
           INNER JOIN user_app_grant g ON g.user_id = u.id
           WHERE g.application_id = $1
             AND ($2 = '' OR u.name ILIKE '%' || $2 || '%' OR u.email ILIKE '%' || $2 || '%')
           ORDER BY u.created_at DESC
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
    pub member_count: i64,
    pub created_at: DateTime<Utc>,
}

pub struct OrgDetail {
    pub id: String,
    pub name: String,
    pub slug: String,
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

const ORG_VISIBILITY: &str = "o.application_id = a.id";

pub async fn list_orgs_for_app(db: &PgPool, app_id: ApplicationId) -> Result<Vec<OrgSummary>> {
    let rows: Vec<(String, String, String, i64, DateTime<Utc>)> = sqlx::query_as(&format!(
        r#"SELECT o.id, o.name, o.slug, COUNT(m.id)::bigint, o.created_at
           FROM organization o
           INNER JOIN application a ON a.id = $1
           LEFT JOIN member m ON m.organization_id = o.id
           WHERE {ORG_VISIBILITY}
           GROUP BY o.id
           ORDER BY o.created_at DESC"#
    ))
    .bind(app_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|(id, name, slug, member_count, created_at)| OrgSummary {
            id,
            name,
            slug,
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
    let ctx = identity::load_app_context(db, app_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?
        .ok_or_else(|| anyhow::anyhow!("application not found"))?;

    let name = name.trim();
    let slug = slug.trim();
    if name.is_empty() {
        anyhow::bail!("organization name is required");
    }
    if slug.is_empty() {
        anyhow::bail!("organization slug is required");
    }

    let row: (String, String, String, DateTime<Utc>) = sqlx::query_as(
        r#"INSERT INTO organization (id, directory_id, application_id, name, slug)
           VALUES ($1, $2, $3, $4, $5)
           RETURNING id, name, slug, created_at"#,
    )
    .bind(OrganizationId::new())
    .bind(ctx.directory_id)
    .bind(app_id)
    .bind(name)
    .bind(slug)
    .fetch_one(db)
    .await?;

    Ok(OrgDetail {
        id: row.0,
        name: row.1,
        slug: row.2,
        created_at: row.3,
    })
}

pub async fn get_org_for_app(
    db: &PgPool,
    app_id: ApplicationId,
    org_id: &str,
) -> Result<Option<OrgDetail>> {
    let ctx = identity::load_app_context(db, app_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?
        .ok_or_else(|| anyhow::anyhow!("application not found"))?;

    if !identity::organization_visible_to_app(db, &ctx, org_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?
    {
        return Ok(None);
    }

    let row: Option<(String, String, String, DateTime<Utc>)> = sqlx::query_as(
        r#"SELECT id, name, slug, created_at
           FROM organization
           WHERE id = $1"#,
    )
    .bind(org_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|(id, name, slug, created_at)| OrgDetail {
        id,
        name,
        slug,
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
    let Some(_org) = org else {
        anyhow::bail!("organization not found");
    };

    let user_id_parsed: crate::ids::UserId = user_id
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid user id"))?;

    if !identity::user_visible_to_application(db, user_id_parsed, app_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?
    {
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

pub async fn remove_org_member(
    db: &PgPool,
    app_id: ApplicationId,
    org_id: &str,
    user_id: &str,
) -> Result<bool> {
    if get_org_for_app(db, app_id, org_id).await?.is_none() {
        anyhow::bail!("organization not found");
    }

    let result = sqlx::query("DELETE FROM member WHERE organization_id = $1 AND user_id = $2")
        .bind(org_id)
        .bind(user_id)
        .execute(db)
        .await?;

    Ok(result.rows_affected() > 0)
}

pub async fn create_application(
    db: &PgPool,
    name: &str,
    client_secret_hash: &str,
    directory_id: Option<DirectoryId>,
) -> Result<ApplicationId> {
    let directory_id = match directory_id {
        Some(id) => id,
        None => identity::get_default_directory_id(db)
            .await
            .map_err(|e| anyhow::anyhow!(e))?,
    };

    let id = ApplicationId::new();
    sqlx::query(
        "INSERT INTO application (id, directory_id, client_secret_hash, name) VALUES ($1, $2, $3, $4)",
    )
    .bind(id)
    .bind(directory_id)
    .bind(client_secret_hash)
    .bind(name)
    .execute(db)
    .await?;

    Ok(id)
}

pub async fn list_applications_for_picker(
    db: &PgPool,
    session: &AdminSession,
) -> Result<Vec<(ApplicationId, String)>> {
    match session.role {
        AdminRole::InstanceAdmin => {
            let rows: Vec<(ApplicationId, String)> = sqlx::query_as(
                "SELECT id, name FROM application ORDER BY name ASC",
            )
            .fetch_all(db)
            .await?;
            Ok(rows)
        }
        AdminRole::DirectoryAdmin => {
            let rows: Vec<(ApplicationId, String)> = sqlx::query_as(
                r#"SELECT a.id, a.name
                   FROM application a
                   INNER JOIN admin_directory_grant g ON g.directory_id = a.directory_id
                   WHERE g.admin_user_id = $1
                   ORDER BY a.name ASC"#,
            )
            .bind(session.admin_id)
            .fetch_all(db)
            .await?;
            Ok(rows)
        }
        AdminRole::AppAdmin => Ok(Vec::new()),
    }
}

pub async fn list_directories_for_picker(
    db: &PgPool,
    session: &AdminSession,
) -> Result<Vec<(DirectoryId, String)>> {
    match session.role {
        AdminRole::InstanceAdmin => {
            let rows: Vec<(DirectoryId, String)> = sqlx::query_as(
                "SELECT id, name FROM directory ORDER BY name ASC",
            )
            .fetch_all(db)
            .await?;
            Ok(rows)
        }
        AdminRole::DirectoryAdmin => {
            let rows: Vec<(DirectoryId, String)> = sqlx::query_as(
                r#"SELECT d.id, d.name
                   FROM directory d
                   INNER JOIN admin_directory_grant g ON g.directory_id = d.id
                   WHERE g.admin_user_id = $1
                   ORDER BY d.name ASC"#,
            )
            .bind(session.admin_id)
            .fetch_all(db)
            .await?;
            Ok(rows)
        }
        AdminRole::AppAdmin => Ok(Vec::new()),
    }
}

async fn operator_in_directory_scope(
    db: &PgPool,
    operator: &OperatorSummary,
    directory_ids: &[DirectoryId],
) -> Result<bool> {
    match operator.role {
        AdminRole::InstanceAdmin => Ok(false),
        AdminRole::DirectoryAdmin => {
            if operator.granted_directory_ids.is_empty() {
                return Ok(false);
            }
            Ok(operator
                .granted_directory_ids
                .iter()
                .all(|id| directory_ids.contains(id)))
        }
        AdminRole::AppAdmin => {
            if operator.granted_app_ids.is_empty() {
                return Ok(false);
            }
            for app_id in &operator.granted_app_ids {
                let directory_id: DirectoryId =
                    sqlx::query_scalar("SELECT directory_id FROM application WHERE id = $1")
                        .bind(app_id)
                        .fetch_one(db)
                        .await?;
                if !directory_ids.contains(&directory_id) {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

pub async fn list_operators_for_session(
    db: &PgPool,
    session: &AdminSession,
) -> Result<Vec<OperatorSummary>> {
    let all = list_operators(db).await?;
    if session.is_instance_admin() {
        return Ok(all);
    }
    if session.role == AdminRole::DirectoryAdmin {
        let mut filtered = Vec::new();
        for operator in all {
            if operator_in_directory_scope(db, &operator, &session.granted_directory_ids).await? {
                filtered.push(operator);
            }
        }
        return Ok(filtered);
    }
    Ok(Vec::new())
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
        let granted_directory_ids = if role == AdminRole::DirectoryAdmin {
            load_granted_directory_ids(db, admin.id).await?
        } else {
            Vec::new()
        };
        out.push(OperatorSummary {
            id: admin.id,
            email: admin.email,
            role,
            granted_app_ids,
            granted_directory_ids,
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
    creator: &AdminSession,
    email: &str,
    raw_password: &str,
    role: AdminRole,
    app_ids: &[ApplicationId],
    directory_ids: &[DirectoryId],
) -> Result<AdminUser> {
    if !admin_access::can_manage_operators(creator) {
        anyhow::bail!("you do not have permission to create operators");
    }
    if !admin_access::can_assign_role(creator, role) {
        anyhow::bail!("you do not have permission to assign this role");
    }

    let email = email.trim();
    if email.is_empty() || !email.contains('@') {
        anyhow::bail!("valid email is required");
    }
    if raw_password.len() < 8 {
        anyhow::bail!("password must be at least 8 characters");
    }

    match role {
        AdminRole::AppAdmin if app_ids.is_empty() => {
            anyhow::bail!("app admin must be assigned at least one application");
        }
        AdminRole::DirectoryAdmin if directory_ids.is_empty() => {
            anyhow::bail!("directory admin must be assigned at least one directory");
        }
        AdminRole::InstanceAdmin if !creator.is_instance_admin() => {
            anyhow::bail!("only instance admins can create instance admins");
        }
        _ => {}
    }

    admin_access::ensure_apps_in_scope(db, creator, app_ids).await?;
    admin_access::ensure_directories_in_scope(db, creator, directory_ids).await?;

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
        sqlx::query("INSERT INTO admin_app_grant (admin_user_id, app_id) VALUES ($1, $2)")
            .bind(user.id)
            .bind(app_id)
            .execute(&mut *tx)
            .await?;
    }

    for directory_id in directory_ids {
        sqlx::query(
            "INSERT INTO admin_directory_grant (admin_user_id, directory_id) VALUES ($1, $2)",
        )
        .bind(user.id)
        .bind(directory_id)
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
