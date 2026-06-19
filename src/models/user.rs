use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{DirectoryId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: UserId,
    pub directory_id: DirectoryId,
    pub name: String,
    pub email: String,
    pub email_verified: bool,
    pub image: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
