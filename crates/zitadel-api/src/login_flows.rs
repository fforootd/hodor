use crate::{ApiState, extractors::ResourceId, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_app::login_flows::{CreateLoginFlowCommand, UpdateLoginFlowCommand};
use zitadel_app::repo::LoginFlowRecord;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/login-flows", get(list).post(create))
        .route(
            "/login-flows/{id}",
            get(get_one).patch(update).delete(delete_one),
        )
        .route("/login-flows/{id}/promote", axum::routing::post(promote))
        .route("/login-flows/{id}/archive", axum::routing::post(archive))
        .route("/login-flows/resolve", axum::routing::post(resolve))
}

#[derive(Deserialize)]
pub struct LoginFlowRequest {
    pub name: String,
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default)]
    pub audience: serde_json::Value,
    #[serde(default)]
    pub auth_methods: serde_json::Value,
    #[serde(default)]
    pub is_default: bool,
}
fn default_strategy() -> String {
    "identifier_first".into()
}

#[derive(Serialize)]
pub struct LoginFlowResponse {
    pub id: String,
    pub name: String,
    pub strategy: String,
    pub state: String,
    pub is_default: bool,
    pub enabled: bool,
    pub priority: i64,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub config: serde_json::Value,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub audience: serde_json::Value,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub auth_methods: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl From<LoginFlowRecord> for LoginFlowResponse {
    fn from(record: LoginFlowRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            strategy: record.strategy,
            state: record.state,
            is_default: record.is_default,
            enabled: record.enabled,
            priority: record.priority as i64,
            config: record.config,
            audience: record.audience,
            auth_methods: record.auth_methods,
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

async fn create(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<LoginFlowRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = CreateLoginFlowCommand {
        flow_id: None,
        name: req.name,
        steps: req.config,
        branding: req.audience,
    };
    match s
        .app
        .runner
        .run(&ctx, "login_flow.create", || {
            s.app.create_login_flow.execute(&ctx, cmd)
        })
        .await
    {
        Ok(record) => response::json_created(LoginFlowResponse::from(record)),
        Err(e) => response::app_error(e),
    }
}

async fn list(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(_p): Query<response::PaginationParams>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "login_flow.list", || {
            s.app.list_login_flows.execute(&ctx)
        })
        .await
    {
        Ok(rows) => {
            let items: Vec<LoginFlowResponse> =
                rows.into_iter().map(LoginFlowResponse::from).collect();
            response::json_ok(response::ListResponse {
                items,
                next_cursor: None,
                total: None,
            })
        }
        Err(e) => response::app_error(e),
    }
}

async fn get_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "login_flow.get", || {
            s.app.get_login_flow.execute(&ctx, &id)
        })
        .await
    {
        Ok(f) => response::json_ok(LoginFlowResponse::from(f)),
        Err(e) => response::app_error(e),
    }
}

async fn update(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
    Json(req): Json<LoginFlowRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let flow_id = id.clone();
    let cmd = UpdateLoginFlowCommand {
        flow_id: Some(id),
        name: req.name,
        steps: req.config,
        branding: req.audience,
    };
    match s
        .app
        .runner
        .run(&ctx, "login_flow.update", || {
            s.app.update_login_flow.execute(&ctx, cmd)
        })
        .await
    {
        Ok(()) => {
            // Re-fetch to return updated state
            match s
                .app
                .runner
                .run(&ctx, "login_flow.get", || {
                    s.app.get_login_flow.execute(&ctx, &flow_id)
                })
                .await
            {
                Ok(f) => response::json_ok(LoginFlowResponse::from(f)),
                Err(e) => response::app_error(e),
            }
        }
        Err(e) => response::app_error(e),
    }
}

async fn delete_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "login_flow.delete", || {
            s.app.delete_login_flow.execute(&ctx, &id)
        })
        .await
    {
        Ok(()) => response::no_content(),
        Err(e) => response::app_error(e),
    }
}

async fn promote(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "login_flow.promote", || {
            s.app.promote_login_flow.execute(&ctx, &id)
        })
        .await
    {
        Ok(true) => response::json_ok(serde_json::json!({"id": id, "state": "active"})),
        Ok(false) => response::not_found("login flow not found"),
        Err(e) => response::app_error(e),
    }
}

async fn archive(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "login_flow.archive", || {
            s.app.archive_login_flow.execute(&ctx, &id)
        })
        .await
    {
        Ok(true) => response::json_ok(serde_json::json!({"id": id, "state": "archived"})),
        Ok(false) => response::not_found("login flow not found"),
        Err(e) => response::app_error(e),
    }
}

/// Resolve which login flow to use based on audience targeting.
async fn resolve(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(_body): Json<serde_json::Value>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "login_flow.resolve", || {
            s.app.resolve_login_flow.execute(&ctx)
        })
        .await
    {
        Ok(Some(f)) => {
            response::json_ok(serde_json::json!({"flow_id": f.id, "flow_name": f.name}))
        }
        Ok(None) => {
            response::json_ok(serde_json::json!({"flow_id": null, "flow_name": "default"}))
        }
        Err(e) => response::app_error(e),
    }
}
