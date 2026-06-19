use anyhow::Result;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, Result as ApiResult},
    ids::{AccountId, ApplicationId, InviteId, MemberId, OrganizationId, UserId},
    models::{app_invite::AppInvite, user::User},
    services::{identity, password},
};

pub struct CreateInviteInput<'a> {
    pub application_id: ApplicationId,
    pub organization_id: OrganizationId,
    pub email: &'a str,
    pub role: &'a str,
    pub name: Option<&'a str>,
    pub expiry_secs: u64,
}

pub struct InviteWithContext {
    pub invite: AppInvite,
    pub organization_name: String,
    pub app_name: String,
}

pub fn invite_url(public_base_url: &str, token: &str) -> String {
    format!("{public_base_url}/invite/{token}")
}

fn generate_token() -> String {
    format!(
        "{}{}",
        Uuid::new_v4().to_string().replace('-', ""),
        Uuid::new_v4().to_string().replace('-', "")
    )
}

pub async fn create_invite(db: &PgPool, input: CreateInviteInput<'_>) -> Result<AppInvite> {
    let email = input.email.trim().to_lowercase();
    if email.is_empty() || !email.contains('@') {
        anyhow::bail!("valid email is required");
    }

    let role = if input.role.trim().is_empty() {
        "member"
    } else {
        input.role.trim()
    };

    let ctx = identity::load_app_context(db, input.application_id)
        .await
        .map_err(|e| anyhow::anyhow!(e))?
        .ok_or_else(|| anyhow::anyhow!("application not found"))?;

    if !identity::organization_visible_to_app(db, &ctx, &input.organization_id.to_string())
        .await
        .map_err(|e| anyhow::anyhow!(e))?
    {
        anyhow::bail!("organization not found");
    }

    let existing_member: Option<UserId> = sqlx::query_scalar(
        r#"SELECT m.user_id
           FROM member m
           JOIN "user" u ON u.id = m.user_id
           WHERE m.organization_id = $1 AND lower(u.email) = $2"#,
    )
    .bind(input.organization_id)
    .bind(&email)
    .fetch_optional(db)
    .await?;

    if existing_member.is_some() {
        anyhow::bail!("user is already a member of this organization");
    }

    let mut tx = db.begin().await?;

    sqlx::query(
        "DELETE FROM app_invite WHERE organization_id = $1 AND lower(email) = $2 AND accepted_at IS NULL",
    )
    .bind(input.organization_id)
    .bind(&email)
    .execute(&mut *tx)
    .await?;

    let id = InviteId::new();
    let token = generate_token();
    let expires_at = Utc::now() + Duration::seconds(input.expiry_secs as i64);
    let name = input
        .name
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .map(str::to_string);

    let invite: AppInvite = sqlx::query_as(
        r#"INSERT INTO app_invite (id, token, application_id, organization_id, email, role, name, expires_at)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
           RETURNING id, token, application_id, organization_id, email, role, name, expires_at,
                     accepted_at, accepted_user_id, created_at"#,
    )
    .bind(id)
    .bind(&token)
    .bind(input.application_id)
    .bind(input.organization_id)
    .bind(&email)
    .bind(role)
    .bind(name)
    .bind(expires_at)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(invite)
}

pub async fn list_pending_invites(
    db: &PgPool,
    application_id: ApplicationId,
    organization_id: OrganizationId,
) -> Result<Vec<AppInvite>> {
    let rows: Vec<AppInvite> = sqlx::query_as(
        r#"SELECT id, token, application_id, organization_id, email, role, name, expires_at,
                  accepted_at, accepted_user_id, created_at
           FROM app_invite
           WHERE application_id = $1 AND organization_id = $2 AND accepted_at IS NULL
             AND expires_at > NOW()
           ORDER BY created_at DESC"#,
    )
    .bind(application_id)
    .bind(organization_id)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

pub struct AppInviteSummary {
    pub email: String,
    pub role: String,
    pub token: String,
    pub expires_at: chrono::DateTime<Utc>,
    pub organization_id: OrganizationId,
    pub organization_name: String,
}

pub async fn list_pending_invites_for_app(
    db: &PgPool,
    application_id: ApplicationId,
) -> Result<Vec<AppInviteSummary>> {
    let rows: Vec<(String, String, String, chrono::DateTime<Utc>, OrganizationId, String)> =
        sqlx::query_as(
            r#"SELECT i.email, i.role, i.token, i.expires_at, i.organization_id, o.name
               FROM app_invite i
               JOIN organization o ON o.id = i.organization_id
               WHERE i.application_id = $1 AND i.accepted_at IS NULL AND i.expires_at > NOW()
               ORDER BY i.created_at DESC"#,
        )
        .bind(application_id)
        .fetch_all(db)
        .await?;

    Ok(rows
        .into_iter()
        .map(
            |(email, role, token, expires_at, organization_id, organization_name)| {
                AppInviteSummary {
                    email,
                    role,
                    token,
                    expires_at,
                    organization_id,
                    organization_name,
                }
            },
        )
        .collect())
}

pub async fn get_invite_by_token(db: &PgPool, token: &str) -> Result<Option<InviteWithContext>> {
    let row: Option<AppInvite> = sqlx::query_as(
        r#"SELECT id, token, application_id, organization_id, email, role, name, expires_at,
                  accepted_at, accepted_user_id, created_at
           FROM app_invite WHERE token = $1"#,
    )
    .bind(token)
    .fetch_optional(db)
    .await?;

    let Some(invite) = row else {
        return Ok(None);
    };

    let meta: Option<(String, String)> = sqlx::query_as(
        r#"SELECT o.name, a.name
           FROM organization o
           JOIN application a ON a.id = $1
           WHERE o.id = $2"#,
    )
    .bind(invite.application_id)
    .bind(invite.organization_id)
    .fetch_optional(db)
    .await?;

    let Some((organization_name, app_name)) = meta else {
        return Ok(None);
    };

    Ok(Some(InviteWithContext {
        invite,
        organization_name,
        app_name,
    }))
}

pub fn validate_invite_active(invite: &AppInvite) -> ApiResult<()> {
    if invite.accepted_at.is_some() {
        return Err(AppError::Conflict("invite has already been accepted".to_string()));
    }
    if invite.expires_at < Utc::now() {
        return Err(AppError::Validation("invite has expired".to_string()));
    }
    Ok(())
}

pub async fn accept_invite(
    db: &PgPool,
    token: &str,
    name: &str,
    password: &str,
) -> ApiResult<User> {
    if password.len() < 8 {
        return Err(AppError::Validation(
            "password must be at least 8 characters".to_string(),
        ));
    }

    let ctx_wrap = get_invite_by_token(db, token)
        .await
        .map_err(AppError::Internal)?
        .ok_or_else(|| AppError::NotFound("invite not found".to_string()))?;

    validate_invite_active(&ctx_wrap.invite)?;

    let application_id = ctx_wrap.invite.application_id;
    let org_id = ctx_wrap.invite.organization_id;
    let email = ctx_wrap.invite.email.clone();
    let role = ctx_wrap.invite.role.clone();

    let app_ctx = identity::load_app_context(db, application_id)
        .await?
        .ok_or_else(|| AppError::NotFound("invite not found".to_string()))?;

    let existing = identity::find_user_for_login(db, &app_ctx, &email).await?;

    let mut tx = db.begin().await?;

    let user = if let Some(user) = existing {
        let hash: Option<String> = sqlx::query_scalar(
            "SELECT password_hash FROM account WHERE user_id = $1 AND provider_id = 'credential'",
        )
        .bind(user.id)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();

        let hash = hash.ok_or_else(|| {
            AppError::Validation(
                "account exists but has no password — contact your administrator".to_string(),
            )
        })?;

        let valid = password::verify(password, &hash).map_err(AppError::Internal)?;
        if !valid {
            return Err(AppError::Unauthorized("invalid password".to_string()));
        }

        if !identity::user_has_app_access(db, user.id, application_id).await? {
            identity::grant_app_access(&mut tx, user.id, application_id).await?;
        }

        user
    } else {
        let display_name = {
            let from_form = name.trim();
            if !from_form.is_empty() {
                from_form.to_string()
            } else if let Some(ref preset) = ctx_wrap.invite.name {
                preset.clone()
            } else {
                return Err(AppError::Validation("name is required".to_string()));
            }
        };

        let password_hash = password::hash(password).map_err(AppError::Internal)?;

        let user: User = sqlx::query_as(
            r#"INSERT INTO "user" (id, directory_id, name, email, email_verified)
               VALUES ($1, $2, $3, $4, false)
               RETURNING id, directory_id, name, email, email_verified, image, created_at, updated_at"#,
        )
        .bind(UserId::new())
        .bind(app_ctx.directory_id)
        .bind(&display_name)
        .bind(&email)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query(
            "INSERT INTO account (id, provider_id, user_id, password_hash) VALUES ($1, 'credential', $2, $3)",
        )
        .bind(AccountId::new())
        .bind(user.id)
        .bind(password_hash)
        .execute(&mut *tx)
        .await?;

        identity::grant_app_access(&mut tx, user.id, application_id).await?;

        user
    };

    let already_member: Option<MemberId> = sqlx::query_scalar(
        "SELECT id FROM member WHERE organization_id = $1 AND user_id = $2",
    )
    .bind(org_id)
    .bind(user.id)
    .fetch_optional(&mut *tx)
    .await?;

    if already_member.is_none() {
        sqlx::query(
            "INSERT INTO member (id, organization_id, user_id, role) VALUES ($1, $2, $3, $4)",
        )
        .bind(MemberId::new())
        .bind(org_id)
        .bind(user.id)
        .bind(&role)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query(
        "UPDATE app_invite SET accepted_at = NOW(), accepted_user_id = $1 WHERE token = $2",
    )
    .bind(user.id)
    .bind(token)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(user)
}
