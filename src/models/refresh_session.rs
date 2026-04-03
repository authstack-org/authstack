use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{RefreshSessionId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct RefreshSession {
    pub id: RefreshSessionId,
    pub user_id: UserId,
    pub jti: String,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}
