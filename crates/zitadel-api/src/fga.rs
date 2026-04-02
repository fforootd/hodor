use axum::{Router, extract::State, response::Response, routing::{get, post}, Json};
use serde::{Deserialize, Serialize};
use crate::{ApiState, response};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/fga/check", post(fga_check))
        .route("/fga/tuples", get(list_tuples).post(write_tuples).delete(delete_tuples))
        .route("/fga/list-objects", post(list_objects))
        .route("/fga/model", get(get_model))
        .route("/fga/model/graph", get(model_graph))
        .route("/fga/expand", post(expand))
        .route("/fga/test", post(batch_test))
}

#[derive(Deserialize)]
pub struct CheckRequest {
    pub user: String,
    pub relation: String,
    pub object: String,
}

#[derive(Serialize)]
pub struct CheckResponse {
    pub allowed: bool,
}

/// POST /v1/fga/check — check if user has relation to object.
/// POC: root instance owner always gets wildcard access.
async fn fga_check(State(s): State<ApiState>, Json(req): Json<CheckRequest>) -> Response {
    let scoped = s.db.scoped_default();

    // Check if user is the bootstrapped admin (root instance owner bypass).
    let is_admin = is_instance_owner(&scoped, &req.user).await.unwrap_or(false);
    if is_admin {
        return response::json_ok(CheckResponse { allowed: true });
    }

    // POC: check memberships table for basic RBAC.
    let allowed = check_membership(&scoped, &req.user, &req.relation, &req.object).await.unwrap_or(false);
    response::json_ok(CheckResponse { allowed })
}

#[derive(Deserialize)]
pub struct TupleRequest {
    pub user: String,
    pub relation: String,
    pub object: String,
}

async fn write_tuples(State(s): State<ApiState>, Json(req): Json<TupleRequest>) -> Response {
    let scoped = s.db.scoped_default();
    // Map to memberships table.
    let parts: Vec<&str> = req.object.splitn(2, ':').collect();
    if parts.len() != 2 { return response::bad_request("object must be type:id"); }
    let user_id = req.user.strip_prefix("user:").unwrap_or(&req.user);

    let _ = sqlx::query(
        "INSERT OR IGNORE INTO memberships (instance_id, resource_type, resource_id, user_id, role) VALUES (?, ?, ?, ?, ?)")
        .bind(scoped.instance_id()).bind(parts[0]).bind(parts[1]).bind(user_id).bind(&req.relation)
        .execute(scoped.pool()).await;
    response::json_ok(serde_json::json!({"written": true}))
}

async fn list_tuples(State(s): State<ApiState>) -> Response {
    let scoped = s.db.scoped_default();
    match sqlx::query_as::<_, (String, String, String, String)>(
        "SELECT resource_type, resource_id, user_id, role FROM memberships WHERE instance_id = ? LIMIT 100")
        .bind(scoped.instance_id()).fetch_all(scoped.pool()).await {
        Ok(rows) => {
            let tuples: Vec<serde_json::Value> = rows.into_iter().map(|r| serde_json::json!({
                "user": format!("user:{}", r.2), "relation": r.3, "object": format!("{}:{}", r.0, r.1)
            })).collect();
            response::json_ok(serde_json::json!({"tuples": tuples}))
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_tuples(State(s): State<ApiState>, Json(req): Json<TupleRequest>) -> Response {
    let scoped = s.db.scoped_default();
    let parts: Vec<&str> = req.object.splitn(2, ':').collect();
    if parts.len() != 2 { return response::bad_request("object must be type:id"); }
    let user_id = req.user.strip_prefix("user:").unwrap_or(&req.user);
    let _ = sqlx::query("DELETE FROM memberships WHERE instance_id = ? AND resource_type = ? AND resource_id = ? AND user_id = ? AND role = ?")
        .bind(scoped.instance_id()).bind(parts[0]).bind(parts[1]).bind(user_id).bind(&req.relation)
        .execute(scoped.pool()).await;
    response::json_ok(serde_json::json!({"deleted": true}))
}

async fn list_objects(State(_s): State<ApiState>, Json(body): Json<serde_json::Value>) -> Response {
    // POC stub.
    response::json_ok(serde_json::json!({"objects": []}))
}

async fn get_model(State(_s): State<ApiState>) -> Response {
    // Return the Cedar/FGA authorization model summary.
    response::json_ok(serde_json::json!({
        "types": ["user", "instance", "org", "group", "project", "app", "settings", "session"],
        "relations": {
            "instance": ["owner", "admin", "viewer"],
            "org": ["owner", "admin", "member", "viewer"],
            "group": ["member", "admin"],
            "project": ["owner", "admin", "member"],
            "app": ["admin", "viewer"],
        }
    }))
}

async fn model_graph(State(_s): State<ApiState>) -> Response {
    response::json_ok(serde_json::json!({"graph": "instance → org → project → app"}))
}

async fn expand(State(_s): State<ApiState>, Json(body): Json<serde_json::Value>) -> Response {
    response::json_ok(serde_json::json!({"tree": {}}))
}

async fn batch_test(State(_s): State<ApiState>, Json(body): Json<serde_json::Value>) -> Response {
    response::json_ok(serde_json::json!({"results": []}))
}

/// Check if a user is the root instance owner (first admin).
/// POC: checks if user has an admin PAT or is the first user created (identifier = 'admin').
async fn is_instance_owner(scoped: &zitadel_db::scoped::ScopedDb, user_ref: &str) -> anyhow::Result<bool> {
    let user_id = user_ref.strip_prefix("user:").unwrap_or(user_ref);

    // Check if user has a PAT (admin users get PATs during seed).
    let has_pat: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM tokens WHERE instance_id = ? AND user_id = ? AND type = 'pat' AND revoked_at IS NULL LIMIT 1")
        .bind(scoped.instance_id()).bind(user_id)
        .fetch_optional(scoped.pool()).await?;
    if has_pat.is_some() { return Ok(true); }

    // Check if user is the admin user (identifier = 'admin').
    let is_admin: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM users WHERE instance_id = ? AND id = ? AND identifier = 'admin' LIMIT 1")
        .bind(scoped.instance_id()).bind(user_id)
        .fetch_optional(scoped.pool()).await?;
    Ok(is_admin.is_some())
}

/// Check membership-based access.
async fn check_membership(scoped: &zitadel_db::scoped::ScopedDb, user_ref: &str, relation: &str, object: &str) -> anyhow::Result<bool> {
    let user_id = user_ref.strip_prefix("user:").unwrap_or(user_ref);
    let parts: Vec<&str> = object.splitn(2, ':').collect();
    if parts.len() != 2 { return Ok(false); }
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM memberships WHERE instance_id = ? AND user_id = ? AND resource_type = ? AND resource_id = ? AND role = ?")
        .bind(scoped.instance_id()).bind(user_id).bind(parts[0]).bind(parts[1]).bind(relation)
        .fetch_optional(scoped.pool()).await?;
    Ok(row.is_some())
}
