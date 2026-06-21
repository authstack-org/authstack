use askama::Template;
use axum::{
    Json, Router,
    extract::{Extension, Form, Path, State},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    error::{AppError, Result},
    ids::OrganizationId,
    middleware::app_auth::AppIdentity,
    models::app_invite::AppInvite,
    services::invites::{self, CreateInviteInput},
};

#[derive(Template)]
#[template(path = "invite/accept.html")]
struct AcceptInviteTemplate {
    token: String,
    email: String,
    organization_name: String,
    app_name: String,
    preset_name: Option<String>,
    error: Option<String>,
    unavailable: bool,
}

#[derive(Template)]
#[template(path = "invite/success.html")]
struct AcceptSuccessTemplate {
    organization_name: String,
    app_name: String,
    email: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateInviteRequest {
    pub email: String,
    pub org_role_id: Option<crate::ids::OrgRoleId>,
    pub role: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InviteResponse {
    pub id: String,
    pub token: String,
    pub invite_url: String,
    pub email: String,
    pub organization_id: String,
    pub role: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AcceptInviteRequest {
    pub name: Option<String>,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AcceptInviteResponse {
    pub id: String,
    pub email: String,
    pub name: String,
}

pub fn public_router() -> Router<AppState> {
    Router::new()
        .route("/invite/{token}", get(accept_page))
        .route("/invite/{token}/accept", post(accept_invite_form))
        .route("/invites/{token}/accept", post(accept_invite_json))
}

pub fn app_router() -> Router<AppState> {
    Router::new().route(
        "/orgs/{org_id}/invites",
        get(list_invites).post(create_invite),
    )
}

fn to_invite_response(invite: AppInvite, role: &str, public_base_url: &str) -> InviteResponse {
    InviteResponse {
        id: invite.id.to_string(),
        token: invite.token.clone(),
        invite_url: invites::invite_url(public_base_url, &invite.token),
        email: invite.email,
        organization_id: invite.organization_id.to_string(),
        role: role.to_string(),
        expires_at: invite.expires_at,
    }
}

async fn create_invite(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(org_id): Path<String>,
    Json(body): Json<CreateInviteRequest>,
) -> Result<Json<InviteResponse>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;

    let invite = invites::create_invite(
        &state.db,
        invites::CreateInviteInput {
            application_id: app.app_id,
            organization_id: org_id,
            email: &body.email,
            org_role_id: body.org_role_id,
            role_slug: body.role.as_deref(),
            name: body.name.as_deref(),
            expiry_secs: state.config.invite_expiry_secs,
        },
    )
    .await
    .map_err(|e| AppError::Validation(e.to_string()))?;

    let role = invites::invite_role_slug(&state.db, invite.org_role_id)
        .await
        .map_err(AppError::Internal)?;

    Ok(Json(to_invite_response(
        invite,
        &role,
        &state.config.public_base_url,
    )))
}

async fn list_invites(
    State(state): State<AppState>,
    Extension(app): Extension<AppIdentity>,
    Path(org_id): Path<String>,
) -> Result<Json<Vec<InviteResponse>>> {
    let org_id: OrganizationId = org_id
        .parse()
        .map_err(|_| AppError::NotFound("organization not found".to_string()))?;

    let rows = invites::list_pending_invites(&state.db, app.app_id, org_id)
        .await
        .map_err(AppError::Internal)?;

    let mut out = Vec::with_capacity(rows.len());
    for inv in rows {
        let role = invites::invite_role_slug(&state.db, inv.org_role_id)
            .await
            .map_err(AppError::Internal)?;
        out.push(to_invite_response(inv, &role, &state.config.public_base_url));
    }

    Ok(Json(out))
}

async fn accept_page(State(state): State<AppState>, Path(token): Path<String>) -> Response {
    let ctx = match invites::get_invite_by_token(&state.db, &token).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return render(AcceptInviteTemplate {
                token,
                email: String::new(),
                organization_name: "Invite not found".to_string(),
                app_name: String::new(),
                preset_name: None,
                error: Some("This invite link is invalid.".to_string()),
                unavailable: true,
            })
            .into_response();
        }
        Err(_) => {
            return render(AcceptInviteTemplate {
                token,
                email: String::new(),
                organization_name: "Error".to_string(),
                app_name: String::new(),
                preset_name: None,
                error: Some("Please try again later.".to_string()),
                unavailable: true,
            })
            .into_response();
        }
    };

    if let Err(e) = invites::validate_invite_active(&ctx.invite) {
        let message = match e {
            AppError::Conflict(m) | AppError::Validation(m) => m,
            _ => "This invite is no longer valid.".to_string(),
        };
        return render(AcceptInviteTemplate {
            token,
            email: ctx.invite.email,
            organization_name: ctx.organization_name,
            app_name: ctx.app_name,
            preset_name: ctx.invite.name,
            error: Some(message),
            unavailable: true,
        })
        .into_response();
    }

    render(AcceptInviteTemplate {
        token,
        email: ctx.invite.email,
        organization_name: ctx.organization_name,
        app_name: ctx.app_name,
        preset_name: ctx.invite.name,
        error: None,
        unavailable: false,
    })
    .into_response()
}

#[derive(Deserialize)]
struct AcceptInviteForm {
    name: Option<String>,
    password: String,
}

async fn accept_invite_form(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Form(body): Form<AcceptInviteForm>,
) -> Response {
    let ctx = match invites::get_invite_by_token(&state.db, &token).await {
        Ok(Some(c)) => c,
        _ => {
            return render(AcceptInviteTemplate {
                token,
                email: String::new(),
                organization_name: "Invite not found".to_string(),
                app_name: String::new(),
                preset_name: None,
                error: Some("This invite link is invalid.".to_string()),
                unavailable: true,
            })
            .into_response();
        }
    };

    let name = body.name.as_deref().unwrap_or("");

    match invites::accept_invite(&state.db, &token, name, &body.password).await {
        Ok(user) => render(AcceptSuccessTemplate {
            organization_name: ctx.organization_name,
            app_name: ctx.app_name,
            email: user.email,
        })
        .into_response(),
        Err(e) => {
            let message = match e {
                AppError::Validation(m) | AppError::Unauthorized(m) | AppError::Conflict(m) => m,
                _ => "Could not accept invite. Please try again.".to_string(),
            };
            render(AcceptInviteTemplate {
                token,
                email: ctx.invite.email,
                organization_name: ctx.organization_name,
                app_name: ctx.app_name,
                preset_name: ctx.invite.name,
                error: Some(message),
                unavailable: false,
            })
            .into_response()
        }
    }
}

async fn accept_invite_json(
    State(state): State<AppState>,
    Path(token): Path<String>,
    Json(body): Json<AcceptInviteRequest>,
) -> Result<Json<AcceptInviteResponse>> {
    let user = invites::accept_invite(
        &state.db,
        &token,
        body.name.as_deref().unwrap_or(""),
        &body.password,
    )
    .await?;

    Ok(Json(AcceptInviteResponse {
        id: user.id.to_string(),
        email: user.email,
        name: user.name,
    }))
}

fn render<T: Template>(tmpl: T) -> Html<String> {
    match tmpl.render() {
        Ok(html) => Html(html),
        Err(e) => Html(format!("<pre>Template error: {e}</pre>")),
    }
}
