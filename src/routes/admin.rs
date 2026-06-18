use askama::Template;
use axum::{
    Json, Router,
    extract::{Extension, Form, Path, State},
    http::header,
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    AppState,
    error::{AppError, Result},
    ids::ApplicationId,
    services::admin_auth::AdminSession,
    models::admin_role::AdminRole,
    services::{admin_ops, auth as auth_service, password},
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
    is_instance_admin: bool,
    applications: Vec<ApplicationRow>,
    new_app: Option<NewAppCredentials>,
    flash: Option<String>,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/new_app.html")]
struct NewAppTemplate {
    admin_email: String,
    is_instance_admin: bool,
    error: Option<String>,
    name: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/app_detail.html")]
struct AppDetailTemplate {
    admin_email: String,
    is_instance_admin: bool,
    app_id: String,
    app_name: String,
    created_at: DateTime<Utc>,
    user_count: i64,
    flash: Option<String>,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/app_users.html")]
struct AppUsersTemplate {
    admin_email: String,
    is_instance_admin: bool,
    app_id: String,
    app_name: String,
    users: Vec<TenantUserRow>,
    flash: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/new_app_user.html")]
struct NewAppUserTemplate {
    admin_email: String,
    is_instance_admin: bool,
    app_id: String,
    app_name: String,
    error: Option<String>,
    name: Option<String>,
    email: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/app_orgs.html")]
struct AppOrgsTemplate {
    admin_email: String,
    is_instance_admin: bool,
    app_id: String,
    app_name: String,
    orgs: Vec<OrgRow>,
    flash: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/org_detail.html")]
struct OrgDetailTemplate {
    admin_email: String,
    is_instance_admin: bool,
    app_id: String,
    app_name: String,
    org_id: String,
    org_name: String,
    org_slug: String,
    org_type: String,
    org_created_at: DateTime<Utc>,
    members: Vec<OrgMemberDisplay>,
    error: Option<String>,
    flash: Option<String>,
}

struct OrgRow {
    id: String,
    name: String,
    slug: String,
    org_type: String,
    member_count: i64,
    created_at: DateTime<Utc>,
}

struct OrgMemberDisplay {
    id: String,
    user_id: String,
    user_name: String,
    user_email: String,
    role: String,
    created_at: DateTime<Utc>,
}

#[derive(Template)]
#[template(path = "admin/operators.html")]
struct OperatorsTemplate {
    admin_email: String,
    is_instance_admin: bool,
    operators: Vec<OperatorRow>,
    flash: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/new_operator.html")]
struct NewOperatorTemplate {
    admin_email: String,
    is_instance_admin: bool,
    applications: Vec<AppPickerRow>,
    error: Option<String>,
    email: Option<String>,
    role: Option<String>,
}

struct ApplicationRow {
    name: String,
    app_id: String,
    created_at: DateTime<Utc>,
    user_count: i64,
}

struct NewAppCredentials {
    app_id: String,
    client_secret: String,
}

struct TenantUserRow {
    id: String,
    name: String,
    email: String,
    email_verified: bool,
    created_at: DateTime<Utc>,
}

struct OperatorRow {
    id: String,
    email: String,
    role_label: String,
    apps_label: String,
    created_at: DateTime<Utc>,
}

struct AppPickerRow {
    app_id: String,
    name: String,
    selected: bool,
}

// ── Routers ───────────────────────────────────────────────────────────────────

pub fn open_router() -> Router<AppState> {
    Router::new()
        .route("/admin/login", get(login_page).post(process_login))
        .route("/admin/logout", post(logout))
}

pub fn protected_router() -> Router<AppState> {
    Router::new()
        .route("/admin/dashboard", get(dashboard))
        .route("/admin/operators", get(operators_page))
        .route("/admin/operators/new", get(new_operator_page).post(create_operator))
        .route("/admin/apps/new", get(new_app_page))
        .route("/admin/apps", post(create_app))
        .route("/admin/apps/{app_id}", get(app_detail))
        .route("/admin/apps/{app_id}/delete", post(delete_app))
        .route("/admin/apps/{app_id}/users", get(app_users))
        .route(
            "/admin/apps/{app_id}/users/new",
            get(new_app_user_page).post(create_app_user),
        )
        .route("/admin/apps/{app_id}/orgs", get(app_orgs))
        .route("/admin/apps/{app_id}/orgs/{org_id}", get(org_detail))
        .route(
            "/admin/apps/{app_id}/orgs/{org_id}/members",
            post(add_org_member),
        )
        .route(
            "/admin/apps/{app_id}/orgs/{org_id}/members/{user_id}/remove",
            post(remove_org_member),
        )
        .route("/admin/applications", post(create_application_json))
}

// ── Open handlers ─────────────────────────────────────────────────────────────

async fn login_page() -> impl IntoResponse {
    render(LoginTemplate {
        error: None,
        email: None,
    })
}

#[derive(Deserialize)]
struct LoginForm {
    email: String,
    password: String,
}

async fn process_login(State(state): State<AppState>, Form(body): Form<LoginForm>) -> Response {
    let user = match crate::services::admin_auth::login_admin(&state.db, &body.email, &body.password)
        .await
    {
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

    let cookie = format!("admin_token={token}; Path=/; HttpOnly; SameSite=Strict");

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

// ── Protected handlers ────────────────────────────────────────────────────────

async fn dashboard(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
) -> Response {
    let apps = match admin_ops::list_applications_for_admin(&state.db, identity.role, identity.admin_id)
        .await
    {
        Ok(a) => a,
        Err(_) => return AppError::Internal(anyhow::anyhow!("db error")).into_response(),
    };

    let applications = apps
        .into_iter()
        .map(|a| ApplicationRow {
            name: a.name,
            app_id: a.id.to_string(),
            created_at: a.created_at,
            user_count: a.user_count,
        })
        .collect();

    let (admin_email, is_instance_admin) = nav_context(&identity);

    render(DashboardTemplate {
        admin_email,
        is_instance_admin,
        applications,
        new_app: None,
        flash: None,
        error: None,
    })
    .into_response()
}

async fn new_app_page(Extension(identity): Extension<AdminSession>) -> Response {
    if !identity.is_instance_admin() {
        return Redirect::to("/admin/dashboard").into_response();
    }

    render(NewAppTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: true,
        error: None,
        name: None,
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateAppForm {
    name: String,
}

async fn create_app(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Form(body): Form<CreateAppForm>,
) -> Response {
    if !identity.is_instance_admin() {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let name = body.name.trim().to_string();
    if name.is_empty() {
        return render(NewAppTemplate {
            admin_email: identity.email.clone(),
            is_instance_admin: true,
            error: Some("Application name is required.".to_string()),
            name: Some(name),
        })
        .into_response();
    }

    let id = ApplicationId::new();
    let client_secret = format!(
        "secret_{}",
        &Uuid::new_v4().to_string().replace('-', "")[..32]
    );
    let secret_hash = match password::hash(&client_secret) {
        Ok(h) => h,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    if let Err(e) = sqlx::query("INSERT INTO application (id, client_secret_hash, name) VALUES ($1, $2, $3)")
        .bind(id)
        .bind(&secret_hash)
        .bind(&name)
        .execute(&state.db)
        .await
    {
        return AppError::Database(e).into_response();
    }

    let apps = match admin_ops::list_applications_for_admin(&state.db, identity.role, identity.admin_id)
        .await
    {
        Ok(a) => a,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let applications = apps
        .into_iter()
        .map(|a| ApplicationRow {
            name: a.name,
            app_id: a.id.to_string(),
            created_at: a.created_at,
            user_count: a.user_count,
        })
        .collect();

    let (admin_email, is_instance_admin) = nav_context(&identity);

    render(DashboardTemplate {
        admin_email,
        is_instance_admin,
        applications,
        new_app: Some(NewAppCredentials {
            app_id: id.to_string(),
            client_secret,
        }),
        flash: None,
        error: None,
    })
    .into_response()
}

async fn app_detail(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !identity.can_access_app(app_id) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(AppDetailTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: identity.is_instance_admin(),
        app_id: app.id.to_string(),
        app_name: app.name,
        created_at: app.created_at,
        user_count: app.user_count,
        flash: None,
        error: None,
    })
    .into_response()
}

async fn delete_app(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
) -> Response {
    if !identity.is_instance_admin() {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    match admin_ops::delete_application(&state.db, app_id).await {
        Ok(true) => Redirect::to("/admin/dashboard").into_response(),
        Ok(false) => render(AppDetailTemplate {
            admin_email: identity.email.clone(),
            is_instance_admin: true,
            app_id: app.id.to_string(),
            app_name: app.name,
            created_at: app.created_at,
            user_count: app.user_count,
            flash: None,
            error: Some("Application could not be deleted.".to_string()),
        })
        .into_response(),
        Err(e) => AppError::Internal(e).into_response(),
    }
}

async fn app_users(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !identity.can_access_app(app_id) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let users = match admin_ops::list_tenant_users(&state.db, app_id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(AppUsersTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: identity.is_instance_admin(),
        app_id: app.id.to_string(),
        app_name: app.name,
        users: users
            .into_iter()
            .map(|u| TenantUserRow {
                id: u.id,
                name: u.name,
                email: u.email,
                email_verified: u.email_verified,
                created_at: u.created_at,
            })
            .collect(),
        flash: None,
    })
    .into_response()
}

async fn new_app_user_page(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !identity.can_access_app(app_id) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(NewAppUserTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: identity.is_instance_admin(),
        app_id: app.id.to_string(),
        app_name: app.name,
        error: None,
        name: None,
        email: None,
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateAppUserForm {
    name: String,
    email: String,
    password: String,
}

async fn create_app_user(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
    Form(body): Form<CreateAppUserForm>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !identity.can_access_app(app_id) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let name = body.name.trim().to_string();
    let email = body.email.trim().to_string();

    if name.is_empty() || email.is_empty() {
        return render(NewAppUserTemplate {
            admin_email: identity.email.clone(),
            is_instance_admin: identity.is_instance_admin(),
            app_id: app.id.to_string(),
            app_name: app.name,
            error: Some("Name and email are required.".to_string()),
            name: Some(name),
            email: Some(email),
        })
        .into_response();
    }

    if body.password.len() < 8 {
        return render(NewAppUserTemplate {
            admin_email: identity.email.clone(),
            is_instance_admin: identity.is_instance_admin(),
            app_id: app.id.to_string(),
            app_name: app.name,
            error: Some("Password must be at least 8 characters.".to_string()),
            name: Some(name),
            email: Some(email),
        })
        .into_response();
    }

    match auth_service::signup(&state.db, app_id, &name, &email, &body.password).await {
        Ok(_) => Redirect::to(&format!("/admin/apps/{}/users", app.id)).into_response(),
        Err(AppError::Conflict(_)) => render(NewAppUserTemplate {
            admin_email: identity.email.clone(),
            is_instance_admin: identity.is_instance_admin(),
            app_id: app.id.to_string(),
            app_name: app.name,
            error: Some("A user with that email already exists in this application.".to_string()),
            name: Some(name),
            email: Some(email),
        })
        .into_response(),
        Err(e) => AppError::Internal(anyhow::anyhow!(e.to_string())).into_response(),
    }
}

async fn app_orgs(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !identity.can_access_app(app_id) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let orgs = match admin_ops::list_orgs_for_app(&state.db, app_id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(AppOrgsTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: identity.is_instance_admin(),
        app_id: app.id.to_string(),
        app_name: app.name,
        orgs: orgs
            .into_iter()
            .map(|o| OrgRow {
                id: o.id,
                name: o.name,
                slug: o.slug,
                org_type: o.org_type,
                member_count: o.member_count,
                created_at: o.created_at,
            })
            .collect(),
        flash: None,
    })
    .into_response()
}

async fn org_detail(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path((app_id, org_id)): Path<(String, String)>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !identity.can_access_app(app_id) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let org = match admin_ops::get_org_for_app(&state.db, app_id, &org_id).await {
        Ok(Some(o)) => o,
        Ok(None) => return Redirect::to(&format!("/admin/apps/{}/orgs", app.id)).into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let members = match admin_ops::list_org_members(&state.db, &org_id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(OrgDetailTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: identity.is_instance_admin(),
        app_id: app.id.to_string(),
        app_name: app.name,
        org_id: org.id,
        org_name: org.name,
        org_slug: org.slug,
        org_type: org.org_type,
        org_created_at: org.created_at,
        members: members
            .into_iter()
            .map(|m| OrgMemberDisplay {
                id: m.id,
                user_id: m.user_id,
                user_name: m.user_name,
                user_email: m.user_email,
                role: m.role,
                created_at: m.created_at,
            })
            .collect(),
        error: None,
        flash: None,
    })
    .into_response()
}

#[derive(Deserialize)]
struct AddOrgMemberForm {
    user_id: String,
    role: Option<String>,
}

async fn add_org_member(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path((app_id, org_id)): Path<(String, String)>,
    Form(body): Form<AddOrgMemberForm>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !identity.can_access_app(app_id) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let org = match admin_ops::get_org_for_app(&state.db, app_id, &org_id).await {
        Ok(Some(o)) => o,
        Ok(None) => return Redirect::to(&format!("/admin/apps/{}/orgs", app.id)).into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let user_id = body.user_id.trim().to_string();
    if user_id.is_empty() {
        return render_org_detail_error(&state, &identity, &app, &org, "Select a user to add.").await;
    }

    let role = body.role.unwrap_or_else(|| "member".to_string());

    match admin_ops::add_org_member(&state.db, app_id, &org_id, &user_id, &role).await {
        Ok(()) => Redirect::to(&format!("/admin/apps/{}/orgs/{}", app.id, org_id)).into_response(),
        Err(e) if e.to_string().contains("23505") => {
            render_org_detail_error(
                &state,
                &identity,
                &app,
                &org,
                "That user is already a member of this organization.",
            )
            .await
        }
        Err(e) => render_org_detail_error(&state, &identity, &app, &org, &e.to_string()).await,
    }
}

async fn remove_org_member(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path((app_id, org_id, user_id)): Path<(String, String, String)>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if !identity.can_access_app(app_id) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    match admin_ops::remove_org_member(&state.db, app_id, &org_id, &user_id).await {
        Ok(true) => Redirect::to(&format!("/admin/apps/{}/orgs/{}", app.id, org_id)).into_response(),
        Ok(false) => Redirect::to(&format!("/admin/apps/{}/orgs/{}", app.id, org_id)).into_response(),
        Err(e) => {
            let org = match admin_ops::get_org_for_app(&state.db, app_id, &org_id).await {
                Ok(Some(o)) => o,
                _ => return AppError::Internal(e).into_response(),
            };
            render_org_detail_error(&state, &identity, &app, &org, &e.to_string()).await
        }
    }
}

async fn render_org_detail_error(
    state: &AppState,
    identity: &AdminSession,
    app: &admin_ops::ApplicationSummary,
    org: &admin_ops::OrgDetail,
    error: &str,
) -> Response {
    let members = match admin_ops::list_org_members(&state.db, &org.id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(OrgDetailTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: identity.is_instance_admin(),
        app_id: app.id.to_string(),
        app_name: app.name.clone(),
        org_id: org.id.clone(),
        org_name: org.name.clone(),
        org_slug: org.slug.clone(),
        org_type: org.org_type.clone(),
        org_created_at: org.created_at,
        members: members
            .into_iter()
            .map(|m| OrgMemberDisplay {
                id: m.id,
                user_id: m.user_id,
                user_name: m.user_name,
                user_email: m.user_email,
                role: m.role,
                created_at: m.created_at,
            })
            .collect(),
        error: Some(error.to_string()),
        flash: None,
    })
    .into_response()
}

async fn operators_page(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
) -> Response {
    if !identity.is_instance_admin() {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let operators = match admin_ops::list_operators(&state.db).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(OperatorsTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: true,
        operators: operators
            .into_iter()
            .map(|op| OperatorRow {
                id: op.id.to_string(),
                email: op.email,
                role_label: role_label(op.role),
                apps_label: if op.role == AdminRole::InstanceAdmin {
                    "All applications".to_string()
                } else if op.granted_app_ids.is_empty() {
                    "No applications".to_string()
                } else {
                    op.granted_app_ids
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                created_at: op.created_at,
            })
            .collect(),
        flash: None,
    })
    .into_response()
}

async fn new_operator_page(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
) -> Response {
    if !identity.is_instance_admin() {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let apps = match admin_ops::list_all_applications_for_picker(&state.db).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(NewOperatorTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: true,
        applications: apps
            .into_iter()
            .map(|(id, name)| AppPickerRow {
                app_id: id.to_string(),
                name,
                selected: false,
            })
            .collect(),
        error: None,
        email: None,
        role: Some("app_admin".to_string()),
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateOperatorForm {
    email: String,
    password: String,
    role: String,
    #[serde(default, deserialize_with = "deserialize_form_string_vec")]
    app_ids: Vec<String>,
}

fn deserialize_form_string_vec<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }

    match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(value) => Ok(vec![value]),
        OneOrMany::Many(values) => Ok(values),
    }
}

async fn create_operator(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Form(body): Form<CreateOperatorForm>,
) -> Response {
    if !identity.is_instance_admin() {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let apps = match admin_ops::list_all_applications_for_picker(&state.db).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let picker: Vec<AppPickerRow> = apps
        .iter()
        .map(|(id, name)| AppPickerRow {
            app_id: id.to_string(),
            name: name.clone(),
            selected: body.app_ids.iter().any(|s| s == &id.to_string()),
        })
        .collect();

    let role = match body.role.parse::<AdminRole>() {
        Ok(r) => r,
        Err(_) => {
            return render(NewOperatorTemplate {
                admin_email: identity.email.clone(),
                is_instance_admin: true,
                applications: picker,
                error: Some("Invalid operator role.".to_string()),
                email: Some(body.email),
                role: Some(body.role),
            })
            .into_response();
        }
    };

    let mut app_ids = Vec::new();
    for raw in &body.app_ids {
        match raw.parse::<ApplicationId>() {
            Ok(id) => app_ids.push(id),
            Err(_) => {
                return render(NewOperatorTemplate {
                    admin_email: identity.email.clone(),
                    is_instance_admin: true,
                    applications: picker,
                    error: Some(format!("Invalid application id: {raw}")),
                    email: Some(body.email),
                    role: Some(body.role),
                })
                .into_response();
            }
        }
    }

    match admin_ops::create_operator(&state.db, &body.email, &body.password, role, &app_ids).await {
        Ok(_) => Redirect::to("/admin/operators").into_response(),
        Err(e) if e.to_string().contains("unique") || e.to_string().contains("23505") => {
            render(NewOperatorTemplate {
                admin_email: identity.email.clone(),
                is_instance_admin: true,
                applications: picker,
                error: Some("An operator with that email already exists.".to_string()),
                email: Some(body.email),
                role: Some(body.role),
            })
            .into_response()
        }
        Err(e) => render(NewOperatorTemplate {
            admin_email: identity.email.clone(),
            is_instance_admin: true,
            applications: picker,
            error: Some(e.to_string()),
            email: Some(body.email),
            role: Some(body.role),
        })
        .into_response(),
    }
}

#[derive(Deserialize)]
struct CreateApplicationJsonRequest {
    name: String,
}

#[derive(serde::Serialize)]
struct CreateApplicationJsonResponse {
    id: ApplicationId,
    client_secret: String,
    name: String,
}

async fn create_application_json(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Json(body): Json<CreateApplicationJsonRequest>,
) -> Result<(axum::http::StatusCode, Json<CreateApplicationJsonResponse>)> {
    if !identity.is_instance_admin() {
        return Err(AppError::Forbidden);
    }

    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("name is required".to_string()));
    }

    let id = ApplicationId::new();
    let client_secret = format!(
        "secret_{}",
        &Uuid::new_v4().to_string().replace('-', "")[..32]
    );
    let secret_hash = password::hash(&client_secret).map_err(AppError::Internal)?;

    sqlx::query("INSERT INTO application (id, client_secret_hash, name) VALUES ($1, $2, $3)")
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

// ── Helpers ───────────────────────────────────────────────────────────────────

fn nav_context(identity: &AdminSession) -> (String, bool) {
    (identity.email.clone(), identity.is_instance_admin())
}

fn role_label(role: AdminRole) -> String {
    match role {
        AdminRole::InstanceAdmin => "Instance admin".to_string(),
        AdminRole::AppAdmin => "App admin".to_string(),
    }
}

fn parse_app_id(raw: &str) -> std::result::Result<ApplicationId, Response> {
    raw.parse::<ApplicationId>()
        .map_err(|_| Redirect::to("/admin/dashboard").into_response())
}

fn render<T: Template>(tmpl: T) -> Html<String> {
    match tmpl.render() {
        Ok(html) => Html(html),
        Err(e) => Html(format!("<pre>Template error: {e}</pre>")),
    }
}
