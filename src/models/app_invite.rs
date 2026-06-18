use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::ids::{ApplicationId, InviteId, OrganizationId, UserId};

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct AppInvite {
    pub id: InviteId,
    pub token: String,
    pub application_id: ApplicationId,
    pub organization_id: OrganizationId,
    pub email: String,
    pub role: String,
    pub name: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub accepted_user_id: Option<UserId>,
    pub created_at: DateTime<Utc>,
}
