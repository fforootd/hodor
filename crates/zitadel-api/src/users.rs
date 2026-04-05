use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_db::{
    create_user as db_create_user, current_instance_id, delete_instance_row, first_org_id,
    get_user as db_get_user, list_users as db_list_users,
    replace_password_credential as db_replace_password_credential, update_user as db_update_user,
};

use crate::{ApiState, response};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/users", get(list_users).post(create_user))
        .route(
            "/users/{id}",
            get(get_user).patch(update_user).delete(delete_user),
        )
        .route("/users/{id}/password", post(set_password))
}

#[derive(Deserialize)]
pub struct UserRequest {
    #[serde(default)]
    pub identifier: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub schema_id: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub org_id: String,
    pub identifier: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub display_name: String,
    pub user_type: String,
    pub state: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub schema_id: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl From<zitadel_db::UserRecord> for UserResponse {
    fn from(record: zitadel_db::UserRecord) -> Self {
        Self {
            id: record.id,
            org_id: record.org_id,
            identifier: record.identifier,
            display_name: record.display_name,
            user_type: record.user_type,
            state: record.state,
            schema_id: record.schema_id,
            metadata: serde_json::from_str(&record.metadata_json)
                .unwrap_or(serde_json::Value::Null),
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

#[derive(Deserialize)]
pub struct ListParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub cursor: Option<String>,
    pub org_id: Option<String>,
    pub state: Option<String>,
}

fn default_limit() -> i64 {
    50
}

#[derive(Deserialize)]
pub struct PasswordRequest {
    pub password: String,
}

async fn create_user(State(state): State<ApiState>, Json(req): Json<UserRequest>) -> Response {
    if req.identifier.is_empty() {
        return response::bad_request("identifier is required");
    }
    let id = Uuid::new_v4().to_string();

    let org_id = match first_org_id(&state.db, current_instance_id().as_ref()).await {
        Ok(Some(id)) => id,
        Ok(None) => return response::internal_error("no org found"),
        Err(e) => return response::internal_error(format!("db error: {e}")),
    };

    let metadata = response::to_json_string(&req.metadata);
    let display = if req.display_name.is_empty() {
        req.identifier.clone()
    } else {
        req.display_name.clone()
    };
    match db_create_user(
        &state.db,
        current_instance_id().as_ref(),
        &id,
        &org_id,
        &req.identifier,
        &display,
        &req.schema_id,
        &metadata,
    )
    .await
    {
        Ok(record) => response::json_created(UserResponse::from(record)),
        Err(e) => response::bad_request(format!("create user: {e}")),
    }
}

async fn get_user(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    match db_get_user(&state.db, current_instance_id().as_ref(), &id).await {
        Ok(Some(u)) => response::json_ok(UserResponse::from(u)),
        Ok(None) => response::not_found("user not found"),
        Err(e) => response::internal_error(format!("load user: {e}")),
    }
}

async fn list_users(State(state): State<ApiState>, Query(params): Query<ListParams>) -> Response {
    let limit = params.limit.min(200);
    let cursor = params.cursor.unwrap_or_default();
    match db_list_users(
        &state.db,
        current_instance_id().as_ref(),
        &cursor,
        limit + 1,
    )
    .await
    {
        Ok(rows) => {
            let has_more = rows.len() as i64 > limit;
            let items: Vec<UserResponse> = rows
                .into_iter()
                .take(limit as usize)
                .map(UserResponse::from)
                .collect();
            let next_cursor = if has_more {
                items.last().map(|u| u.id.clone())
            } else {
                None
            };
            response::json_ok(response::ListResponse {
                items,
                next_cursor,
                total: None,
            })
        }
        Err(e) => response::internal_error(format!("list users: {e}")),
    }
}

async fn update_user(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<UserRequest>,
) -> Response {
    if req.display_name.is_empty() && req.state.is_empty() {
        return response::bad_request("no fields to update");
    }
    match db_update_user(
        &state.db,
        current_instance_id().as_ref(),
        &id,
        (!req.display_name.is_empty()).then_some(req.display_name.as_str()),
        (!req.state.is_empty()).then_some(req.state.as_str()),
    )
    .await
    {
        Ok(false) => response::not_found("user not found"),
        Ok(true) => match db_get_user(&state.db, current_instance_id().as_ref(), &id).await {
            Ok(Some(u)) => response::json_ok(UserResponse::from(u)),
            Ok(None) => response::not_found("user not found"),
            Err(e) => response::internal_error(format!("load user: {e}")),
        },
        Err(e) => response::internal_error(format!("update user: {e}")),
    }
}

async fn delete_user(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    match delete_instance_row(&state.db, current_instance_id().as_ref(), "users", &id).await {
        Ok(true) => response::no_content(),
        Ok(false) => response::not_found("user not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn set_password(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<PasswordRequest>,
) -> Response {
    // Hash password.
    let hash = match state.passwords.hash(&req.password) {
        Ok(h) => h,
        Err(e) => return response::internal_error(format!("hash password: {e}")),
    };

    let cred_json = zitadel_authn::password::encode_credential_json(&hash);
    let cred_id = Uuid::new_v4().to_string();
    match db_replace_password_credential(
        &state.db,
        current_instance_id().as_ref(),
        &id,
        &cred_id,
        &cred_json,
    )
    .await
    {
        Ok(_) => response::no_content(),
        Err(e) => response::internal_error(format!("set password: {e}")),
    }
}
