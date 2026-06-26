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
    ids::{ApplicationId, DirectoryId, OrganizationId},
    services::admin_auth::AdminSession,
    models::admin_role::AdminRole,
    services::{admin_access, admin_ops, auth as auth_service, identity, invites, password, roles},
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
    can_manage_operators: bool,
    can_create_applications: bool,
    show_directories_nav: bool,
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
    can_manage_operators: bool,
    show_directories_nav: bool,
    directories: Vec<DirectorySelectRow>,
    error: Option<String>,
    name: Option<String>,
    directory_id: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/directories.html")]
struct DirectoriesTemplate {
    admin_email: String,
    is_instance_admin: bool,
    can_manage_directories: bool,
    can_manage_operators: bool,
    directories: Vec<DirectoryRow>,
    flash: Option<String>,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/directory_detail.html")]
struct DirectoryDetailTemplate {
    admin_email: String,
    is_instance_admin: bool,
    can_manage_operators: bool,
    can_manage_directories: bool,
    directory_id: String,
    directory_name: String,
    directory_slug: String,
    application_count: i64,
    created_at: DateTime<Utc>,
    admins: Vec<DirectoryAdminDisplay>,
    flash: Option<String>,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/new_directory_admin.html")]
struct NewDirectoryAdminTemplate {
    admin_email: String,
    is_instance_admin: bool,
    can_manage_operators: bool,
    directory_id: String,
    directory_name: String,
    error: Option<String>,
    email: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/new_directory.html")]
struct NewDirectoryTemplate {
    admin_email: String,
    is_instance_admin: bool,
    error: Option<String>,
    name: Option<String>,
    slug: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/app_detail.html")]
struct AppDetailTemplate {
    admin_email: String,
    is_instance_admin: bool,
    can_delete_applications: bool,
    can_manage_operators: bool,
    show_directories_nav: bool,
    app_id: String,
    app_name: String,
    created_at: DateTime<Utc>,
    user_count: i64,
    flash: Option<String>,
    error: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/app_permissions.html")]
struct AppPermissionsTemplate {
    admin_email: String,
    is_instance_admin: bool,
    can_manage_operators: bool,
    show_directories_nav: bool,
    app_id: String,
    app_name: String,
    permissions: Vec<AppPermissionDisplay>,
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
    pending_invites: Vec<AppInviteDisplay>,
    new_invite_url: Option<String>,
    flash: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/invite_app_user.html")]
struct InviteAppUserTemplate {
    admin_email: String,
    is_instance_admin: bool,
    app_id: String,
    app_name: String,
    organizations: Vec<OrgSelectRow>,
    error: Option<String>,
    name: Option<String>,
    email: Option<String>,
    org_id: Option<String>,
    role: Option<String>,
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
#[template(path = "admin/new_team_org.html")]
struct NewTeamOrgTemplate {
    admin_email: String,
    is_instance_admin: bool,
    app_id: String,
    app_name: String,
    error: Option<String>,
    name: Option<String>,
    slug: Option<String>,
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
    org_created_at: DateTime<Utc>,
    members: Vec<OrgMemberDisplay>,
    pending_invites: Vec<InviteDisplay>,
    org_roles: Vec<OrgRoleDisplay>,
    app_permissions: Vec<AppPermissionDisplay>,
    new_invite_url: Option<String>,
    error: Option<String>,
    flash: Option<String>,
}

struct OrgRoleDisplay {
    id: String,
    slug: String,
    name: String,
    description: Option<String>,
    permission_assignments: Vec<PermissionAssignmentDisplay>,
    member_count: i64,
}

struct PermissionAssignmentDisplay {
    id: String,
    key: String,
    name: String,
    checked: bool,
}

struct AppPermissionDisplay {
    id: String,
    key: String,
    name: String,
    description: Option<String>,
}

struct OrgRow {
    id: String,
    name: String,
    slug: String,
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

struct InviteDisplay {
    email: String,
    role: String,
    invite_url: String,
    expires_at: DateTime<Utc>,
}

struct AppInviteDisplay {
    email: String,
    role: String,
    invite_url: String,
    expires_at: DateTime<Utc>,
    org_id: String,
    org_name: String,
}

struct OrgSelectRow {
    org_id: String,
    label: String,
    selected: bool,
}

#[derive(Template)]
#[template(path = "admin/operators.html")]
struct OperatorsTemplate {
    admin_email: String,
    is_instance_admin: bool,
    can_manage_operators: bool,
    show_directories_nav: bool,
    operators: Vec<OperatorRow>,
    flash: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/new_operator.html")]
struct NewOperatorTemplate {
    admin_email: String,
    is_instance_admin: bool,
    can_manage_operators: bool,
    show_directories_nav: bool,
    show_instance_admin_role: bool,
    applications: Vec<AppPickerRow>,
    directories: Vec<DirectorySelectRow>,
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
    scope_label: String,
    created_at: DateTime<Utc>,
}

struct AppPickerRow {
    app_id: String,
    name: String,
    selected: bool,
}

struct DirectorySelectRow {
    directory_id: String,
    label: String,
    selected: bool,
}

struct DirectoryRow {
    directory_id: String,
    name: String,
    slug: String,
    application_count: i64,
    admin_count: i64,
    created_at: DateTime<Utc>,
}

struct DirectoryAdminDisplay {
    email: String,
    created_at: DateTime<Utc>,
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
        .route("/admin/directories", get(directories_page))
        .route(
            "/admin/directories/new",
            get(new_directory_page).post(create_directory),
        )
        .route("/admin/directories/{directory_id}", get(directory_detail))
        .route(
            "/admin/directories/{directory_id}/admins/new",
            get(new_directory_admin_page).post(create_directory_admin),
        )
        .route("/admin/apps/new", get(new_app_page))
        .route("/admin/apps", post(create_app))
        .route("/admin/apps/{app_id}", get(app_detail))
        .route("/admin/apps/{app_id}/delete", post(delete_app))
        .route("/admin/apps/{app_id}/users", get(app_users))
        .route(
            "/admin/apps/{app_id}/users/new",
            get(new_app_user_page).post(create_app_user),
        )
        .route(
            "/admin/apps/{app_id}/users/invite",
            get(invite_app_user_page).post(create_app_user_invite),
        )
        .route("/admin/apps/{app_id}/orgs", get(app_orgs))
        .route(
            "/admin/apps/{app_id}/orgs/new",
            get(new_team_org_page).post(create_team_org),
        )
        .route("/admin/apps/{app_id}/orgs/{org_id}", get(org_detail))
        .route(
            "/admin/apps/{app_id}/orgs/{org_id}/members",
            post(add_org_member),
        )
        .route(
            "/admin/apps/{app_id}/orgs/{org_id}/members/{user_id}/remove",
            post(remove_org_member),
        )
        .route(
            "/admin/apps/{app_id}/orgs/{org_id}/invites",
            post(create_org_invite),
        )
        .route("/admin/apps/{app_id}/permissions", get(app_permissions_page).post(create_app_permission_admin))
        .route(
            "/admin/apps/{app_id}/permissions/{perm_id}/delete",
            post(delete_app_permission_admin),
        )
        .route(
            "/admin/apps/{app_id}/orgs/{org_id}/roles",
            post(create_org_role_admin),
        )
        .route(
            "/admin/apps/{app_id}/orgs/{org_id}/roles/{role_id}/update",
            post(update_org_role_admin),
        )
        .route(
            "/admin/apps/{app_id}/orgs/{org_id}/roles/{role_id}/delete",
            post(delete_org_role_admin),
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
    let apps = match admin_ops::list_applications_for_admin(&state.db, &identity).await
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

    let nav = nav_context(&identity);

    render(DashboardTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_manage_operators: nav.can_manage_operators,
        can_create_applications: nav.can_create_applications,
        show_directories_nav: nav.show_directories_nav,
        applications,
        new_app: None,
        flash: None,
        error: None,
    })
    .into_response()
}

async fn new_app_page(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
) -> Response {
    if !admin_access::can_create_applications(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let directories = match directory_select_rows(&state.db, &identity, None).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let nav = nav_context(&identity);
    render(NewAppTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_manage_operators: nav.can_manage_operators,
        show_directories_nav: nav.show_directories_nav,
        directories,
        error: None,
        name: None,
        directory_id: None,
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateAppForm {
    name: String,
    directory_id: Option<String>,
}

async fn create_app(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Form(body): Form<CreateAppForm>,
) -> Response {
    if !admin_access::can_create_applications(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let name = body.name.trim().to_string();
    let directory_id = match parse_directory_id_option(body.directory_id.as_deref()) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if let Some(dir_id) = directory_id {
        match admin_access::can_access_directory(&state.db, &identity, dir_id).await {
            Ok(true) => {}
            Ok(false) => {
                return render_new_app_error(
                    &state,
                    &identity,
                    "You do not have access to that directory.",
                    Some(name),
                    directory_id,
                )
                .await
            }
            Err(e) => return AppError::Internal(e).into_response(),
        }
    }

    if name.is_empty() {
        return render_new_app_error(
            &state,
            &identity,
            "Application name is required.",
            Some(name),
            directory_id,
        )
        .await;
    }

    let client_secret = format!(
        "secret_{}",
        &Uuid::new_v4().to_string().replace('-', "")[..32]
    );
    let secret_hash = match password::hash(&client_secret) {
        Ok(h) => h,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let id = match admin_ops::create_application(
        &state.db,
        &name,
        &secret_hash,
        directory_id,
    )
    .await
    {
        Ok(id) => id,
        Err(e) => {
            return render_new_app_error(&state, &identity, &e.to_string(), Some(name), directory_id)
                .await
        }
    };

    let apps = match admin_ops::list_applications_for_admin(&state.db, &identity).await
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

    let nav = nav_context(&identity);

    render(DashboardTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_manage_operators: nav.can_manage_operators,
        can_create_applications: nav.can_create_applications,
        show_directories_nav: nav.show_directories_nav,
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
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let nav = nav_context(&identity);
    render(AppDetailTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_delete_applications: nav.can_delete_applications,
        can_manage_operators: nav.can_manage_operators,
        show_directories_nav: nav.show_directories_nav,
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
    if !admin_access::can_delete_applications(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    match admin_ops::delete_application(&state.db, app_id).await {
        Ok(true) => Redirect::to("/admin/dashboard").into_response(),
        Ok(false) => {
            let nav = nav_context(&identity);
            render(AppDetailTemplate {
                admin_email: nav.admin_email,
                is_instance_admin: nav.is_instance_admin,
                can_delete_applications: nav.can_delete_applications,
                can_manage_operators: nav.can_manage_operators,
                show_directories_nav: nav.show_directories_nav,
                app_id: app.id.to_string(),
                app_name: app.name,
                created_at: app.created_at,
                user_count: app.user_count,
                flash: None,
                error: Some("Application could not be deleted.".to_string()),
            })
            .into_response()
        }
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
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    render_app_users_page(
        &state,
        &identity,
        &app,
        users,
        None,
        None,
    )
    .await
}

async fn render_app_users_page(
    state: &AppState,
    identity: &AdminSession,
    app: &admin_ops::ApplicationSummary,
    users: Vec<admin_ops::TenantUserRow>,
    flash: Option<String>,
    new_invite_url: Option<String>,
) -> Response {
    let pending = match invites::list_pending_invites_for_app(&state.db, app.id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(AppUsersTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: identity.is_instance_admin(),
        app_id: app.id.to_string(),
        app_name: app.name.clone(),
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
        pending_invites: pending
            .into_iter()
            .map(|inv| AppInviteDisplay {
                email: inv.email,
                role: inv.role,
                invite_url: invites::invite_url(&state.config.public_base_url, &inv.token),
                expires_at: inv.expires_at,
                org_id: inv.organization_id.to_string(),
                org_name: inv.organization_name,
            })
            .collect(),
        new_invite_url,
        flash,
    })
    .into_response()
}

fn team_org_select_rows(
    orgs: &[admin_ops::OrgSummary],
    selected_org_id: Option<&str>,
) -> Vec<OrgSelectRow> {
    let default_org = selected_org_id
        .filter(|id| orgs.iter().any(|o| o.id == *id))
        .map(str::to_string)
        .or_else(|| orgs.first().map(|o| o.id.clone()));

    orgs.iter()
        .map(|o| OrgSelectRow {
            org_id: o.id.clone(),
            label: format!("{} ({})", o.name, o.slug),
            selected: default_org.as_deref() == Some(o.id.as_str()),
        })
        .collect()
}

async fn invite_app_user_page(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    render(InviteAppUserTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: identity.is_instance_admin(),
        app_id: app.id.to_string(),
        app_name: app.name,
        organizations: team_org_select_rows(&orgs, None),
        error: None,
        name: None,
        email: None,
        org_id: None,
        role: Some("member".to_string()),
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateAppUserInviteForm {
    email: String,
    name: Option<String>,
    org_id: String,
    role: Option<String>,
}

async fn create_app_user_invite(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
    Form(body): Form<CreateAppUserInviteForm>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    let org_id_str = body.org_id.trim();
    let organization_id: OrganizationId = match org_id_str.parse() {
        Ok(id) => id,
        Err(_) => {
            return render(InviteAppUserTemplate {
                admin_email: identity.email.clone(),
                is_instance_admin: identity.is_instance_admin(),
                app_id: app.id.to_string(),
                app_name: app.name.clone(),
                organizations: team_org_select_rows(&orgs, Some(org_id_str)),
                error: Some("Select a valid organization.".to_string()),
                name: body.name,
                email: Some(body.email),
                org_id: Some(org_id_str.to_string()),
                role: body.role,
            })
            .into_response();
        }
    };

    let invite = match invites::create_invite(
        &state.db,
        invites::CreateInviteInput {
            application_id: app_id,
            organization_id,
            email: &body.email,
            org_role_id: None,
            role_slug: body.role.as_deref(),
            name: body.name.as_deref(),
            expiry_secs: state.config.invite_expiry_secs,
        },
    )
    .await
    {
        Ok(inv) => inv,
        Err(e) => {
            return render(InviteAppUserTemplate {
                admin_email: identity.email.clone(),
                is_instance_admin: identity.is_instance_admin(),
                app_id: app.id.to_string(),
                app_name: app.name.clone(),
                organizations: team_org_select_rows(&orgs, Some(org_id_str)),
                error: Some(e.to_string()),
                name: body.name,
                email: Some(body.email),
                org_id: Some(org_id_str.to_string()),
                role: body.role,
            })
            .into_response();
        }
    };

    let users = match admin_ops::list_tenant_users(&state.db, app_id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let invite_url = invites::invite_url(&state.config.public_base_url, &invite.token);

    render_app_users_page(
        &state,
        &identity,
        &app,
        users,
        Some(format!(
            "Invite created for {}. Share this link (email is not sent automatically):",
            invite.email
        )),
        Some(invite_url),
    )
    .await
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
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    match identity::load_app_context(&state.db, app_id).await {
        Ok(Some(ctx)) => {
            match auth_service::signup(&state.db, &ctx, &name, &email, &body.password).await {
                Ok(_) => Redirect::to(&format!("/admin/apps/{}/users", app.id)).into_response(),
                Err(AppError::Conflict(_)) => render(NewAppUserTemplate {
                    admin_email: identity.email.clone(),
                    is_instance_admin: identity.is_instance_admin(),
                    app_id: app.id.to_string(),
                    app_name: app.name,
                    error: Some(
                        "A user with that email already exists in this directory.".to_string(),
                    ),
                    name: Some(name),
                    email: Some(email),
                })
                .into_response(),
                Err(e) => AppError::Internal(anyhow::anyhow!(e.to_string())).into_response(),
            }
        }
        Ok(None) | Err(_) => AppError::Internal(anyhow::anyhow!("application not found")).into_response(),
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
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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
                member_count: o.member_count,
                created_at: o.created_at,
            })
            .collect(),
        flash: None,
    })
    .into_response()
}

async fn new_team_org_page(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(NewTeamOrgTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: identity.is_instance_admin(),
        app_id: app.id.to_string(),
        app_name: app.name,
        error: None,
        name: None,
        slug: None,
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateTeamOrgForm {
    name: String,
    slug: String,
}

async fn create_team_org(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
    Form(body): Form<CreateTeamOrgForm>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let name = body.name.trim().to_string();
    let slug = body.slug.trim().to_string();

    match admin_ops::create_team_org(&state.db, app_id, &name, &slug).await {
        Ok(org) => Redirect::to(&format!("/admin/apps/{}/orgs/{}", app.id, org.id)).into_response(),
        Err(e) if e.to_string().contains("23505") => render(NewTeamOrgTemplate {
            admin_email: identity.email.clone(),
            is_instance_admin: identity.is_instance_admin(),
            app_id: app.id.to_string(),
            app_name: app.name,
            error: Some("An organization with that slug already exists in this application.".to_string()),
            name: Some(name),
            slug: Some(slug),
        })
        .into_response(),
        Err(e) => render(NewTeamOrgTemplate {
            admin_email: identity.email.clone(),
            is_instance_admin: identity.is_instance_admin(),
            app_id: app.id.to_string(),
            app_name: app.name,
            error: Some(e.to_string()),
            name: Some(name),
            slug: Some(slug),
        })
        .into_response(),
    }
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
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    build_org_detail_page(
        &state,
        &identity,
        &app,
        &org,
        members,
        None,
        None,
        None,
    )
    .await
}

async fn build_org_detail_page(
    state: &AppState,
    identity: &AdminSession,
    app: &admin_ops::ApplicationSummary,
    org: &admin_ops::OrgDetail,
    members: Vec<admin_ops::OrgMemberRow>,
    error: Option<String>,
    flash: Option<String>,
    new_invite_url: Option<String>,
) -> Response {
    let org_id: OrganizationId = match org.id.parse() {
        Ok(id) => id,
        Err(e) => return AppError::Internal(e.into()).into_response(),
    };

    let pending = match invites::list_pending_invites(&state.db, app.id, org_id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let app_permissions: Vec<AppPermissionDisplay> = match roles::list_app_permissions(&state.db, app.id).await {
        Ok(rows) => rows
            .into_iter()
            .map(|p| AppPermissionDisplay {
                id: p.id.to_string(),
                key: p.key,
                name: p.name,
                description: p.description,
            })
            .collect(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let org_roles = match load_org_role_displays(&state.db, org_id, &app_permissions).await {
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
        pending_invites: {
            let mut out = Vec::new();
            for inv in pending {
                let role = invites::invite_role_slug(&state.db, inv.org_role_id)
                    .await
                    .unwrap_or_else(|_| "member".to_string());
                out.push(InviteDisplay {
                    email: inv.email,
                    role,
                    invite_url: invites::invite_url(&state.config.public_base_url, &inv.token),
                    expires_at: inv.expires_at,
                });
            }
            out
        },
        org_roles,
        app_permissions,
        new_invite_url,
        error,
        flash,
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateOrgInviteForm {
    email: String,
    role: Option<String>,
    name: Option<String>,
}

async fn create_org_invite(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path((app_id, org_id)): Path<(String, String)>,
    Form(body): Form<CreateOrgInviteForm>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    let organization_id: OrganizationId = match org_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return render_org_detail_error(
                &state,
                &identity,
                &app,
                &org,
                "Invalid organization id.",
            )
            .await;
        }
    };

    let invite = match invites::create_invite(
        &state.db,
        invites::CreateInviteInput {
            application_id: app_id,
            organization_id,
            email: &body.email,
            org_role_id: None,
            role_slug: body.role.as_deref(),
            name: body.name.as_deref(),
            expiry_secs: state.config.invite_expiry_secs,
        },
    )
    .await
    {
        Ok(inv) => inv,
        Err(e) => {
            return render_org_detail_error(&state, &identity, &app, &org, &e.to_string()).await;
        }
    };

    let invite_url = invites::invite_url(&state.config.public_base_url, &invite.token);
    let members = match admin_ops::list_org_members(&state.db, &org_id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    build_org_detail_page(
        &state,
        &identity,
        &app,
        &org,
        members,
        None,
        Some(format!(
            "Invite created for {}. Share this link (email is not sent automatically):",
            invite.email
        )),
        Some(invite_url),
    )
    .await
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
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    match admin_ops::add_org_member(
        &state.db,
        app_id,
        &org_id,
        &user_id,
        None,
        body.role.as_deref(),
    )
    .await
    {
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
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    build_org_detail_page(
        state,
        identity,
        app,
        org,
        members,
        Some(error.to_string()),
        None,
        None,
    )
    .await
}

async fn directories_page(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
) -> Response {
    if !admin_access::can_view_directories(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let directories = match admin_ops::list_directories_for_session(&state.db, &identity).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let nav = nav_context(&identity);
    render(DirectoriesTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_manage_directories: nav.can_manage_directories,
        can_manage_operators: nav.can_manage_operators,
        directories: directories
            .into_iter()
            .map(|d| DirectoryRow {
                directory_id: d.id.to_string(),
                name: d.name,
                slug: d.slug,
                application_count: d.application_count,
                admin_count: d.admin_count,
                created_at: d.created_at,
            })
            .collect(),
        flash: None,
        error: None,
    })
    .into_response()
}

async fn directory_detail(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(directory_id): Path<String>,
) -> Response {
    let directory_id = match parse_directory_id(&directory_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if !admin_access::can_view_directories(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    match admin_access::can_access_directory(&state.db, &identity, directory_id).await {
        Ok(true) => {}
        Ok(false) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    }

    let directory = match admin_ops::get_directory(&state.db, directory_id).await {
        Ok(Some(d)) => d,
        Ok(None) => return Redirect::to("/admin/directories").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let admins = match admin_ops::list_directory_admins(&state.db, directory_id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let nav = nav_context(&identity);
    render(DirectoryDetailTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_manage_operators: nav.can_manage_operators,
        can_manage_directories: nav.can_manage_directories,
        directory_id: directory.id.to_string(),
        directory_name: directory.name,
        directory_slug: directory.slug,
        application_count: directory.application_count,
        created_at: directory.created_at,
        admins: admins
            .into_iter()
            .map(|a| DirectoryAdminDisplay {
                email: a.email,
                created_at: a.created_at,
            })
            .collect(),
        flash: None,
        error: None,
    })
    .into_response()
}

async fn new_directory_admin_page(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(directory_id): Path<String>,
) -> Response {
    let directory_id = match parse_directory_id(&directory_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if !admin_access::can_manage_operators(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    match admin_access::can_access_directory(&state.db, &identity, directory_id).await {
        Ok(true) => {}
        Ok(false) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    }

    let directory = match admin_ops::get_directory(&state.db, directory_id).await {
        Ok(Some(d)) => d,
        Ok(None) => return Redirect::to("/admin/directories").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let nav = nav_context(&identity);
    render(NewDirectoryAdminTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_manage_operators: nav.can_manage_operators,
        directory_id: directory.id.to_string(),
        directory_name: directory.name,
        error: None,
        email: None,
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateDirectoryAdminForm {
    email: String,
    password: String,
}

async fn create_directory_admin(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(directory_id): Path<String>,
    Form(body): Form<CreateDirectoryAdminForm>,
) -> Response {
    let directory_id = match parse_directory_id(&directory_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };

    if !admin_access::can_manage_operators(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    match admin_access::can_access_directory(&state.db, &identity, directory_id).await {
        Ok(true) => {}
        Ok(false) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    }

    let directory = match admin_ops::get_directory(&state.db, directory_id).await {
        Ok(Some(d)) => d,
        Ok(None) => return Redirect::to("/admin/directories").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    match admin_ops::create_operator(
        &state.db,
        &identity,
        &body.email,
        &body.password,
        AdminRole::DirectoryAdmin,
        &[],
        &[directory_id],
    )
    .await
    {
        Ok(_) => Redirect::to(&format!(
            "/admin/directories/{}",
            directory.id
        ))
        .into_response(),
        Err(e) if e.to_string().contains("unique") || e.to_string().contains("23505") => {
            let nav = nav_context(&identity);
            render(NewDirectoryAdminTemplate {
                admin_email: nav.admin_email,
                is_instance_admin: nav.is_instance_admin,
                can_manage_operators: nav.can_manage_operators,
                directory_id: directory.id.to_string(),
                directory_name: directory.name,
                error: Some("An operator with that email already exists.".to_string()),
                email: Some(body.email),
            })
            .into_response()
        }
        Err(e) => {
            let nav = nav_context(&identity);
            render(NewDirectoryAdminTemplate {
                admin_email: nav.admin_email,
                is_instance_admin: nav.is_instance_admin,
                can_manage_operators: nav.can_manage_operators,
                directory_id: directory.id.to_string(),
                directory_name: directory.name,
                error: Some(e.to_string()),
                email: Some(body.email),
            })
            .into_response()
        }
    }
}

async fn new_directory_page(Extension(identity): Extension<AdminSession>) -> Response {
    if !admin_access::can_manage_directories(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    render(NewDirectoryTemplate {
        admin_email: identity.email.clone(),
        is_instance_admin: true,
        error: None,
        name: None,
        slug: None,
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateDirectoryForm {
    name: String,
    slug: String,
}

async fn create_directory(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Form(body): Form<CreateDirectoryForm>,
) -> Response {
    if !admin_access::can_manage_directories(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let name = body.name.trim().to_string();
    let slug = body.slug.trim().to_string();

    match admin_ops::create_directory(&state.db, &name, &slug).await {
        Ok(_) => Redirect::to("/admin/directories").into_response(),
        Err(e) if e.to_string().contains("23505") => render(NewDirectoryTemplate {
            admin_email: identity.email.clone(),
            is_instance_admin: true,
            error: Some("A directory with that slug already exists.".to_string()),
            name: Some(name),
            slug: Some(slug),
        })
        .into_response(),
        Err(e) => render(NewDirectoryTemplate {
            admin_email: identity.email.clone(),
            is_instance_admin: true,
            error: Some(e.to_string()),
            name: Some(name),
            slug: Some(slug),
        })
        .into_response(),
    }
}

async fn operators_page(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
) -> Response {
    if !admin_access::can_manage_operators(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let operators = match admin_ops::list_operators_for_session(&state.db, &identity).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let nav = nav_context(&identity);
    render(OperatorsTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_manage_operators: nav.can_manage_operators,
        show_directories_nav: nav.show_directories_nav,
        operators: operators
            .into_iter()
            .map(|op| {
                let scope_label = operator_scope_label(&op);
                OperatorRow {
                    id: op.id.to_string(),
                    email: op.email,
                    role_label: role_label(op.role),
                    scope_label,
                    created_at: op.created_at,
                }
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
    if !admin_access::can_manage_operators(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let apps = match admin_ops::list_applications_for_picker(&state.db, &identity).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };
    let directories = match admin_ops::list_directories_for_picker(&state.db, &identity).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(new_operator_template(
        &identity,
        apps
            .into_iter()
            .map(|(id, name)| AppPickerRow {
                app_id: id.to_string(),
                name,
                selected: false,
            })
            .collect(),
        directories
            .into_iter()
            .map(|(id, name)| DirectorySelectRow {
                directory_id: id.to_string(),
                label: name,
                selected: false,
            })
            .collect(),
        None,
        None,
        Some("app_admin".to_string()),
    ))
    .into_response()
}

#[derive(Deserialize)]
struct CreateOperatorForm {
    email: String,
    password: String,
    role: String,
    #[serde(default, deserialize_with = "deserialize_form_string_vec")]
    app_ids: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_form_string_vec")]
    directory_ids: Vec<String>,
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
    if !admin_access::can_manage_operators(&identity) {
        return Redirect::to("/admin/dashboard").into_response();
    }

    let apps = match admin_ops::list_applications_for_picker(&state.db, &identity).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };
    let directories = match admin_ops::list_directories_for_picker(&state.db, &identity).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    let app_picker: Vec<AppPickerRow> = apps
        .iter()
        .map(|(id, name)| AppPickerRow {
            app_id: id.to_string(),
            name: name.clone(),
            selected: body.app_ids.iter().any(|s| s == &id.to_string()),
        })
        .collect();
    let directory_picker: Vec<DirectorySelectRow> = directories
        .iter()
        .map(|(id, name)| DirectorySelectRow {
            directory_id: id.to_string(),
            label: name.clone(),
            selected: body.directory_ids.iter().any(|s| s == &id.to_string()),
        })
        .collect();

    let role = match body.role.parse::<AdminRole>() {
        Ok(r) => r,
        Err(_) => {
            return render(new_operator_template(
                &identity,
                app_picker,
                directory_picker,
                Some("Invalid operator role.".to_string()),
                Some(body.email),
                Some(body.role),
            ))
            .into_response();
        }
    };

    let mut app_ids = Vec::new();
    for raw in &body.app_ids {
        match raw.parse::<ApplicationId>() {
            Ok(id) => app_ids.push(id),
            Err(_) => {
                return render(new_operator_template(
                    &identity,
                    app_picker,
                    directory_picker,
                    Some(format!("Invalid application id: {raw}")),
                    Some(body.email),
                    Some(body.role),
                ))
                .into_response();
            }
        }
    }

    let mut directory_ids = Vec::new();
    for raw in &body.directory_ids {
        match raw.parse::<DirectoryId>() {
            Ok(id) => directory_ids.push(id),
            Err(_) => {
                return render(new_operator_template(
                    &identity,
                    app_picker,
                    directory_picker,
                    Some(format!("Invalid directory id: {raw}")),
                    Some(body.email),
                    Some(body.role),
                ))
                .into_response();
            }
        }
    }

    match admin_ops::create_operator(
        &state.db,
        &identity,
        &body.email,
        &body.password,
        role,
        &app_ids,
        &directory_ids,
    )
    .await
    {
        Ok(_) => Redirect::to("/admin/operators").into_response(),
        Err(e) if e.to_string().contains("unique") || e.to_string().contains("23505") => {
            render(new_operator_template(
                &identity,
                app_picker,
                directory_picker,
                Some("An operator with that email already exists.".to_string()),
                Some(body.email),
                Some(body.role),
            ))
            .into_response()
        }
        Err(e) => render(new_operator_template(
            &identity,
            app_picker,
            directory_picker,
            Some(e.to_string()),
            Some(body.email),
            Some(body.role),
        ))
        .into_response(),
    }
}

#[derive(Deserialize)]
struct CreateApplicationJsonRequest {
    name: String,
    directory_id: Option<DirectoryId>,
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
    if !admin_access::can_create_applications(&identity) {
        return Err(AppError::Forbidden);
    }

    let name = body.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::Validation("name is required".to_string()));
    }

    if let Some(dir_id) = body.directory_id {
        if !admin_access::can_access_directory(&state.db, &identity, dir_id)
            .await
            .map_err(AppError::Internal)?
        {
            return Err(AppError::Forbidden);
        }
    }

    let client_secret = format!(
        "secret_{}",
        &Uuid::new_v4().to_string().replace('-', "")[..32]
    );
    let secret_hash = password::hash(&client_secret).map_err(AppError::Internal)?;

    let id = admin_ops::create_application(&state.db, &name, &secret_hash, body.directory_id)
        .await
        .map_err(AppError::Internal)?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(CreateApplicationJsonResponse {
            id,
            client_secret,
            name,
        }),
    ))
}

async fn load_org_role_displays(
    db: &sqlx::PgPool,
    organization_id: OrganizationId,
    app_permissions: &[AppPermissionDisplay],
) -> std::result::Result<Vec<OrgRoleDisplay>, anyhow::Error> {
    let org_roles = roles::list_org_roles(db, organization_id).await?;
    let mut out = Vec::with_capacity(org_roles.len());
    for role in org_roles {
        let permission_ids = roles::list_org_role_permission_ids(db, role.id).await?;
        let permission_assignments = app_permissions
            .iter()
            .map(|perm| PermissionAssignmentDisplay {
                id: perm.id.clone(),
                key: perm.key.clone(),
                name: perm.name.clone(),
                checked: permission_ids.iter().any(|id| id.to_string() == perm.id),
            })
            .collect();
        let member_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*)::bigint FROM member WHERE org_role_id = $1",
        )
        .bind(role.id)
        .fetch_one(db)
        .await?;
        out.push(OrgRoleDisplay {
            id: role.id.to_string(),
            slug: role.slug,
            name: role.name,
            description: role.description,
            permission_assignments,
            member_count,
        });
    }
    Ok(out)
}

async fn app_permissions_page(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render_app_permissions_page(&state, &identity, &app, None, None).await
}

async fn render_app_permissions_page(
    state: &AppState,
    identity: &AdminSession,
    app: &admin_ops::ApplicationSummary,
    flash: Option<String>,
    error: Option<String>,
) -> Response {
    let nav = nav_context(identity);
    let permissions = match roles::list_app_permissions(&state.db, app.id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };

    render(AppPermissionsTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_manage_operators: nav.can_manage_operators,
        show_directories_nav: nav.show_directories_nav,
        app_id: app.id.to_string(),
        app_name: app.name.clone(),
        permissions: permissions
            .into_iter()
            .map(|p| AppPermissionDisplay {
                id: p.id.to_string(),
                key: p.key,
                name: p.name,
                description: p.description,
            })
            .collect(),
        flash,
        error,
    })
    .into_response()
}

#[derive(Deserialize)]
struct CreateAppPermissionForm {
    key: String,
    name: String,
    description: Option<String>,
}

async fn create_app_permission_admin(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
    Form(body): Form<CreateAppPermissionForm>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
    }

    let app = match admin_ops::get_application_summary(&state.db, app_id).await {
        Ok(Some(a)) => a,
        Ok(None) => return Redirect::to("/admin/dashboard").into_response(),
        Err(e) => return AppError::Internal(e).into_response(),
    };

    match roles::create_app_permission(
        &state.db,
        app_id,
        &body.key,
        &body.name,
        body.description.as_deref(),
    )
    .await
    {
        Ok(_) => {
            render_app_permissions_page(
                &state,
                &identity,
                &app,
                Some(format!("Permission `{}` created.", body.key.trim())),
                None,
            )
            .await
        }
        Err(e) => {
            render_app_permissions_page(&state, &identity, &app, None, Some(e.to_string())).await
        }
    }
}

async fn delete_app_permission_admin(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path((app_id, perm_id)): Path<(String, String)>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
    }

    let perm_id: crate::ids::AppPermissionId = match perm_id.parse() {
        Ok(id) => id,
        Err(_) => return Redirect::to(&format!("/admin/apps/{app_id}/permissions")).into_response(),
    };

    let _ = roles::delete_app_permission(&state.db, app_id, perm_id).await;
    Redirect::to(&format!("/admin/apps/{app_id}/permissions")).into_response()
}

#[derive(Deserialize)]
struct CreateOrgRoleForm {
    slug: String,
    name: String,
    description: Option<String>,
}

async fn create_org_role_admin(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path((app_id, org_id)): Path<(String, String)>,
    Form(body): Form<CreateOrgRoleForm>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    let organization_id: OrganizationId = match org_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return render_org_detail_error(&state, &identity, &app, &org, "Invalid organization id.")
                .await;
        }
    };

    match roles::create_org_role(
        &state.db,
        organization_id,
        &body.slug,
        &body.name,
        body.description.as_deref(),
        &[],
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/admin/apps/{}/orgs/{}", app.id, org_id)).into_response(),
        Err(e) => render_org_detail_error(&state, &identity, &app, &org, &e.to_string()).await,
    }
}

#[derive(Deserialize)]
struct UpdateOrgRoleForm {
    name: String,
    description: Option<String>,
    #[serde(default, deserialize_with = "deserialize_form_string_vec")]
    permission_ids: Vec<String>,
}

async fn update_org_role_admin(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path((app_id, org_id, role_id)): Path<(String, String, String)>,
    Form(body): Form<UpdateOrgRoleForm>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    let organization_id: OrganizationId = match org_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return render_org_detail_error(&state, &identity, &app, &org, "Invalid organization id.")
                .await;
        }
    };

    let role_id: crate::ids::OrgRoleId = match role_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return render_org_detail_error(&state, &identity, &app, &org, "Invalid role id.").await;
        }
    };

    let permission_ids: Vec<crate::ids::AppPermissionId> = body
        .permission_ids
        .iter()
        .filter_map(|s| s.parse().ok())
        .collect();

    match roles::update_org_role(
        &state.db,
        organization_id,
        role_id,
        Some(&body.name),
        Some(body.description.as_deref()),
        Some(&permission_ids),
    )
    .await
    {
        Ok(_) => Redirect::to(&format!("/admin/apps/{}/orgs/{}", app.id, org_id)).into_response(),
        Err(e) => render_org_detail_error(&state, &identity, &app, &org, &e.to_string()).await,
    }
}

async fn delete_org_role_admin(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path((app_id, org_id, role_id)): Path<(String, String, String)>,
) -> Response {
    let app_id = match parse_app_id(&app_id) {
        Ok(id) => id,
        Err(resp) => return resp,
    };
    if let Err(resp) = require_app_access(&state.db, &identity, app_id).await {
        return resp;
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

    let organization_id: OrganizationId = match org_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return render_org_detail_error(&state, &identity, &app, &org, "Invalid organization id.")
                .await;
        }
    };

    let role_id: crate::ids::OrgRoleId = match role_id.parse() {
        Ok(id) => id,
        Err(_) => {
            return render_org_detail_error(&state, &identity, &app, &org, "Invalid role id.").await;
        }
    };

    match roles::delete_org_role(&state.db, organization_id, role_id).await {
        Ok(_) => Redirect::to(&format!("/admin/apps/{}/orgs/{}", app.id, org_id)).into_response(),
        Err(e) => render_org_detail_error(&state, &identity, &app, &org, &e.to_string()).await,
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn nav_context(identity: &AdminSession) -> AdminNav {
    AdminNav {
        admin_email: identity.email.clone(),
        is_instance_admin: identity.is_instance_admin(),
        can_manage_operators: admin_access::can_manage_operators(identity),
        can_create_applications: admin_access::can_create_applications(identity),
        can_delete_applications: admin_access::can_delete_applications(identity),
        can_manage_directories: admin_access::can_manage_directories(identity),
        show_directories_nav: admin_access::can_view_directories(identity),
    }
}

struct AdminNav {
    admin_email: String,
    is_instance_admin: bool,
    can_manage_operators: bool,
    can_create_applications: bool,
    can_delete_applications: bool,
    can_manage_directories: bool,
    show_directories_nav: bool,
}

async fn require_app_access(
    db: &sqlx::PgPool,
    identity: &AdminSession,
    app_id: ApplicationId,
) -> std::result::Result<(), Response> {
    match admin_access::can_access_app(db, identity, app_id).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Redirect::to("/admin/dashboard").into_response()),
        Err(e) => Err(AppError::Internal(e).into_response()),
    }
}

fn operator_scope_label(op: &admin_ops::OperatorSummary) -> String {
    match op.role {
        AdminRole::InstanceAdmin => "All applications".to_string(),
        AdminRole::DirectoryAdmin => {
            if op.granted_directory_ids.is_empty() {
                "No directories".to_string()
            } else {
                op.granted_directory_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        AdminRole::AppAdmin => {
            if op.granted_app_ids.is_empty() {
                "No applications".to_string()
            } else {
                op.granted_app_ids
                    .iter()
                    .map(|id| id.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
    }
}

fn new_operator_template(
    identity: &AdminSession,
    applications: Vec<AppPickerRow>,
    directories: Vec<DirectorySelectRow>,
    error: Option<String>,
    email: Option<String>,
    role: Option<String>,
) -> NewOperatorTemplate {
    let nav = nav_context(identity);
    NewOperatorTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_manage_operators: nav.can_manage_operators,
        show_directories_nav: nav.show_directories_nav,
        show_instance_admin_role: nav.is_instance_admin,
        applications,
        directories,
        error,
        email,
        role,
    }
}

fn role_label(role: AdminRole) -> String {
    match role {
        AdminRole::InstanceAdmin => "Instance admin".to_string(),
        AdminRole::AppAdmin => "App admin".to_string(),
        AdminRole::DirectoryAdmin => "Directory admin".to_string(),
    }
}

fn parse_directory_id_option(
    raw: Option<&str>,
) -> std::result::Result<Option<DirectoryId>, Response> {
    match raw.map(str::trim).filter(|s| !s.is_empty()) {
        None => Ok(None),
        Some(s) => s
            .parse::<DirectoryId>()
            .map(Some)
            .map_err(|_| Redirect::to("/admin/dashboard").into_response()),
    }
}

async fn directory_select_rows(
    db: &sqlx::PgPool,
    session: &AdminSession,
    selected: Option<DirectoryId>,
) -> std::result::Result<Vec<DirectorySelectRow>, anyhow::Error> {
    let picker = admin_ops::list_directories_for_picker(db, session).await?;
    let default_id = if session.is_instance_admin() {
        identity::get_default_directory_id(db).await.ok()
    } else {
        None
    };
    Ok(picker
        .into_iter()
        .map(|(id, name)| {
            let is_selected = selected
                .map(|sel| sel == id)
                .unwrap_or_else(|| default_id.map(|def| def == id).unwrap_or(false));
            DirectorySelectRow {
                directory_id: id.to_string(),
                label: name,
                selected: is_selected,
            }
        })
        .collect())
}

async fn render_new_app_error(
    state: &AppState,
    identity: &AdminSession,
    error: &str,
    name: Option<String>,
    directory_id: Option<DirectoryId>,
) -> Response {
    let directories = match directory_select_rows(&state.db, identity, directory_id).await {
        Ok(rows) => rows,
        Err(e) => return AppError::Internal(e).into_response(),
    };
    let nav = nav_context(identity);
    render(NewAppTemplate {
        admin_email: nav.admin_email,
        is_instance_admin: nav.is_instance_admin,
        can_manage_operators: nav.can_manage_operators,
        show_directories_nav: nav.show_directories_nav,
        directories,
        error: Some(error.to_string()),
        name,
        directory_id: directory_id.map(|id| id.to_string()),
    })
    .into_response()
}

fn parse_directory_id(raw: &str) -> std::result::Result<DirectoryId, Response> {
    raw.parse::<DirectoryId>()
        .map_err(|_| Redirect::to("/admin/directories").into_response())
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
