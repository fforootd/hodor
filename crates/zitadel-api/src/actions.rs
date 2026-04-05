use crate::{ApiState, response};
use axum::{
    Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use serde::Serialize;
use zitadel_db::{current_instance_id, delete_instance_row, get_action, list_actions};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/actions", get(list))
        .route("/actions/{id}", get(get_one).delete(delete_one))
}

#[derive(Serialize)]
struct ActionResponse {
    id: String,
    name: String,
    // Aliases for IdentityListView compatibility.
    identifier: String,
    display_name: String,
    state: String,
    // Action-specific fields.
    hook: String,
    action_type: String,
    trigger_expr: String,
    config: serde_json::Value,
    priority: i64,
    enabled: bool,
    fail_open: bool,
    metadata: serde_json::Value,
    created_at: String,
}

async fn list(State(s): State<ApiState>) -> Response {
    match list_actions(&s.db, current_instance_id().as_ref()).await {
        Ok(rows) => {
            let items: Vec<ActionResponse> = rows.into_iter().map(action_from_row).collect();
            response::json_ok(serde_json::json!({ "items": items }))
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match get_action(&s.db, current_instance_id().as_ref(), &id).await {
        Ok(Some(r)) => response::json_ok(action_from_row(r)),
        Ok(None) => response::not_found("action not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match delete_instance_row(&s.db, current_instance_id().as_ref(), "actions", &id).await {
        Ok(true) => response::no_content(),
        Ok(false) => response::not_found("action not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

fn action_from_row(r: zitadel_db::ActionRecord) -> ActionResponse {
    let enabled = r.enabled;
    let name = r.name.clone();
    ActionResponse {
        id: r.id,
        name: name.clone(),
        identifier: name.clone(),
        display_name: name,
        state: if enabled {
            "active".into()
        } else {
            "disabled".into()
        },
        hook: r.hook,
        action_type: r.action_type,
        trigger_expr: r.trigger_expr,
        config: serde_json::from_str(&r.config_json).unwrap_or_default(),
        priority: r.priority,
        enabled,
        fail_open: r.fail_open,
        metadata: serde_json::from_str(&r.metadata_json).unwrap_or_default(),
        created_at: r.created_at,
    }
}
