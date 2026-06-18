use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::DirectoryId;
use crate::models::identity_policy::IdentityPolicy;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Directory {
    pub id: DirectoryId,
    pub name: String,
    pub slug: String,
    pub identity_policy: IdentityPolicy,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub const DEFAULT_DIRECTORY_ID: &str = "dir_00000000000000000000000001";
pub const DEFAULT_DIRECTORY_SLUG: &str = "default";
