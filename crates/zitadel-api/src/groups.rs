use axum::{Router, extract::{Path, Query, State}, response::Response, routing::get, Json};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::{ApiState, response};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/groups", get(list).post(create))
        .route("/groups/{id}", get(get_one).patch(update).delete(delete_one))
}

#[derive(Deserialize)]
pub struct CreateRequest { #[serde(default)] pub name: String, #[serde(default)] pub metadata: serde_json::Value }

#[derive(Serialize)]
pub struct ItemResponse { pub id: String, pub name: String, pub state: String, pub created_at: String, pub updated_at: String }

#[derive(Deserialize)]
pub struct ListParams { #[serde(default = "default_limit")] pub limit: i64, pub cursor: Option<String> }
fn default_limit() -> i64 { 50 }

async fn create(State(s): State<ApiState>, Json(req): Json<CreateRequest>) -> Response {
    if req.name.is_empty() { return response::bad_request("name is required"); }
    let scoped = s.db.scoped_default();
    let id = Uuid::new_v4().to_string();
    match sqlx::query("INSERT INTO groups (id, instance_id, name, state) VALUES (?, ?, ?, 'active')")
        .bind(&id).bind(scoped.instance_id()).bind(&req.name).execute(scoped.pool()).await {
        Ok(_) => response::json_created(ItemResponse { id, name: req.name, state: "active".into(), created_at: String::new(), updated_at: String::new() }),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, name, state, created_at, updated_at FROM groups WHERE instance_id = ? AND id = ?")
        .bind(scoped.instance_id()).bind(&id).fetch_optional(scoped.pool()).await {
        Ok(Some(r)) => response::json_ok(ItemResponse { id: r.0, name: r.1, state: r.2, created_at: r.3, updated_at: r.4 }),
        Ok(None) => response::not_found("not found"), Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn list(State(s): State<ApiState>, Query(p): Query<ListParams>) -> Response {
    let scoped = s.db.scoped_default();
    let cursor = p.cursor.unwrap_or_default();
    match sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, name, state, created_at, updated_at FROM groups WHERE instance_id = ? AND id > ? ORDER BY id LIMIT ?")
        .bind(scoped.instance_id()).bind(&cursor).bind(p.limit.min(200))
        .fetch_all(scoped.pool()).await {
        Ok(rows) => {
            let items: Vec<ItemResponse> = rows.into_iter().map(|r| ItemResponse { id: r.0, name: r.1, state: r.2, created_at: r.3, updated_at: r.4 }).collect();
            response::json_ok(response::ListResponse { items, next_cursor: None, total: None })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn update(State(s): State<ApiState>, Path(id): Path<String>, Json(req): Json<CreateRequest>) -> Response {
    let scoped = s.db.scoped_default();
    if req.name.is_empty() { return response::bad_request("name required"); }
    match sqlx::query("UPDATE groups SET name = ?, updated_at = datetime('now') WHERE instance_id = ? AND id = ?")
        .bind(&req.name).bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await {
        Ok(r) if r.rows_affected() == 0 => response::not_found("not found"),
        Ok(_) => response::json_ok(serde_json::json!({"id": id, "updated": true})),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query("DELETE FROM groups WHERE instance_id = ? AND id = ?")
        .bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await {
        Ok(r) if r.rows_affected() == 0 => response::not_found("not found"), Ok(_) => response::no_content(), Err(e) => response::internal_error(format!("{e}")),
    }
}
