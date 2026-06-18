use anyhow::{Context, bail};
use sqlx::PgPool;

use crate::models::admin_user::AdminUser;
use crate::{db, services::admin_auth};

const MIN_PASSWORD_LEN: usize = 8;

/// Exit code returned when admins already exist (safe to ignore in entrypoints).
pub const EXIT_ALREADY_EXISTS: i32 = 1;

pub struct BootstrapAdminOptions {
    pub email: String,
    pub password: String,
}

pub async fn run(database_url: &str, options: BootstrapAdminOptions) -> anyhow::Result<AdminUser> {
    let migrator = db::connect_migrator(database_url).await?;
    sqlx::migrate!("./migrations")
        .run(&migrator)
        .await
        .context("failed to run database migrations")?;

    let db = db::connect(database_url, 2).await?;
    bootstrap_first_admin(&db, &options.email, &options.password).await
}

pub async fn bootstrap_first_admin(
    db: &PgPool,
    email: &str,
    password: &str,
) -> anyhow::Result<AdminUser> {
    let email = email.trim();
    if email.is_empty() {
        bail!("email is required");
    }
    if !email.contains('@') {
        bail!("email must be a valid email address");
    }
    if password.len() < MIN_PASSWORD_LEN {
        bail!("password must be at least {MIN_PASSWORD_LEN} characters");
    }

    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_user")
        .fetch_one(db)
        .await
        .context("failed to count admin users")?;

    if count > 0 {
        bail!("refusing bootstrap: {count} admin user(s) already exist");
    }

    admin_auth::create_admin(db, email, password)
        .await
        .context("failed to create admin user")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bootstrap_refuses_when_admin_exists() {
        let database_url = match std::env::var("DATABASE_URL") {
            Ok(url) if !url.is_empty() => url,
            Err(_) => return,
        };

        let db = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&database_url)
            .await
            .expect("database connection");

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM admin_user")
            .fetch_one(&db)
            .await
            .expect("count admins");

        if count == 0 {
            return;
        }

        let err = bootstrap_first_admin(&db, "bootstrap-test@authstack.local", "password12345")
            .await
            .expect_err("expected bootstrap to fail when admins exist");

        assert!(err.to_string().contains("already exist"));
    }
}
