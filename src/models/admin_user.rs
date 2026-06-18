use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::AdminUserId;
use crate::models::admin_role::AdminRole;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AdminUser {
    pub id: AdminUserId,
    pub email: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AdminUser {
    pub fn admin_role(&self) -> Option<AdminRole> {
        self.role.parse().ok()
    }
}
