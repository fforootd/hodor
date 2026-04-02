use axum::{Router, extract::{Path, State}, response::Response, routing::get, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{ApiState, response};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/providers", get(list).post(create))
        .route("/providers/{id}", get(get_one).patch(update).delete(delete_one))
}

#[derive(Deserialize)]
pub struct ProviderRequest { pub name: String, #[serde(default = "default_protocol")] pub protocol: String, #[serde(default)] pub config: serde_json::Value }
fn default_protocol() -> String { "oidc".into() }

#[derive(Serialize)]
pub struct ProviderResponse { pub id: String, pub name: String, pub protocol: String, pub enabled: bool, pub config: serde_json::Value, pub created_at: String }

async fn create(State(s): State<ApiState>, Json(req): Json<ProviderRequest>) -> Response {
    let scoped = s.db.scoped_default();
    let id = Uuid::new_v4().to_string();
    let cfg = serde_json::to_string(&req.config).unwrap_or_else(|_| "{}".into());
    match sqlx::query("INSERT INTO providers (id, instance_id, name, protocol, config) VALUES (?, ?, ?, ?, ?)")
        .bind(&id).bind(scoped.instance_id()).bind(&req.name).bind(&req.protocol).bind(&cfg)
        .execute(scoped.pool()).await {
        Ok(_) => response::json_created(ProviderResponse { id, name: req.name, protocol: req.protocol, enabled: true, config: req.config, created_at: String::new() }),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

async fn list(State(s): State<ApiState>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query_as::<_, (String, String, String, i64, String, String)>(
        "SELECT id, name, protocol, enabled, COALESCE(config,'{}'), created_at FROM providers WHERE instance_id = ? ORDER BY display_order, name")
        .bind(scoped.instance_id()).fetch_all(scoped.pool()).await {
        Ok(rows) => {
            let items: Vec<ProviderResponse> = rows.into_iter().map(|r| ProviderResponse { id: r.0, name: r.1, protocol: r.2, enabled: r.3 != 0, config: serde_json::from_str(&r.4).unwrap_or_default(), created_at: r.5 }).collect();
            response::json_ok(response::ListResponse { items, next_cursor: None, total: None })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query_as::<_, (String, String, String, i64, String, String)>(
        "SELECT id, name, protocol, enabled, COALESCE(config,'{}'), created_at FROM providers WHERE instance_id = ? AND id = ?")
        .bind(scoped.instance_id()).bind(&id).fetch_optional(scoped.pool()).await {
        Ok(Some(r)) => response::json_ok(ProviderResponse { id: r.0, name: r.1, protocol: r.2, enabled: r.3 != 0, config: serde_json::from_str(&r.4).unwrap_or_default(), created_at: r.5 }),
        Ok(None) => response::not_found("provider not found"), Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn update(State(s): State<ApiState>, Path(id): Path<String>, Json(req): Json<ProviderRequest>) -> Response {
    let scoped = s.db.scoped_default();
    let cfg = serde_json::to_string(&req.config).unwrap_or_else(|_| "{}".into());
    match sqlx::query("UPDATE providers SET name = ?, config = ?, updated_at = datetime('now') WHERE instance_id = ? AND id = ?")
        .bind(&req.name).bind(&cfg).bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await {
        Ok(r) if r.rows_affected() == 0 => response::not_found("provider not found"),
        Ok(_) => response::json_ok(serde_json::json!({"id": id, "updated": true})),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query("DELETE FROM providers WHERE instance_id = ? AND id = ?")
        .bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await {
        Ok(r) if r.rows_affected() == 0 => response::not_found("provider not found"), Ok(_) => response::no_content(), Err(e) => response::internal_error(format!("{e}")),
    }
}
