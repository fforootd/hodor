use axum::{Router, extract::{Query, State}, response::Response, routing::get};
use serde::{Deserialize, Serialize};
use crate::{ApiState, response};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/events", get(list_events))
}

#[derive(Deserialize)]
pub struct EventParams { #[serde(default = "default_limit")] pub limit: i64, pub cursor: Option<String>, pub event_type: Option<String> }
fn default_limit() -> i64 { 50 }

#[derive(Serialize)]
pub struct EventResponse { pub id: String, pub event_type: String, pub aggregate_id: Option<String>, pub aggregate_type: Option<String>, pub actor_id: Option<String>, pub created_at: String }

async fn list_events(State(s): State<ApiState>, Query(p): Query<EventParams>) -> Response {
    let scoped = s.db.scoped_default();
    let cursor = p.cursor.unwrap_or_default();
    match sqlx::query_as::<_, (String, String, Option<String>, Option<String>, Option<String>, String)>(
        "SELECT id, event_type, aggregate_id, aggregate_type, actor_id, created_at FROM events WHERE instance_id = ? AND id > ? ORDER BY created_at DESC LIMIT ?")
        .bind(scoped.instance_id()).bind(&cursor).bind(p.limit.min(200))
        .fetch_all(scoped.pool()).await {
        Ok(rows) => {
            let items: Vec<EventResponse> = rows.into_iter().map(|r| EventResponse { id: r.0, event_type: r.1, aggregate_id: r.2, aggregate_type: r.3, actor_id: r.4, created_at: r.5 }).collect();
            response::json_ok(response::ListResponse { items, next_cursor: None, total: None })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}
