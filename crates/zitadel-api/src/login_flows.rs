// TODO(CLAUDE-2): Migrate login flow management to use cases (CLAUDE-2 handles login execution).
use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_db::{
    create_login_flow as db_create_login_flow, current_instance_id, delete_instance_row,
    get_login_flow_record, list_login_flow_records, resolve_login_flow as db_resolve_login_flow,
    set_login_flow_state, update_login_flow as db_update_login_flow,
};

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

impl From<zitadel_db::LoginFlowRecord> for LoginFlowResponse {
    fn from(record: zitadel_db::LoginFlowRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            strategy: record.strategy,
            state: record.state,
            is_default: record.is_default,
            enabled: record.enabled,
            priority: record.priority,
            config: serde_json::from_str(&record.config_json).unwrap_or_default(),
            audience: serde_json::from_str(&record.audience_json).unwrap_or_default(),
            auth_methods: serde_json::from_str(&record.auth_methods_json).unwrap_or_default(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

async fn create(State(s): State<ApiState>, Json(req): Json<LoginFlowRequest>) -> Response {
    let id = Uuid::new_v4().to_string();
    let config = response::to_json_string(&req.config);
    let audience = response::to_json_string(&req.audience);
    let auth_methods = response::to_json_string(&req.auth_methods);
    match db_create_login_flow(
        &s.db,
        current_instance_id().as_ref(),
        &id,
        &req.name,
        &req.strategy,
        &config,
        &audience,
        &auth_methods,
        req.is_default,
    )
    .await
    {
        Ok(_) => response::json_created(LoginFlowResponse {
            id,
            name: req.name,
            strategy: req.strategy,
            state: "draft".into(),
            is_default: req.is_default,
            enabled: true,
            priority: 0,
            config: req.config,
            audience: req.audience,
            auth_methods: req.auth_methods,
            created_at: String::new(),
            updated_at: String::new(),
        }),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

async fn list(State(s): State<ApiState>, Query(p): Query<response::PaginationParams>) -> Response {
    let cursor = p.cursor.unwrap_or_default();
    match list_login_flow_records(
        &s.db,
        current_instance_id().as_ref(),
        &cursor,
        p.limit.min(200),
    )
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
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match get_login_flow_record(&s.db, current_instance_id().as_ref(), &id).await {
        Ok(Some(f)) => response::json_ok(LoginFlowResponse::from(f)),
        Ok(None) => response::not_found("login flow not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn update(
    State(s): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<LoginFlowRequest>,
) -> Response {
    let config = response::to_json_string(&req.config);
    let auth_methods = response::to_json_string(&req.auth_methods);
    match db_update_login_flow(
        &s.db,
        current_instance_id().as_ref(),
        &id,
        &req.name,
        &req.strategy,
        &config,
        &auth_methods,
        req.is_default,
    )
    .await
    {
        Ok(false) => response::not_found("login flow not found"),
        Ok(true) => match get_login_flow_record(&s.db, current_instance_id().as_ref(), &id).await {
            Ok(Some(f)) => response::json_ok(LoginFlowResponse::from(f)),
            _ => response::not_found("not found"),
        },
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match delete_instance_row(&s.db, current_instance_id().as_ref(), "login_flows", &id).await {
        Ok(true) => response::no_content(),
        Ok(false) => response::not_found("login flow not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn promote(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match set_login_flow_state(&s.db, current_instance_id().as_ref(), &id, "active", true).await {
        Ok(true) => response::json_ok(serde_json::json!({"id": id, "state": "active"})),
        Ok(false) => response::not_found("login flow not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn archive(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match set_login_flow_state(
        &s.db,
        current_instance_id().as_ref(),
        &id,
        "archived",
        false,
    )
    .await
    {
        Ok(true) => response::json_ok(serde_json::json!({"id": id, "state": "archived"})),
        Ok(false) => response::not_found("login flow not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

/// Resolve which login flow to use based on audience targeting.
async fn resolve(State(s): State<ApiState>, Json(_body): Json<serde_json::Value>) -> Response {
    match db_resolve_login_flow(&s.db, current_instance_id().as_ref()).await {
        Ok(Some(r)) => response::json_ok(serde_json::json!({"flow_id": r.0, "flow_name": r.1})),
        Ok(None) => response::json_ok(serde_json::json!({"flow_id": null, "flow_name": "default"})),
        Err(e) => response::internal_error(format!("{e}")),
    }
}
