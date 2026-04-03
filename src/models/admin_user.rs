use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::AdminUserId;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AdminUser {
    pub id: AdminUserId,
    pub email: String,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
