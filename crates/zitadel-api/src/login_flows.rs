use axum::{Router, extract::{Path, Query, State}, response::Response, routing::get, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{ApiState, response};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/login-flows", get(list).post(create))
        .route("/login-flows/{id}", get(get_one).patch(update).delete(delete_one))
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
fn default_strategy() -> String { "identifier_first".into() }

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

#[derive(Deserialize)]
pub struct ListParams { #[serde(default = "default_limit")] pub limit: i64, pub cursor: Option<String> }
fn default_limit() -> i64 { 50 }

async fn create(State(s): State<ApiState>, Json(req): Json<LoginFlowRequest>) -> Response {
    let scoped = s.db.scoped_default();
    let id = Uuid::new_v4().to_string();
    let config = serde_json::to_string(&req.config).unwrap_or_else(|_| "{}".into());
    let audience = serde_json::to_string(&req.audience).unwrap_or_else(|_| "{}".into());
    let auth_methods = serde_json::to_string(&req.auth_methods).unwrap_or_else(|_| "{}".into());

    match sqlx::query(
        "INSERT INTO login_flows (id, instance_id, name, strategy, config, audience, auth_methods, is_default) VALUES (?, ?, ?, ?, ?, ?, ?, ?)")
        .bind(&id).bind(scoped.instance_id()).bind(&req.name).bind(&req.strategy)
        .bind(&config).bind(&audience).bind(&auth_methods).bind(req.is_default as i32)
        .execute(scoped.pool()).await {
        Ok(_) => response::json_created(LoginFlowResponse {
            id, name: req.name, strategy: req.strategy, state: "draft".into(),
            is_default: req.is_default, enabled: true, priority: 0,
            config: req.config, audience: req.audience, auth_methods: req.auth_methods,
            created_at: String::new(), updated_at: String::new(),
        }),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

async fn list(State(s): State<ApiState>, Query(p): Query<ListParams>) -> Response {
    let scoped = s.db.scoped_default();
    let cursor = p.cursor.unwrap_or_default();
    match sqlx::query_as::<_, (String, String, String, String, i64, i64, i64, String, String, String, String, String)>(
        "SELECT id, name, strategy, state, is_default, enabled, priority, \
         COALESCE(config,'{}'), COALESCE(audience,'{}'), COALESCE(auth_methods,'{}'), created_at, updated_at \
         FROM login_flows WHERE instance_id = ? AND id > ? ORDER BY priority DESC, name LIMIT ?")
        .bind(scoped.instance_id()).bind(&cursor).bind(p.limit.min(200))
        .fetch_all(scoped.pool()).await {
        Ok(rows) => {
            let items: Vec<LoginFlowResponse> = rows.into_iter().map(|r| LoginFlowResponse {
                id: r.0, name: r.1, strategy: r.2, state: r.3, is_default: r.4 != 0, enabled: r.5 != 0,
                priority: r.6, config: serde_json::from_str(&r.7).unwrap_or_default(),
                audience: serde_json::from_str(&r.8).unwrap_or_default(),
                auth_methods: serde_json::from_str(&r.9).unwrap_or_default(),
                created_at: r.10, updated_at: r.11,
            }).collect();
            response::json_ok(response::ListResponse { items, next_cursor: None, total: None })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match load(&s.db.scoped_default(), &id).await {
        Ok(Some(f)) => response::json_ok(f),
        Ok(None) => response::not_found("login flow not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn update(State(s): State<ApiState>, Path(id): Path<String>, Json(req): Json<LoginFlowRequest>) -> Response {
    let scoped = s.db.scoped_default();
    let config = serde_json::to_string(&req.config).unwrap_or_else(|_| "{}".into());
    let auth_methods = serde_json::to_string(&req.auth_methods).unwrap_or_else(|_| "{}".into());
    match sqlx::query("UPDATE login_flows SET name = ?, strategy = ?, config = ?, auth_methods = ?, is_default = ?, updated_at = datetime('now') WHERE instance_id = ? AND id = ?")
        .bind(&req.name).bind(&req.strategy).bind(&config).bind(&auth_methods).bind(req.is_default as i32)
        .bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await {
        Ok(r) if r.rows_affected() == 0 => response::not_found("login flow not found"),
        Ok(_) => match load(&scoped, &id).await { Ok(Some(f)) => response::json_ok(f), _ => response::not_found("not found") },
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query("DELETE FROM login_flows WHERE instance_id = ? AND id = ?")
        .bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await {
        Ok(r) if r.rows_affected() == 0 => response::not_found("not found"),
        Ok(_) => response::no_content(),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn promote(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    let _ = sqlx::query("UPDATE login_flows SET state = 'active', enabled = 1, updated_at = datetime('now') WHERE instance_id = ? AND id = ?")
        .bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await;
    response::json_ok(serde_json::json!({"id": id, "state": "active"}))
}

async fn archive(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    let _ = sqlx::query("UPDATE login_flows SET state = 'archived', enabled = 0, updated_at = datetime('now') WHERE instance_id = ? AND id = ?")
        .bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await;
    response::json_ok(serde_json::json!({"id": id, "state": "archived"}))
}

/// Resolve which login flow to use based on audience targeting.
async fn resolve(State(s): State<ApiState>, Json(body): Json<serde_json::Value>) -> Response {
    let scoped = s.db.scoped_default();
    // POC: return the default flow or first active flow.
    match sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM login_flows WHERE instance_id = ? AND enabled = 1 ORDER BY is_default DESC, priority DESC LIMIT 1")
        .bind(scoped.instance_id()).fetch_optional(scoped.pool()).await {
        Ok(Some(r)) => response::json_ok(serde_json::json!({"flow_id": r.0, "flow_name": r.1})),
        Ok(None) => response::json_ok(serde_json::json!({"flow_id": null, "flow_name": "default"})),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn load(scoped: &zitadel_db::scoped::ScopedDb, id: &str) -> anyhow::Result<Option<LoginFlowResponse>> {
    let row = sqlx::query_as::<_, (String, String, String, String, i64, i64, i64, String, String, String, String, String)>(
        "SELECT id, name, strategy, state, is_default, enabled, priority, \
         COALESCE(config,'{}'), COALESCE(audience,'{}'), COALESCE(auth_methods,'{}'), created_at, updated_at \
         FROM login_flows WHERE instance_id = ? AND id = ?")
        .bind(scoped.instance_id()).bind(id).fetch_optional(scoped.pool()).await?;
    Ok(row.map(|r| LoginFlowResponse {
        id: r.0, name: r.1, strategy: r.2, state: r.3, is_default: r.4 != 0, enabled: r.5 != 0,
        priority: r.6, config: serde_json::from_str(&r.7).unwrap_or_default(),
        audience: serde_json::from_str(&r.8).unwrap_or_default(),
        auth_methods: serde_json::from_str(&r.9).unwrap_or_default(),
        created_at: r.10, updated_at: r.11,
    }))
}
