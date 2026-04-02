use axum::{Router, extract::{Path, State}, response::Response, routing::{get, put, delete}, Json};
use serde::{Deserialize, Serialize};
use crate::{ApiState, response};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/settings/{type_}", get(get_settings).put(put_settings).delete(delete_settings))
}

#[derive(Serialize)]
struct SettingsResponse { #[serde(rename = "type")] type_: String, scope: String, data: serde_json::Value }

async fn get_settings(State(s): State<ApiState>, Path(type_): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    // Hierarchical resolution: instance → org → app. For POC, just instance scope.
    match sqlx::query_as::<_, (String, String, String)>(
        "SELECT type, scope, data FROM settings WHERE instance_id = ? AND type = ? ORDER BY CASE scope WHEN 'app' THEN 1 WHEN 'org' THEN 2 ELSE 3 END LIMIT 1")
        .bind(scoped.instance_id()).bind(&type_).fetch_optional(scoped.pool()).await {
        Ok(Some(r)) => response::json_ok(SettingsResponse { type_: r.0, scope: r.1, data: serde_json::from_str(&r.2).unwrap_or_default() }),
        Ok(None) => response::not_found(format!("settings '{type_}' not found")),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn put_settings(State(s): State<ApiState>, Path(type_): Path<String>, Json(data): Json<serde_json::Value>) -> Response {
    let scoped = s.db.scoped_default();
    let data_str = serde_json::to_string(&data).unwrap_or_else(|_| "{}".into());
    let id = uuid::Uuid::new_v4().to_string();
    match sqlx::query(
        "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) VALUES (?, ?, ?, 'instance', '', ?) \
         ON CONFLICT(instance_id, type, scope, scope_id) DO UPDATE SET data = excluded.data, updated_at = datetime('now')")
        .bind(&id).bind(scoped.instance_id()).bind(&type_).bind(&data_str)
        .execute(scoped.pool()).await {
        Ok(_) => response::json_ok(SettingsResponse { type_, scope: "instance".into(), data }),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_settings(State(s): State<ApiState>, Path(type_): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query("DELETE FROM settings WHERE instance_id = ? AND type = ?")
        .bind(scoped.instance_id()).bind(&type_).execute(scoped.pool()).await {
        Ok(_) => response::no_content(), Err(e) => response::internal_error(format!("{e}")),
    }
}
