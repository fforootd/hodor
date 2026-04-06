use crate::{ApiState, extractors::ResourceId, middleware::Identity, response};
use axum::{
    Extension, Router,
    extract::State,
    response::Response,
    routing::get,
};
use serde::Serialize;
use zitadel_app::repo::{ActionRecord, ListParams as AppListParams};

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
    priority: i32,
    enabled: bool,
    fail_open: bool,
    metadata: serde_json::Value,
    created_at: String,
}

async fn list(State(s): State<ApiState>, Extension(identity): Extension<Identity>) -> Response {
    let ctx = response::build_actor_context(&identity);
    let params = AppListParams {
        limit: Some(200),
        cursor: None,
        search: None,
    };
    match s.app.runner.run_fn(&ctx, "action.list", || s.app.list_actions.execute(&ctx, &params)).await {
        Ok(result) => {
            let items: Vec<ActionResponse> = result.items.into_iter().map(action_from_record).collect();
            response::json_ok(serde_json::json!({ "items": items }))
        }
        Err(e) => response::app_error(e),
    }
}

async fn get_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run_fn(&ctx, "action.get", || s.app.get_action.execute(&ctx, &id)).await {
        Ok(r) => response::json_ok(action_from_record(r)),
        Err(e) => response::app_error(e),
    }
}

async fn delete_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run_fn(&ctx, "action.delete", || s.app.delete_action.execute(&ctx, &id)).await {
        Ok(()) => response::no_content(),
        Err(e) => response::app_error(e),
    }
}

fn action_from_record(r: ActionRecord) -> ActionResponse {
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
        config: r.config,
        priority: r.priority,
        enabled,
        fail_open: r.fail_open,
        metadata: r.metadata,
        created_at: r.created_at,
    }
}
