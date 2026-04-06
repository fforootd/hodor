use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use zitadel_app::{
    credentials::SetPasswordCommand,
    repo::{ListParams as AppListParams, UserRecord},
    users::{CreateUserCommand, UpdateUserCommand},
};

use crate::{ApiState, extractors::ResourceId, middleware::Identity, response};

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

impl From<UserRecord> for UserResponse {
    fn from(r: UserRecord) -> Self {
        Self {
            id: r.id,
            org_id: r.org_id,
            identifier: r.identifier,
            display_name: r.display_name,
            user_type: r.user_type,
            state: r.state,
            schema_id: r.schema_id,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Deserialize)]
pub struct ListParams {
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub cursor: Option<String>,
    pub org_id: Option<String>,
    pub state: Option<String>,
}

fn default_limit() -> u32 {
    50
}

#[derive(Deserialize)]
pub struct PasswordRequest {
    pub password: String,
}

async fn create_user(
    State(state): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<UserRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let display = if req.display_name.is_empty() {
        req.identifier.clone()
    } else {
        req.display_name
    };
    let cmd = CreateUserCommand {
        identifier: req.identifier,
        display_name: display,
        user_type: "human".to_string(),
        schema_id: req.schema_id,
        org_id: None,
        metadata: req.metadata,
    };
    match state
        .app
        .runner
        .run(&ctx, "user.create", || {
            state.app.create_user.execute(&ctx, cmd)
        })
        .await
    {
        Ok(user) => response::json_created(UserResponse::from(user)),
        Err(e) => response::app_error(e),
    }
}

async fn get_user(
    State(state): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match state
        .app
        .runner
        .run(&ctx, "user.get", || state.app.get_user.execute(&ctx, &id))
        .await
    {
        Ok(user) => response::json_ok(UserResponse::from(user)),
        Err(e) => response::app_error(e),
    }
}

async fn list_users(
    State(state): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(params): Query<ListParams>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let app_params = AppListParams {
        limit: Some(params.limit.min(200)),
        cursor: params.cursor,
        search: None,
    };
    match state
        .app
        .runner
        .run(&ctx, "user.list", || {
            state
                .app
                .list_users
                .execute(&ctx, params.org_id.as_deref(), &app_params)
        })
        .await
    {
        Ok(result) => {
            let items: Vec<UserResponse> =
                result.items.into_iter().map(UserResponse::from).collect();
            response::json_ok(response::ListResponse {
                items,
                next_cursor: result.next_cursor,
                total: result.total_count.map(|c| c as i64),
            })
        }
        Err(e) => response::app_error(e),
    }
}

async fn update_user(
    State(state): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
    Json(req): Json<UserRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = UpdateUserCommand {
        user_id: id,
        display_name: if req.display_name.is_empty() {
            None
        } else {
            Some(req.display_name)
        },
        metadata: if req.metadata.is_null() {
            None
        } else {
            Some(req.metadata)
        },
    };
    match state
        .app
        .runner
        .run(&ctx, "user.update", || {
            state.app.update_user.execute(&ctx, cmd)
        })
        .await
    {
        Ok(user) => response::json_ok(UserResponse::from(user)),
        Err(e) => response::app_error(e),
    }
}

async fn delete_user(
    State(state): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match state
        .app
        .runner
        .run(&ctx, "user.delete", || {
            state.app.delete_user.execute(&ctx, &id)
        })
        .await
    {
        Ok(()) => {
            if let Err(error) = state.app.repos.fga_admin.rebuild_platform_store().await {
                return response::internal(error);
            }
            response::no_content()
        }
        Err(e) => response::app_error(e),
    }
}

async fn set_password(
    State(state): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
    Json(req): Json<PasswordRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    // Hash password in the transport layer (Swapper is transport infrastructure).
    let hash = match state.passwords.hash(&req.password) {
        Ok(h) => h,
        Err(e) => return response::internal(e),
    };
    let cred_json = zitadel_authn::password::encode_credential_json(&hash);
    let cmd = SetPasswordCommand {
        user_id: id,
        password_hash: cred_json,
    };
    match state
        .app
        .runner
        .run(&ctx, "user.set_password", || {
            state.app.set_password.execute(&ctx, cmd)
        })
        .await
    {
        Ok(()) => response::no_content(),
        Err(e) => response::app_error(e),
    }
}
