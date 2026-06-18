use askama::Template;
use axum::{
    Router,
    extract::{Extension, Path, Query, State},
    routing::get,
};
use serde::Deserialize;

use crate::{
    AppState,
    ids::ApplicationId,
    services::admin_auth::AdminSession,
    services::{admin_access, admin_ops, datastar},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/admin/sse/apps/{app_id}/users/search",
            get(search_users_sse),
        )
        .route(
            "/admin/sse/apps/{app_id}/users/picker",
            get(picker_users_sse),
        )
}

#[derive(Debug, Deserialize)]
struct UserSearchQuery {
    q: Option<String>,
}

#[derive(Template)]
#[template(path = "admin/fragments/user_table_rows.html", escape = "none")]
struct UserTableRowsTemplate {
    users: Vec<UserRow>,
}

struct UserRow {
    id: String,
    name: String,
    email: String,
    email_verified: bool,
    created_at: String,
}

#[derive(Template)]
#[template(path = "admin/fragments/user_picker_results.html", escape = "none")]
struct UserPickerResultsTemplate {
    users: Vec<PickerUserRow>,
}

struct PickerUserRow {
    id: String,
    name: String,
    email: String,
}

async fn picker_users_sse(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
    Query(query): Query<UserSearchQuery>,
) -> axum::response::Response {
    let app_id = match app_id.parse::<ApplicationId>() {
        Ok(id) => id,
        Err(_) => return datastar::sse_response(String::new()),
    };

    if !admin_access::can_access_app(&state.db, &identity, app_id)
        .await
        .unwrap_or(false)
    {
        return datastar::sse_response(String::new());
    }

    let q = query.q.unwrap_or_default();
    let users = match admin_ops::search_tenant_users(&state.db, app_id, &q).await {
        Ok(rows) => rows,
        Err(_) => return datastar::sse_response(String::new()),
    };

    let rows: Vec<PickerUserRow> = users
        .into_iter()
        .map(|u| PickerUserRow {
            id: u.id,
            name: u.name,
            email: u.email,
        })
        .collect();

    let tmpl = UserPickerResultsTemplate { users: rows };
    let html = match tmpl.render() {
        Ok(h) => h,
        Err(_) => String::new(),
    };

    let body = datastar::sse_patch_elements("#user-picker-results", "inner", &html);
    datastar::sse_response(body)
}

async fn search_users_sse(
    State(state): State<AppState>,
    Extension(identity): Extension<AdminSession>,
    Path(app_id): Path<String>,
    Query(query): Query<UserSearchQuery>,
) -> axum::response::Response {
    let app_id = match app_id.parse::<ApplicationId>() {
        Ok(id) => id,
        Err(_) => return datastar::sse_response(String::new()),
    };

    if !admin_access::can_access_app(&state.db, &identity, app_id)
        .await
        .unwrap_or(false)
    {
        return datastar::sse_response(String::new());
    }

    let q = query.q.unwrap_or_default();
    let users = match admin_ops::search_tenant_users(&state.db, app_id, &q).await {
        Ok(rows) => rows,
        Err(_) => return datastar::sse_response(String::new()),
    };

    let rows: Vec<UserRow> = users
        .into_iter()
        .map(|u| UserRow {
            id: u.id,
            name: u.name,
            email: u.email,
            email_verified: u.email_verified,
            created_at: u.created_at.format("%Y-%m-%d %H:%M UTC").to_string(),
        })
        .collect();

    let tmpl = UserTableRowsTemplate { users: rows };
    let html = match tmpl.render() {
        Ok(h) => h,
        Err(_) => String::new(),
    };

    let body = datastar::sse_patch_elements("#users-table-body", "inner", &html);
    datastar::sse_response(body)
}
