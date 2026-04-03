use crate::{ApiState, response};
use axum::{
    Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use serde::Serialize;

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
    let scoped = s.db.scoped_default();
    let config = scoped.as_text("config");
    let metadata = scoped.as_text("metadata");
    let created_at = scoped.as_text("created_at");
    let enabled = scoped.bool_as_int("enabled");
    let fail_open = scoped.bool_as_int("fail_open");
    let sql = format!(
        "SELECT id, name, hook, action_type, COALESCE(trigger_expr, 'true'), \
         COALESCE({config}, '{{}}'), priority, {enabled}, {fail_open}, \
         COALESCE({metadata}, '{{}}'), {created_at} \
         FROM actions WHERE instance_id = $1 ORDER BY priority, name"
    );
    match sqlx::query_as::<_, ActionRow>(&sql)
        .bind(scoped.instance_id())
        .fetch_all(scoped.pool())
        .await
    {
        Ok(rows) => {
            let items: Vec<ActionResponse> = rows.into_iter().map(action_from_row).collect();
            response::json_ok(serde_json::json!({ "items": items }))
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    let config = scoped.as_text("config");
    let metadata = scoped.as_text("metadata");
    let created_at = scoped.as_text("created_at");
    let enabled = scoped.bool_as_int("enabled");
    let fail_open = scoped.bool_as_int("fail_open");
    let sql = format!(
        "SELECT id, name, hook, action_type, COALESCE(trigger_expr, 'true'), \
         COALESCE({config}, '{{}}'), priority, {enabled}, {fail_open}, \
         COALESCE({metadata}, '{{}}'), {created_at} \
         FROM actions WHERE instance_id = $1 AND id = $2"
    );
    match sqlx::query_as::<_, ActionRow>(&sql)
        .bind(scoped.instance_id())
        .bind(&id)
        .fetch_optional(scoped.pool())
        .await
    {
        Ok(Some(r)) => response::json_ok(action_from_row(r)),
        Ok(None) => response::not_found("action not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    response::delete_by_id(&s.db.scoped_default(), "actions", &id, "action").await
}

type ActionRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    i64,
    i64,
    i64,
    String,
    String,
);

fn action_from_row(r: ActionRow) -> ActionResponse {
    let enabled = r.7 != 0;
    let name = r.1.clone();
    ActionResponse {
        id: r.0,
        name: name.clone(),
        identifier: name.clone(),
        display_name: name,
        state: if enabled {
            "active".into()
        } else {
            "disabled".into()
        },
        hook: r.2,
        action_type: r.3,
        trigger_expr: r.4,
        config: serde_json::from_str(&r.5).unwrap_or_default(),
        priority: r.6,
        enabled,
        fail_open: r.8 != 0,
        metadata: serde_json::from_str(&r.9).unwrap_or_default(),
        created_at: r.10,
    }
}
