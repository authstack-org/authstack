use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AppPermissionId, ApplicationId};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AppPermission {
    pub id: AppPermissionId,
    pub application_id: ApplicationId,
    pub key: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
