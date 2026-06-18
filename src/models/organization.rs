use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{ApplicationId, DirectoryId, OrganizationId};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "org_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum OrgType {
    Personal,
    Team,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Organization {
    pub id: OrganizationId,
    pub directory_id: DirectoryId,
    pub application_id: Option<ApplicationId>,
    pub name: String,
    pub slug: String,
    pub org_type: OrgType,
    pub logo: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
