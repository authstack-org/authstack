use askama::Template;
use axum::{
    extract::{Extension, Form, State},
    http::{header, HeaderMap},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    error::{AppError, Result},
    ids::{AdminUserId, ApplicationId},
    middleware::admin_auth::AdminIdentity,
    services::{admin_auth, password},
    AppState,
};

// ── Template structs ──────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "admin/login.html")]
struct LoginTemplate {
    error: Option<String>,
    email: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/dashboard.html")]
struct DashboardTemplate {
    admin_email: String,
    applications: Vec<ApplicationRow>,
    new_app: Option<NewAppCredentials>,
    flash: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/new_app.html")]
struct NewAppTemplate {
    admin_email: String,
    error: Option<String>,
    name: Option<String>,
}

struct ApplicationRow {
    name: String,
    app_id: String,
    created_at: DateTime<Utc>,
}

struct NewAppCredentials {
    app_id: String,
    client_secret: String,
}

// ── Routers ───────────────────────────────────────────────────────────────────

/// Open routes: no admin JWT required.
pub fn open_router() -> Router<AppState> {
    Router::new()
        .route("/admin/login", get(login_page).post(process_login))
        .route("/admin/logout", post(logout))
        .route("/admin/users", post(create_admin_user))
}

/// Protected routes: admin JWT cookie required (middleware applied in main.rs).
pub fn protected_router() -> Router<AppState> {
    Router::new()
        .route("/admin/dashboard", get(dashboard))
        .route("/admin/apps/new", get(new_app_page))
        .route("/admin/apps", post(create_app))
        // JSON API for programmatic access (e.g. CI, scripts)
        .route("/admin/applications", post(create_application_json))
}

// ── Open handlers ─────────────────────────────────────────────────────────────

async fn login_page() -> impl IntoResponse {
    render(LoginTemplate { error: None, email: None })
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

async fn process_login(
    State(state): State<AppState>,
    Form(body): Form<LoginForm>,
) -> Response {
    let user = match admin_auth::login_admin(&state.db, &body.email, &body.password).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return render(LoginTemplate {
                error: Some("Invalid email or password.".to_string()),
                email: Some(body.email),
            })
            .into_response();
        }
        Err(_) => {
            return render(LoginTemplate {
                error: Some("Something went wrong. Please try again.".to_string()),
                email: Some(body.email),
            })
            .into_response();
        }
    };

    let token = match state.jwt.issue_admin_token(user.id, &user.email) {
        Ok(t) => t,
        Err(_) => {
            return render(LoginTemplate {
                error: Some("Failed to issue session token.".to_string()),
                email: Some(body.email),
            })
            .into_response();
        }
    };

    let cookie = format!(
        "admin_token={token}; Path=/; HttpOnly; SameSite=Strict"
    );

    (
        [(header::SET_COOKIE, cookie)],
        Redirect::to("/admin/dashboard"),
    )
        .into_response()
}

async fn logout() -> impl IntoResponse {
    let clear = "admin_token=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0".to_string();
    ([(header::SET_COOKIE, clear)], Redirect::to("/admin/login"))
}

// Bootstrap: create first admin user via X-Admin-Key
#[derive(Deserialize)]
struct CreateAdminUserRequest {
    email: String,
    password: String,
}

#[derive(Serialize)]
struct CreateAdminUserResponse {
    id: AdminUserId,
    email: String,
}

async fn create_admin_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateAdminUserRequest>,
) -> Result<Json<CreateAdminUserResponse>> {
    let provided_key = headers
        .get("X-Admin-Key")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided_key != state.config.admin_key {
        return Err(AppError::Unauthorized("invalid admin key".to_string()));
    }

    let user = admin_auth::create_admin(&state.db, &body.email, &body.password)
        .await
        .map_err(|e| {
            if e.to_string().contains("unique") || e.to_string().contains("23505") {
                AppError::Conflict("an admin with that email already exists".to_string())
            } else {
                AppError::Internal(e)
            }
        })?;

    Ok(Json(CreateAdminUserResponse {
        id: user.id,
        email: user.email,
    }))
}

// ── Protected handlers ────────────────────────────────────────────────────────

async fn dashboard(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminIdentity>,
) -> Response {
    let rows: Vec<(String, String, DateTime<Utc>)> = match sqlx::query_as(
        "SELECT name, id, created_at FROM application ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(r) => r,
        Err(_) => return AppError::Internal(anyhow::anyhow!("db error")).into_response(),
    };

    let applications = rows
        .into_iter()
        .map(|(name, app_id, created_at)| ApplicationRow {
            name,
            app_id,
            created_at,
        })
        .collect();

    render(DashboardTemplate {
        admin_email: identity.email,
        applications,
        new_app: None,
        flash: None,
    })
    .into_response()
}

async fn new_app_page(Extension(identity): Extension<AdminIdentity>) -> impl IntoResponse {
    render(NewAppTemplate {
        admin_email: identity.email,
        error: None,
        name: None,
    })
}

#[derive(Deserialize)]
struct CreateAppForm {
    name: String,
}

async fn create_app(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminIdentity>,
    Form(body): Form<CreateAppForm>,
) -> Response {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return render(NewAppTemplate {
            admin_email: identity.email,
            error: Some("Application name is required.".to_string()),
            name: Some(name),
        })
        .into_response();
    }

    let id = ApplicationId::new();
    let client_secret = format!("secret_{}", &Uuid::new_v4().to_string().replace('-', "")[..32]);
    let secret_hash = match password::hash(&client_secret) {
        Ok(h) => h,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let result = sqlx::query(
        "INSERT INTO application (id, client_secret_hash, name) VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(&secret_hash)
    .bind(&name)
    .execute(&state.db)
    .await;

    if let Err(e) = result {
        return AppError::Database(e).into_response();
    }

    // Re-fetch all apps to show dashboard with credentials
    let rows: Vec<(String, String, DateTime<Utc>)> = match sqlx::query_as(
        "SELECT name, id, created_at FROM application ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    {
        Ok(r) => r,
        Err(e) => return AppError::Database(e).into_response(),
    };

    let applications = rows
        .into_iter()
        .map(|(n, app_id, created_at)| ApplicationRow {
            name: n,
            app_id,
            created_at,
        })
        .collect();

    render(DashboardTemplate {
        admin_email: identity.email,
        applications,
        new_app: Some(NewAppCredentials {
            app_id: id.to_string(),
            client_secret,
        }),
        flash: None,
    })
    .into_response()
}

// JSON API: create application — for programmatic / CI use
#[derive(Deserialize)]
struct CreateApplicationJsonRequest {
    name: String,
}

#[derive(Serialize)]
struct CreateApplicationJsonResponse {
    id: ApplicationId,
    client_secret: String,
    name: String,
}

async fn create_application_json(
    State(state): State<AppState>,
    Extension(_identity): Extension<AdminIdentity>,
    Json(body): Json<CreateApplicationJsonRequest>,
) -> Result<(axum::http::StatusCode, Json<CreateApplicationJsonResponse>)> {
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("name is required".to_string()));
    }

    let id = ApplicationId::new();
    let client_secret = format!("secret_{}", &Uuid::new_v4().to_string().replace('-', "")[..32]);
    let secret_hash = password::hash(&client_secret).map_err(AppError::Internal)?;

    sqlx::query(
        "INSERT INTO application (id, client_secret_hash, name) VALUES ($1, $2, $3)",
    )
    .bind(id)
    .bind(&secret_hash)
    .bind(&name)
    .execute(&state.db)
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(CreateApplicationJsonResponse {
            id,
            client_secret,
            name,
        }),
    ))
}

// ── Helper ────────────────────────────────────────────────────────────────────

fn render<T: Template>(tmpl: T) -> Html<String> {
    match tmpl.render() {
        Ok(html) => Html(html),
        Err(e) => Html(format!("<pre>Template error: {e}</pre>")),
    }
}
