use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

async fn create(State(s): State<ApiState>, Json(req): Json<LoginFlowRequest>) -> Response {
    let scoped = s.db.scoped_default();
    let id = Uuid::new_v4().to_string();
    let config = response::to_json_string(&req.config);
    let audience = response::to_json_string(&req.audience);
    let auth_methods = response::to_json_string(&req.auth_methods);
    let sql = format!(
        "INSERT INTO login_flows (id, instance_id, name, strategy, config, audience, auth_methods, is_default) \
         VALUES ($1, $2, $3, $4, {}, {}, {}, $8)",
        scoped.json_bind(5),
        scoped.json_bind(6),
        scoped.json_bind(7),
    );

    match sqlx::query(&sql)
        .bind(&id)
        .bind(scoped.instance_id())
        .bind(&req.name)
        .bind(&req.strategy)
        .bind(&config)
        .bind(&audience)
        .bind(&auth_methods)
        .bind(req.is_default)
        .execute(scoped.pool())
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
    let scoped = s.db.scoped_default();
    let cursor = p.cursor.unwrap_or_default();
    let is_default = scoped.bool_as_int("is_default");
    let enabled = scoped.bool_as_int("enabled");
    let config = scoped.as_text("config");
    let audience = scoped.as_text("audience");
    let auth_methods = scoped.as_text("auth_methods");
    let (created_at, updated_at) = scoped.select_timestamps();
    let sql = format!(
        "SELECT id, name, strategy, state, {is_default}, {enabled}, priority, \
         COALESCE({config}, '{{}}'), COALESCE({audience}, '{{}}'), COALESCE({auth_methods}, '{{}}'), {created_at}, {updated_at} \
         FROM login_flows WHERE instance_id = $1 AND id > $2 ORDER BY priority DESC, name LIMIT $3"
    );
    match sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            i64,
            i64,
            i64,
            String,
            String,
            String,
            String,
            String,
        ),
    >(&sql)
    .bind(scoped.instance_id())
    .bind(&cursor)
    .bind(p.limit.min(200))
    .fetch_all(scoped.pool())
    .await
    {
        Ok(rows) => {
            let items: Vec<LoginFlowResponse> = rows
                .into_iter()
                .map(|r| LoginFlowResponse {
                    id: r.0,
                    name: r.1,
                    strategy: r.2,
                    state: r.3,
                    is_default: r.4 != 0,
                    enabled: r.5 != 0,
                    priority: r.6,
                    config: serde_json::from_str(&r.7).unwrap_or_default(),
                    audience: serde_json::from_str(&r.8).unwrap_or_default(),
                    auth_methods: serde_json::from_str(&r.9).unwrap_or_default(),
                    created_at: r.10,
                    updated_at: r.11,
                })
                .collect();
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
    match load(&s.db.scoped_default(), &id).await {
        Ok(Some(f)) => response::json_ok(f),
        Ok(None) => response::not_found("login flow not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn update(
    State(s): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<LoginFlowRequest>,
) -> Response {
    let scoped = s.db.scoped_default();
    let config = response::to_json_string(&req.config);
    let auth_methods = response::to_json_string(&req.auth_methods);
    let sql = format!(
        "UPDATE login_flows SET name = $1, strategy = $2, config = {}, auth_methods = {}, is_default = $5, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $6 AND id = $7",
        scoped.json_bind(3),
        scoped.json_bind(4),
    );
    match sqlx::query(&sql)
        .bind(&req.name)
        .bind(&req.strategy)
        .bind(&config)
        .bind(&auth_methods)
        .bind(req.is_default)
        .bind(scoped.instance_id())
        .bind(&id)
        .execute(scoped.pool())
        .await
    {
        Ok(r) if r.rows_affected() == 0 => response::not_found("login flow not found"),
        Ok(_) => match load(&scoped, &id).await {
            Ok(Some(f)) => response::json_ok(f),
            _ => response::not_found("not found"),
        },
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    response::delete_by_id(&s.db.scoped_default(), "login_flows", &id, "login flow").await
}

async fn promote(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    let _ = sqlx::query("UPDATE login_flows SET state = 'active', enabled = TRUE, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND id = $2")
        .bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await;
    response::json_ok(serde_json::json!({"id": id, "state": "active"}))
}

async fn archive(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    let _ = sqlx::query("UPDATE login_flows SET state = 'archived', enabled = FALSE, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND id = $2")
        .bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await;
    response::json_ok(serde_json::json!({"id": id, "state": "archived"}))
}

/// Resolve which login flow to use based on audience targeting.
async fn resolve(State(s): State<ApiState>, Json(_body): Json<serde_json::Value>) -> Response {
    let scoped = s.db.scoped_default();
    // POC: return the default flow or first active flow.
    match sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM login_flows WHERE instance_id = $1 AND enabled = TRUE ORDER BY is_default DESC, priority DESC LIMIT 1")
        .bind(scoped.instance_id()).fetch_optional(scoped.pool()).await {
        Ok(Some(r)) => response::json_ok(serde_json::json!({"flow_id": r.0, "flow_name": r.1})),
        Ok(None) => response::json_ok(serde_json::json!({"flow_id": null, "flow_name": "default"})),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn load(
    scoped: &zitadel_db::scoped::ScopedDb,
    id: &str,
) -> anyhow::Result<Option<LoginFlowResponse>> {
    let is_default = scoped.bool_as_int("is_default");
    let enabled = scoped.bool_as_int("enabled");
    let config = scoped.as_text("config");
    let audience = scoped.as_text("audience");
    let auth_methods = scoped.as_text("auth_methods");
    let (created_at, updated_at) = scoped.select_timestamps();
    let sql = format!(
        "SELECT id, name, strategy, state, {is_default}, {enabled}, priority, \
         COALESCE({config}, '{{}}'), COALESCE({audience}, '{{}}'), COALESCE({auth_methods}, '{{}}'), {created_at}, {updated_at} \
         FROM login_flows WHERE instance_id = $1 AND id = $2"
    );
    let row = sqlx::query_as::<
        _,
        (
            String,
            String,
            String,
            String,
            i64,
            i64,
            i64,
            String,
            String,
            String,
            String,
            String,
        ),
    >(&sql)
    .bind(scoped.instance_id())
    .bind(id)
    .fetch_optional(scoped.pool())
    .await?;
    Ok(row.map(|r| LoginFlowResponse {
        id: r.0,
        name: r.1,
        strategy: r.2,
        state: r.3,
        is_default: r.4 != 0,
        enabled: r.5 != 0,
        priority: r.6,
        config: serde_json::from_str(&r.7).unwrap_or_default(),
        audience: serde_json::from_str(&r.8).unwrap_or_default(),
        auth_methods: serde_json::from_str(&r.9).unwrap_or_default(),
        created_at: r.10,
        updated_at: r.11,
    }))
}
