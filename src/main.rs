use anyhow::Context;
use axum::Router;
use dotenvy::dotenv;
use std::sync::Arc;
use axum::routing::get_service;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cli;
mod commands;
mod config;
mod db;
mod error;
mod ids;
mod jwk;
mod middleware;
mod models;
mod openapi;
mod routes;
mod services;

pub use ids::*;

use cli::Command;
use config::Config;
use services::jwt::JwtService;

#[derive(Clone)]
pub struct AppState {
    pub db: sqlx::PgPool,
    pub config: Arc<Config>,
    pub jwt: Arc<JwtService>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    match cli::parse() {
        Command::OpenApi => {
            println!("{}", serde_json::to_string_pretty(&openapi::spec())?);
            return Ok(());
        }
        Command::BootstrapAdmin(options) => {
            cli::run_bootstrap_admin(options).await;
        }
        Command::Serve => {}
    }

    dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "authstack=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = Arc::new(Config::from_env()?);

    let migrator = db::connect_migrator(&config.database_url).await?;
    sqlx::migrate!("./migrations").run(&migrator).await?;

    let db = db::connect(&config.database_url, 20).await?;

    let jwt = JwtService::new(
        &config.jwt_private_key,
        &config.jwt_public_key,
        config.access_token_expiry_secs,
        config.refresh_token_expiry_secs,
        config.jwt_kid.clone(),
    )
    .context("failed to initialise JWT service — check JWT_PRIVATE_KEY and JWT_PUBLIC_KEY")?;

    let state = AppState {
        db,
        config: config.clone(),
        jwt: Arc::new(jwt),
    };

    let protected = Router::new()
        .merge(routes::auth::router())
        .merge(routes::users::router())
        .merge(routes::orgs::router())
        .merge(routes::members::router())
        .merge(routes::invites::app_router())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::app_auth::authenticate_app,
        ));

    let admin_protected = Router::new()
        .merge(routes::admin::protected_router())
        .merge(routes::admin_sse::router())
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::middleware::admin_auth::authenticate_admin,
        ));

    let static_service = ServeDir::new("static");
    let static_router = Router::new().nest_service("/static", get_service(static_service));

    let app = Router::new()
        .merge(protected)
        .merge(routes::auth::bearer_router())
        .merge(routes::me::router())
        .merge(routes::admin::open_router())
        .merge(admin_protected)
        .merge(routes::invites::public_router())
        .merge(static_router)
        .merge(routes::jwks::router())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = format!("0.0.0.0:{}", config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("authstack listening on {}", addr);
    axum::serve(listener, app).await?;

    Ok(())
}
