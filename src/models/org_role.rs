use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{OrgRoleId, OrganizationId};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrgRole {
    pub id: OrgRoleId,
    pub organization_id: OrganizationId,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
