use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{ApplicationId, DirectoryId};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Application {
    pub id: ApplicationId,
    pub directory_id: DirectoryId,
    pub client_secret_hash: String,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
