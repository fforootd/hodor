use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use serde::Serialize;

pub fn routes() -> Router<ApiState> {
    Router::new().route(
        "/settings/{type_}",
        get(get_settings).put(put_settings).delete(delete_settings),
    )
}

#[derive(Serialize)]
struct SettingsResponse {
    #[serde(rename = "type")]
    type_: String,
    scope: String,
    data: serde_json::Value,
}

async fn get_settings(State(s): State<ApiState>, Path(type_): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    // Hierarchical resolution: instance → org → app. For POC, just instance scope.
    let sql = format!(
        "SELECT type, scope, {} FROM settings WHERE instance_id = $1 AND type = $2 ORDER BY CASE scope WHEN 'app' THEN 1 WHEN 'org' THEN 2 ELSE 3 END LIMIT 1",
        scoped.as_text("data"),
    );
    match sqlx::query_as::<_, (String, String, String)>(&sql)
        .bind(scoped.instance_id())
        .bind(&type_)
        .fetch_optional(scoped.pool())
        .await
    {
        Ok(Some(r)) => response::json_ok(SettingsResponse {
            type_: r.0,
            scope: r.1,
            data: serde_json::from_str(&r.2).unwrap_or_default(),
        }),
        Ok(None) => response::not_found(format!("settings '{type_}' not found")),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn put_settings(
    State(s): State<ApiState>,
    Path(type_): Path<String>,
    Json(data): Json<serde_json::Value>,
) -> Response {
    let scoped = s.db.scoped_default();
    let data_str = serde_json::to_string(&data).unwrap_or_else(|_| "{}".into());
    let id = uuid::Uuid::new_v4().to_string();
    let sql = format!(
        "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) VALUES ($1, $2, $3, 'instance', '', {}) \
         ON CONFLICT(instance_id, type, scope, scope_id) DO UPDATE SET data = {}, updated_at = CURRENT_TIMESTAMP",
        scoped.json_bind(4),
        scoped.json_bind(4),
    );
    match sqlx::query(&sql)
        .bind(&id)
        .bind(scoped.instance_id())
        .bind(&type_)
        .bind(&data_str)
        .execute(scoped.pool())
        .await
    {
        Ok(_) => response::json_ok(SettingsResponse {
            type_,
            scope: "instance".into(),
            data,
        }),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_settings(State(s): State<ApiState>, Path(type_): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query("DELETE FROM settings WHERE instance_id = $1 AND type = $2")
        .bind(scoped.instance_id())
        .bind(&type_)
        .execute(scoped.pool())
        .await
    {
        Ok(_) => response::no_content(),
        Err(e) => response::internal_error(format!("{e}")),
    }
}
