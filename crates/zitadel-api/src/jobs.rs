use crate::{ApiState, response};
use axum::{Router, extract::State, response::Response, routing::get};
use serde_json::{Value, json};
use zitadel_db::{current_instance_id, list_jobs_for_instance};

pub fn routes() -> Router<ApiState> {
    Router::new().route("/jobs", get(list_jobs))
}

async fn list_jobs(State(s): State<ApiState>) -> Response {
    match list_jobs_for_instance(&s.db, current_instance_id().as_ref()).await {
        Ok(rows) => {
            let items: Vec<Value> = rows
                .into_iter()
                .map(|row| {
                    let meta: Value =
                        serde_json::from_str(&row.config_json).unwrap_or_else(|_| json!({}));
                    let last_status = row.last_status.clone();
                    let status = if last_status == "idle" {
                        "scheduled".to_string()
                    } else {
                        last_status
                    };
                    json!({
                        "name": row.name,
                        "display_name": row.display_name,
                        "description": row.description,
                        "schedule": row.cron,
                        "enabled": row.enabled,
                        "status": status,
                        "last_error": row.last_error,
                        "run_count": row.run_count,
                        "last_removed_count": row.last_rows_removed,
                        "strategy": meta.get("strategy").and_then(Value::as_str).unwrap_or("unknown"),
                        "targets": meta.get("targets").cloned().unwrap_or_else(|| json!([])),
                        "retention": meta.get("retention").and_then(Value::as_str).unwrap_or(""),
                        "cadence": meta.get("cadence").and_then(Value::as_str).unwrap_or(""),
                        "last_run_at": row.last_run_at,
                        "next_run_at": row.next_run_at,
                        "lease_expires_at": row.lease_expires_at,
                        "created_at": row.created_at,
                        "updated_at": row.updated_at,
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
