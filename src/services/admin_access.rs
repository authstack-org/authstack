use anyhow::Result;
use sqlx::PgPool;

use crate::ids::{ApplicationId, DirectoryId};
use crate::models::admin_role::AdminRole;
use crate::services::admin_auth::AdminSession;

pub fn can_manage_directories(session: &AdminSession) -> bool {
    session.is_instance_admin()
}

pub fn can_view_directories(session: &AdminSession) -> bool {
    can_manage_directories(session) || session.role == AdminRole::DirectoryAdmin
}

pub fn can_manage_operators(session: &AdminSession) -> bool {
    matches!(
        session.role,
        AdminRole::InstanceAdmin | AdminRole::DirectoryAdmin
    )
}

pub fn can_create_applications(session: &AdminSession) -> bool {
    matches!(
        session.role,
        AdminRole::InstanceAdmin | AdminRole::DirectoryAdmin
    )
}

pub fn can_delete_applications(session: &AdminSession) -> bool {
    can_create_applications(session)
}

pub fn can_assign_role(creator: &AdminSession, role: AdminRole) -> bool {
    match creator.role {
        AdminRole::InstanceAdmin => true,
        AdminRole::DirectoryAdmin => {
            matches!(role, AdminRole::AppAdmin | AdminRole::DirectoryAdmin)
        }
        AdminRole::AppAdmin => false,
    }
}

pub async fn can_access_directory(
    db: &PgPool,
    session: &AdminSession,
    directory_id: DirectoryId,
) -> Result<bool> {
    if session.is_instance_admin() {
        return Ok(true);
    }
    if session.role == AdminRole::DirectoryAdmin {
        return Ok(session.granted_directory_ids.contains(&directory_id));
    }
    if session.role == AdminRole::AppAdmin {
        let count: i64 = sqlx::query_scalar(
            r#"SELECT COUNT(*)::bigint
               FROM application a
               INNER JOIN admin_app_grant g ON g.app_id = a.id
               WHERE g.admin_user_id = $1 AND a.directory_id = $2"#,
        )
        .bind(session.admin_id)
        .bind(directory_id)
        .fetch_one(db)
        .await?;
        return Ok(count > 0);
    }
    Ok(false)
}

pub async fn can_access_app(
    db: &PgPool,
    session: &AdminSession,
    app_id: ApplicationId,
) -> Result<bool> {
    if session.is_instance_admin() {
        return Ok(true);
    }
    if session.role == AdminRole::AppAdmin {
        return Ok(session.granted_app_ids.contains(&app_id));
    }
    if session.role == AdminRole::DirectoryAdmin {
        let directory_id: Option<DirectoryId> =
            sqlx::query_scalar("SELECT directory_id FROM application WHERE id = $1")
                .bind(app_id)
                .fetch_optional(db)
                .await?;
        return Ok(directory_id
            .map(|id| session.granted_directory_ids.contains(&id))
            .unwrap_or(false));
    }
    Ok(false)
}

pub async fn ensure_apps_in_scope(
    db: &PgPool,
    session: &AdminSession,
    app_ids: &[ApplicationId],
) -> Result<()> {
    for app_id in app_ids {
        if !can_access_app(db, session, *app_id).await? {
            anyhow::bail!("application {app_id} is outside your scope");
        }
    }
    Ok(())
}

pub async fn ensure_directories_in_scope(
    db: &PgPool,
    session: &AdminSession,
    directory_ids: &[DirectoryId],
) -> Result<()> {
    for directory_id in directory_ids {
        if !can_access_directory(db, session, *directory_id).await? {
            anyhow::bail!("directory {directory_id} is outside your scope");
        }
    }
    Ok(())
}
