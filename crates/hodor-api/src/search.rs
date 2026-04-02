use axum::{Router, extract::{Query, State}, response::Response, routing::get};
use serde::Deserialize;
use crate::{ApiState, response};

pub fn routes() -> Router<ApiState> {
    Router::new().route("/search", get(search))
}

#[derive(Deserialize)]
pub struct SearchParams { pub q: Option<String>, #[serde(default = "default_limit")] pub limit: i64 }
fn default_limit() -> i64 { 20 }

async fn search(State(s): State<ApiState>, Query(p): Query<SearchParams>) -> Response {
    let q = match p.q {
        Some(q) if !q.is_empty() => q,
        _ => return response::bad_request("q parameter required"),
    };
    let scoped = s.db.scoped_default();
    let pattern = format!("%{q}%");
    // Search across users and orgs.
    let users: Vec<serde_json::Value> = match sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, identifier, display_name FROM users WHERE instance_id = ? AND (identifier LIKE ? OR display_name LIKE ?) LIMIT ?")
        .bind(scoped.instance_id()).bind(&pattern).bind(&pattern).bind(p.limit)
        .fetch_all(scoped.pool()).await {
        Ok(rows) => rows.into_iter().map(|r| serde_json::json!({"type": "user", "id": r.0, "identifier": r.1, "display_name": r.2})).collect(),
        Err(_) => vec![],
    };
    let orgs: Vec<serde_json::Value> = match sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM orgs WHERE instance_id = ? AND name LIKE ? LIMIT ?")
        .bind(scoped.instance_id()).bind(&pattern).bind(p.limit)
        .fetch_all(scoped.pool()).await {
        Ok(rows) => rows.into_iter().map(|r| serde_json::json!({"type": "org", "id": r.0, "name": r.1})).collect(),
        Err(_) => vec![],
    };
    let mut items = users;
    items.extend(orgs);
    response::json_ok(response::ListResponse { items, next_cursor: None, total: None })
}
