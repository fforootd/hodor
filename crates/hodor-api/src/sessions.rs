use axum::{Router, extract::{Path, State}, response::Response, routing::get, Json};
use serde::Serialize;
use crate::{ApiState, response};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session))
        .route("/sessions/{id}/revoke", axum::routing::post(revoke_session))
}

#[derive(Serialize)]
struct SessionResponse { id: String, user_id: String, org_id: String, created_at: String, expires_at: Option<String>, revoked_at: Option<String> }

async fn list_sessions(State(s): State<ApiState>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query_as::<_, (String, String, String, String, Option<String>, Option<String>)>(
        "SELECT id, user_id, org_id, created_at, expires_at, revoked_at FROM sessions WHERE instance_id = ? ORDER BY created_at DESC LIMIT 50")
        .bind(scoped.instance_id()).fetch_all(scoped.pool()).await {
        Ok(rows) => {
            let items: Vec<SessionResponse> = rows.into_iter().map(|r| SessionResponse { id: r.0, user_id: r.1, org_id: r.2, created_at: r.3, expires_at: r.4, revoked_at: r.5 }).collect();
            response::json_ok(response::ListResponse { items, next_cursor: None, total: None })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn get_session(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query_as::<_, (String, String, String, String, Option<String>, Option<String>)>(
        "SELECT id, user_id, org_id, created_at, expires_at, revoked_at FROM sessions WHERE instance_id = ? AND id = ?")
        .bind(scoped.instance_id()).bind(&id).fetch_optional(scoped.pool()).await {
        Ok(Some(r)) => response::json_ok(SessionResponse { id: r.0, user_id: r.1, org_id: r.2, created_at: r.3, expires_at: r.4, revoked_at: r.5 }),
        Ok(None) => response::not_found("session not found"), Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn revoke_session(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query("UPDATE sessions SET revoked_at = datetime('now') WHERE instance_id = ? AND id = ?")
        .bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await {
        Ok(r) if r.rows_affected() == 0 => response::not_found("session not found"),
        Ok(_) => response::no_content(), Err(e) => response::internal_error(format!("{e}")),
    }
}
