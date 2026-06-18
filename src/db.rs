use anyhow::{Context, Result};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgConnection, PgPool};

pub async fn configure_connection(conn: &mut PgConnection) -> Result<()> {
    sqlx::query("SET search_path TO tenant, admin, public")
        .execute(conn)
        .await?;
    Ok(())
}

/// Pool for running sqlx migrations. Uses the default search path so `_sqlx_migrations` lives in `public`.
pub async fn connect_migrator(database_url: &str) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(1)
        .connect(database_url)
        .await
        .context("failed to connect to database for migrations")
}

/// Pool where every connection resolves unqualified table names via `tenant`, then `admin`.
pub async fn connect(database_url: &str, max_connections: u32) -> Result<PgPool> {
    PgPoolOptions::new()
        .max_connections(max_connections)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                configure_connection(conn)
                    .await
                    .map_err(|e| sqlx::Error::Configuration(Box::from(e.to_string())))
            })
        })
        .connect(database_url)
        .await
        .context("failed to connect to database")
}

