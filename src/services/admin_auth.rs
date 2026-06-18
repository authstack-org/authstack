use anyhow::Result;
use sqlx::PgPool;

use crate::ids::{AdminUserId, ApplicationId};
use crate::models::admin_role::AdminRole;
use crate::models::admin_user::AdminUser;
use crate::services::{admin_ops, password};

pub async fn create_admin(db: &PgPool, email: &str, raw_password: &str) -> Result<AdminUser> {
    let password_hash = password::hash(raw_password)?;
    let id = AdminUserId::new();
    let row: AdminUser = sqlx::query_as(
        "INSERT INTO admin_user (id, email, password_hash, role) VALUES ($1, $2, $3, 'instance_admin')
         RETURNING id, email, password_hash, role, created_at, updated_at",
    )
    .bind(id)
    .bind(email)
    .bind(password_hash)
    .fetch_one(db)
    .await?;
    Ok(row)
}

pub async fn login_admin(
    db: &PgPool,
    email: &str,
    raw_password: &str,
) -> Result<Option<AdminUser>> {
    let row: Option<AdminUser> = sqlx::query_as(
        "SELECT id, email, password_hash, role, created_at, updated_at FROM admin_user WHERE email = $1",
    )
    .bind(email)
    .fetch_optional(db)
    .await?;

    match row {
        Some(user) if password::verify(raw_password, &user.password_hash)? => Ok(Some(user)),
        _ => Ok(None),
    }
}

#[derive(Clone, Debug)]
pub struct AdminSession {
    pub admin_id: AdminUserId,
    pub email: String,
    pub role: AdminRole,
    pub granted_app_ids: Vec<ApplicationId>,
}

impl AdminSession {
    pub fn is_instance_admin(&self) -> bool {
        self.role == AdminRole::InstanceAdmin
    }

    pub fn can_access_app(&self, app_id: ApplicationId) -> bool {
        self.is_instance_admin() || self.granted_app_ids.contains(&app_id)
    }
}

pub async fn load_session(db: &PgPool, admin_id: AdminUserId, email: String) -> Result<AdminSession> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT role FROM admin_user WHERE id = $1")
            .bind(admin_id)
            .fetch_optional(db)
            .await?;

    let role_str = row
        .map(|(r,)| r)
        .ok_or_else(|| anyhow::anyhow!("admin user not found"))?;
    let role = role_str
        .parse::<AdminRole>()
        .map_err(|_| anyhow::anyhow!("invalid admin role: {role_str}"))?;

    let granted_app_ids = if role == AdminRole::AppAdmin {
        admin_ops::load_granted_app_ids(db, admin_id).await?
    } else {
        Vec::new()
    };

    Ok(AdminSession {
        admin_id,
        email,
        role,
        granted_app_ids,
    })
}
