use crate::{ApiState, response};
use axum::{Router, extract::State, response::Response, routing::get};
use serde_json::{Value, json};
use sqlx::Row;

pub fn routes() -> Router<ApiState> {
    Router::new().route("/jobs", get(list_jobs))
}

async fn list_jobs(State(s): State<ApiState>) -> Response {
    let scoped = s.db.scoped_default();
    let config_json = scoped.as_text("config_json");
    let last_run_at = scoped.as_text("last_run_at");
    let next_run_at = scoped.as_text("next_run_at");
    let lease_expires_at = scoped.as_text("lease_expires_at");
    let created_at = scoped.as_text("created_at");
    let updated_at = scoped.as_text("updated_at");
    let enabled = scoped.bool_as_int("enabled");

    let sql = format!(
        "SELECT name, display_name, description, cron, {enabled}, last_status, last_error, \
                run_count, last_rows_removed, {config_json}, {last_run_at}, {next_run_at}, \
                {lease_expires_at}, {created_at}, {updated_at} \
         FROM jobs WHERE instance_id = $1 ORDER BY name ASC"
    );

    match sqlx::query(&sql)
        .bind(scoped.instance_id())
        .fetch_all(scoped.pool())
        .await
    {
        Ok(rows) => {
            let items: Vec<Value> = rows
                .into_iter()
                .map(|row| {
                    let meta: Value =
                        serde_json::from_str(&row.get::<String, _>(9)).unwrap_or_else(|_| json!({}));
                    let last_status: String = row.get(5);
                    let status = if last_status == "idle" {
                        "scheduled".to_string()
                    } else {
                        last_status
                    };
                    json!({
                        "name": row.get::<String, _>(0),
                        "display_name": row.get::<String, _>(1),
                        "description": row.get::<String, _>(2),
                        "schedule": row.get::<String, _>(3),
                        "enabled": row.get::<i64, _>(4) != 0,
                        "status": status,
                        "last_error": row.get::<String, _>(6),
                        "run_count": row.get::<i64, _>(7),
                        "last_removed_count": row.get::<i64, _>(8),
                        "strategy": meta.get("strategy").and_then(Value::as_str).unwrap_or("unknown"),
                        "targets": meta.get("targets").cloned().unwrap_or_else(|| json!([])),
                        "retention": meta.get("retention").and_then(Value::as_str).unwrap_or(""),
                        "cadence": meta.get("cadence").and_then(Value::as_str).unwrap_or(""),
                        "last_run_at": row.get::<Option<String>, _>(10),
                        "next_run_at": row.get::<Option<String>, _>(11),
                        "lease_expires_at": row.get::<Option<String>, _>(12),
                        "created_at": row.get::<String, _>(13),
                        "updated_at": row.get::<String, _>(14),
                    })
                })
                .collect();
            let total = items.len() as i64;
            response::json_ok(response::ListResponse {
                items,
                next_cursor: None::<String>,
                total: Some(total),
            })
        }
        Err(error) => response::internal_error(format!("{error}")),
    }
}
