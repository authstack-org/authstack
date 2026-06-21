#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use utoipa::{
    Modify, OpenApi, ToSchema,
    openapi::{
        Components,
        security::{ApiKey, ApiKeyValue, Http, HttpAuthScheme, HttpBuilder, SecurityScheme},
    },
};

pub fn spec() -> utoipa::openapi::OpenApi {
    ApiDoc::openapi()
}

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Authstack API",
        version = "0.1.0",
        description = "HTTP API exposed by Authstack."
    ),
    servers(
        (url = "http://localhost:8080", description = "Local development")
    ),
    tags(
        (name = "Admin", description = "Admin panel endpoints."),
        (name = "Auth", description = "End-user authentication endpoints."),
        (name = "Me", description = "Current user endpoints."),
        (name = "Users", description = "Application-scoped users."),
        (name = "Organizations", description = "Application-scoped organizations."),
        (name = "Permissions", description = "Application permission catalog."),
        (name = "OrgRoles", description = "Organization role definitions."),
        (name = "Members", description = "Organization membership."),
        (name = "Invites", description = "Organization invite links."),
        (name = "JWKS", description = "JWT verification keys.")
    ),
    paths(
        admin_login_page,
        admin_process_login,
        admin_logout,
        admin_dashboard,
        admin_new_app_page,
        admin_create_app,
        admin_create_application,
        auth_signup,
        auth_login,
        auth_refresh,
        auth_logout,
        auth_switch_org,
        me_organizations,
        users_list,
        users_get,
        orgs_list,
        orgs_create,
        orgs_get,
        permissions_list,
        permissions_create,
        permissions_get,
        permissions_delete,
        org_roles_list,
        org_roles_create,
        org_roles_get,
        org_roles_update,
        org_roles_delete,
        members_list,
        members_add,
        members_remove,
        invites_list,
        invites_create,
        invites_accept,
        jwks
    ),
    components(schemas(
        AddMemberRequest,
        AcceptInviteRequest,
        AcceptInviteResponse,
        AppPermission,
        CreateAppPermissionRequest,
        CreateApplicationRequest,
        CreateApplicationResponse,
        CreateAppForm,
        CreateInviteRequest,
        CreateOrgRequest,
        CreateOrgRoleRequest,
        ErrorResponse,
        InviteResponse,
        Jwk,
        JwksResponse,
        LoginForm,
        LoginRequest,
        Member,
        OkResponse,
        Organization,
        OrgRole,
        OrgRoleDetail,
        RefreshRequest,
        SignupRequest,
        SignupResponse,
        SwitchOrgRequest,
        TokenResponse,
        UpdateOrgRoleRequest,
        User,
        UserOrganization
    )),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_with(Components::new);

        components.add_security_scheme(
            "appBasicAuth",
            SecurityScheme::Http(Http::new(HttpAuthScheme::Basic)),
        );
        components.add_security_scheme(
            "bearerAuth",
            SecurityScheme::Http(
                HttpBuilder::new()
                    .scheme(HttpAuthScheme::Bearer)
                    .bearer_format("JWT")
                    .build(),
            ),
        );
        components.add_security_scheme(
            "adminCookie",
            SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::new("admin_token"))),
        );
    }
}

#[derive(Debug, Deserialize, ToSchema)]
struct LoginForm {
    #[schema(format = Email)]
    email: String,
    password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
struct CreateAppForm {
    name: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct OkResponse {
    ok: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
struct CreateApplicationRequest {
    name: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct CreateApplicationResponse {
    id: String,
    client_secret: String,
    name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
struct SignupRequest {
    name: String,
    #[schema(format = Email)]
    email: String,
    password: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct SignupResponse {
    id: String,
    #[schema(format = Email)]
    email: String,
    name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
struct LoginRequest {
    #[schema(format = Email)]
    email: String,
    password: String,
}

#[derive(Debug, Deserialize, ToSchema)]
struct RefreshRequest {
    refresh_token: String,
    org_id: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
struct SwitchOrgRequest {
    org_id: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    #[schema(example = "Bearer")]
    token_type: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct User {
    id: String,
    directory_id: String,
    name: String,
    #[schema(format = Email)]
    email: String,
    email_verified: bool,
    image: Option<String>,
    #[schema(format = DateTime)]
    created_at: String,
    #[schema(format = DateTime)]
    updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct Organization {
    id: String,
    directory_id: String,
    application_id: String,
    name: String,
    slug: String,
    logo: Option<String>,
    #[schema(format = DateTime)]
    created_at: String,
    #[schema(format = DateTime)]
    updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
struct CreateOrgRequest {
    name: String,
    slug: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct AppPermission {
    id: String,
    application_id: String,
    key: String,
    name: String,
    description: Option<String>,
    #[schema(format = DateTime)]
    created_at: String,
    #[schema(format = DateTime)]
    updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
struct CreateAppPermissionRequest {
    key: String,
    name: String,
    description: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
struct OrgRole {
    id: String,
    organization_id: String,
    slug: String,
    name: String,
    description: Option<String>,
    #[schema(format = DateTime)]
    created_at: String,
    #[schema(format = DateTime)]
    updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct OrgRoleDetail {
    #[serde(flatten)]
    role: OrgRole,
    permission_ids: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
struct CreateOrgRoleRequest {
    slug: String,
    name: String,
    description: Option<String>,
    #[serde(default)]
    permission_ids: Vec<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
struct UpdateOrgRoleRequest {
    name: Option<String>,
    description: Option<String>,
    permission_ids: Option<Vec<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
struct Member {
    id: String,
    organization_id: String,
    user_id: String,
    org_role_id: String,
    role: String,
    #[schema(format = DateTime)]
    created_at: String,
    #[schema(format = DateTime)]
    updated_at: String,
}

#[derive(Debug, Deserialize, ToSchema)]
struct AddMemberRequest {
    user_id: String,
    org_role_id: Option<String>,
    role: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
struct UserOrganization {
    organization: Organization,
    role: String,
    permissions: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
struct JwksResponse {
    keys: Vec<Jwk>,
}

#[derive(Debug, Serialize, ToSchema)]
struct Jwk {
    kty: String,
    #[serde(rename = "use")]
    key_use: String,
    kid: String,
    alg: String,
    crv: String,
    x: String,
    y: String,
}

#[utoipa::path(
    get,
    path = "/admin/login",
    tag = "Admin",
    summary = "Render admin login page",
    responses((status = 200, description = "HTML response", body = String, content_type = "text/html"))
)]
fn admin_login_page() {}

#[utoipa::path(
    post,
    path = "/admin/login",
    tag = "Admin",
    summary = "Submit admin login form",
    request_body(content = LoginForm, content_type = "application/x-www-form-urlencoded"),
    responses((status = 303, description = "Redirect"))
)]
fn admin_process_login() {}

#[utoipa::path(
    post,
    path = "/admin/logout",
    tag = "Admin",
    summary = "Clear the admin session cookie",
    responses((status = 303, description = "Redirect"))
)]
fn admin_logout() {}

#[utoipa::path(
    get,
    path = "/admin/dashboard",
    tag = "Admin",
    summary = "Render admin dashboard",
    security(("adminCookie" = [])),
    responses((status = 200, description = "HTML response", body = String, content_type = "text/html"))
)]
fn admin_dashboard() {}

#[utoipa::path(
    get,
    path = "/admin/apps/new",
    tag = "Admin",
    summary = "Render new application form",
    security(("adminCookie" = [])),
    responses((status = 200, description = "HTML response", body = String, content_type = "text/html"))
)]
fn admin_new_app_page() {}

#[utoipa::path(
    post,
    path = "/admin/apps",
    tag = "Admin",
    summary = "Create an application from the admin form",
    security(("adminCookie" = [])),
    request_body(content = CreateAppForm, content_type = "application/x-www-form-urlencoded"),
    responses((status = 200, description = "HTML response", body = String, content_type = "text/html"))
)]
fn admin_create_app() {}

#[utoipa::path(
    post,
    path = "/admin/applications",
    tag = "Admin",
    summary = "Create an application",
    security(("adminCookie" = [])),
    request_body = CreateApplicationRequest,
    responses(
        (status = 201, description = "Application created", body = CreateApplicationResponse),
        (status = 400, description = "Validation error", body = ErrorResponse),
        (status = 401, description = "Unauthorized", body = ErrorResponse),
        (status = 500, description = "Internal error", body = ErrorResponse)
    )
)]
fn admin_create_application() {}

#[utoipa::path(
    post,
    path = "/auth/signup",
    tag = "Auth",
    summary = "Create a user",
    security(("appBasicAuth" = [])),
    request_body = SignupRequest,
    responses((status = 200, description = "User created", body = SignupResponse))
)]
fn auth_signup() {}

#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "Auth",
    summary = "Authenticate a user and return access and refresh tokens",
    security(("appBasicAuth" = [])),
    request_body = LoginRequest,
    responses((status = 200, description = "Authenticated", body = TokenResponse))
)]
fn auth_login() {}

#[utoipa::path(
    post,
    path = "/auth/refresh",
    tag = "Auth",
    summary = "Rotate a refresh token and return a new token pair",
    security(("appBasicAuth" = [])),
    request_body = RefreshRequest,
    responses((status = 200, description = "Token pair rotated", body = TokenResponse))
)]
fn auth_refresh() {}

#[utoipa::path(
    post,
    path = "/auth/logout",
    tag = "Auth",
    summary = "Revoke a refresh token",
    security(("appBasicAuth" = [])),
    request_body = RefreshRequest,
    responses((status = 200, description = "Refresh token revoked", body = OkResponse))
)]
fn auth_logout() {}

#[utoipa::path(
    post,
    path = "/auth/switch-org",
    tag = "Auth",
    summary = "Issue a token pair for another organization",
    security(("bearerAuth" = [])),
    request_body = SwitchOrgRequest,
    responses((status = 200, description = "Organization switched", body = TokenResponse))
)]
fn auth_switch_org() {}

#[utoipa::path(
    get,
    path = "/me/organizations",
    tag = "Me",
    summary = "List organizations the current user belongs to",
    security(("bearerAuth" = [])),
    responses((status = 200, description = "Organizations listed", body = Vec<UserOrganization>))
)]
fn me_organizations() {}

#[utoipa::path(
    get,
    path = "/users",
    tag = "Users",
    summary = "List all users in this application",
    security(("appBasicAuth" = [])),
    responses((status = 200, description = "Users listed", body = Vec<User>))
)]
fn users_list() {}

#[utoipa::path(
    get,
    path = "/users/{id}",
    tag = "Users",
    summary = "Get a user by ID",
    security(("appBasicAuth" = [])),
    params(("id" = String, Path, description = "User ID")),
    responses((status = 200, description = "User found", body = User))
)]
fn users_get() {}

#[utoipa::path(
    get,
    path = "/orgs",
    tag = "Organizations",
    summary = "List all organizations in this application",
    security(("appBasicAuth" = [])),
    responses((status = 200, description = "Organizations listed", body = Vec<Organization>))
)]
fn orgs_list() {}

#[utoipa::path(
    post,
    path = "/orgs",
    tag = "Organizations",
    summary = "Create a team organization",
    security(("appBasicAuth" = [])),
    request_body = CreateOrgRequest,
    responses((status = 200, description = "Organization created", body = Organization))
)]
fn orgs_create() {}

#[utoipa::path(
    get,
    path = "/orgs/{id}",
    tag = "Organizations",
    summary = "Get an organization by ID",
    security(("appBasicAuth" = [])),
    params(("id" = String, Path, description = "Organization ID")),
    responses((status = 200, description = "Organization found", body = Organization))
)]
fn orgs_get() {}

#[utoipa::path(
    get,
    path = "/permissions",
    tag = "Permissions",
    summary = "List application permissions",
    security(("appBasicAuth" = [])),
    responses((status = 200, description = "Permissions listed", body = Vec<AppPermission>))
)]
fn permissions_list() {}

#[utoipa::path(
    post,
    path = "/permissions",
    tag = "Permissions",
    summary = "Create an application permission",
    security(("appBasicAuth" = [])),
    request_body = CreateAppPermissionRequest,
    responses((status = 200, description = "Permission created", body = AppPermission))
)]
fn permissions_create() {}

#[utoipa::path(
    get,
    path = "/permissions/{id}",
    tag = "Permissions",
    summary = "Get a permission by ID",
    security(("appBasicAuth" = [])),
    params(("id" = String, Path, description = "Permission ID")),
    responses((status = 200, description = "Permission found", body = AppPermission))
)]
fn permissions_get() {}

#[utoipa::path(
    delete,
    path = "/permissions/{id}",
    tag = "Permissions",
    summary = "Delete an application permission",
    security(("appBasicAuth" = [])),
    params(("id" = String, Path, description = "Permission ID")),
    responses((status = 200, description = "Permission deleted", body = OkResponse))
)]
fn permissions_delete() {}

#[utoipa::path(
    get,
    path = "/orgs/{org_id}/roles",
    tag = "OrgRoles",
    summary = "List organization roles",
    security(("appBasicAuth" = [])),
    params(("org_id" = String, Path, description = "Organization ID")),
    responses((status = 200, description = "Roles listed", body = Vec<OrgRoleDetail>))
)]
fn org_roles_list() {}

#[utoipa::path(
    post,
    path = "/orgs/{org_id}/roles",
    tag = "OrgRoles",
    summary = "Create an organization role",
    security(("appBasicAuth" = [])),
    params(("org_id" = String, Path, description = "Organization ID")),
    request_body = CreateOrgRoleRequest,
    responses((status = 200, description = "Role created", body = OrgRoleDetail))
)]
fn org_roles_create() {}

#[utoipa::path(
    get,
    path = "/orgs/{org_id}/roles/{role_id}",
    tag = "OrgRoles",
    summary = "Get an organization role",
    security(("appBasicAuth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID"),
        ("role_id" = String, Path, description = "Organization role ID")
    ),
    responses((status = 200, description = "Role found", body = OrgRoleDetail))
)]
fn org_roles_get() {}

#[utoipa::path(
    patch,
    path = "/orgs/{org_id}/roles/{role_id}",
    tag = "OrgRoles",
    summary = "Update an organization role",
    security(("appBasicAuth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID"),
        ("role_id" = String, Path, description = "Organization role ID")
    ),
    request_body = UpdateOrgRoleRequest,
    responses((status = 200, description = "Role updated", body = OrgRoleDetail))
)]
fn org_roles_update() {}

#[utoipa::path(
    delete,
    path = "/orgs/{org_id}/roles/{role_id}",
    tag = "OrgRoles",
    summary = "Delete an organization role",
    security(("appBasicAuth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID"),
        ("role_id" = String, Path, description = "Organization role ID")
    ),
    responses((status = 200, description = "Role deleted", body = OkResponse))
)]
fn org_roles_delete() {}

#[utoipa::path(
    get,
    path = "/orgs/{org_id}/members",
    tag = "Members",
    summary = "List members of an organization",
    security(("appBasicAuth" = [])),
    params(("org_id" = String, Path, description = "Organization ID")),
    responses((status = 200, description = "Members listed", body = Vec<Member>))
)]
fn members_list() {}

#[utoipa::path(
    post,
    path = "/orgs/{org_id}/members",
    tag = "Members",
    summary = "Add a user to an organization",
    security(("appBasicAuth" = [])),
    params(("org_id" = String, Path, description = "Organization ID")),
    request_body = AddMemberRequest,
    responses((status = 200, description = "Member added", body = Member))
)]
fn members_add() {}

#[utoipa::path(
    delete,
    path = "/orgs/{org_id}/members/{user_id}",
    tag = "Members",
    summary = "Remove a user from an organization",
    security(("appBasicAuth" = [])),
    params(
        ("org_id" = String, Path, description = "Organization ID"),
        ("user_id" = String, Path, description = "User ID")
    ),
    responses((status = 200, description = "Member removed", body = OkResponse))
)]
fn members_remove() {}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
struct CreateInviteRequest {
    #[schema(format = Email)]
    email: String,
    org_role_id: Option<String>,
    role: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
struct InviteResponse {
    id: String,
    token: String,
    invite_url: String,
    #[schema(format = Email)]
    email: String,
    organization_id: String,
    role: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
struct AcceptInviteRequest {
    name: Option<String>,
    password: String,
}

#[derive(Debug, Serialize, ToSchema)]
struct AcceptInviteResponse {
    id: String,
    #[schema(format = Email)]
    email: String,
    name: String,
}

#[utoipa::path(
    get,
    path = "/orgs/{org_id}/invites",
    tag = "Invites",
    summary = "List pending organization invites",
    security(("appBasicAuth" = [])),
    params(("org_id" = String, Path, description = "Organization ID")),
    responses((status = 200, description = "Pending invites", body = Vec<InviteResponse>))
)]
fn invites_list() {}

#[utoipa::path(
    post,
    path = "/orgs/{org_id}/invites",
    tag = "Invites",
    summary = "Create an organization invite link",
    security(("appBasicAuth" = [])),
    params(("org_id" = String, Path, description = "Organization ID")),
    request_body = CreateInviteRequest,
    responses((status = 200, description = "Invite created", body = InviteResponse))
)]
fn invites_create() {}

#[utoipa::path(
    post,
    path = "/invites/{token}/accept",
    tag = "Invites",
    summary = "Accept an invite and set a password",
    params(("token" = String, Path, description = "Invite token from the invite URL")),
    request_body = AcceptInviteRequest,
    responses((status = 200, description = "Invite accepted", body = AcceptInviteResponse))
)]
fn invites_accept() {}

#[utoipa::path(
    get,
    path = "/.well-known/jwks.json",
    tag = "JWKS",
    summary = "Return the public JSON Web Key Set",
    responses((status = 200, description = "JSON Web Key Set", body = JwksResponse))
)]
fn jwks() {}
