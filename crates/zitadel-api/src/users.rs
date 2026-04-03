use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{ApiState, response};

type UserListRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);
type UserDetailRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/users", get(list_users).post(create_user))
        .route(
            "/users/{id}",
            get(get_user).patch(update_user).delete(delete_user),
        )
        .route("/users/{id}/password", post(set_password))
}

#[derive(Deserialize)]
pub struct UserRequest {
    #[serde(default)]
    pub identifier: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub schema_id: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub org_id: String,
    pub identifier: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub display_name: String,
    pub user_type: String,
    pub state: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub schema_id: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Deserialize)]
pub struct ListParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub cursor: Option<String>,
    pub org_id: Option<String>,
    pub state: Option<String>,
}

fn default_limit() -> i64 {
    50
}

#[derive(Deserialize)]
pub struct PasswordRequest {
    pub password: String,
}

async fn create_user(State(state): State<ApiState>, Json(req): Json<UserRequest>) -> Response {
    if req.identifier.is_empty() {
        return response::bad_request("identifier is required");
    }

    let scoped = state.db.scoped_default();
    let id = Uuid::new_v4().to_string();

    // Get first org as default.
    let org_id: String =
        match sqlx::query_as::<_, (String,)>("SELECT id FROM orgs WHERE instance_id = $1 LIMIT 1")
            .bind(scoped.instance_id())
            .fetch_optional(scoped.pool())
            .await
        {
            Ok(Some(r)) => r.0,
            Ok(None) => return response::internal_error("no org found"),
            Err(e) => return response::internal_error(format!("db error: {e}")),
        };

    let metadata = response::to_json_string(&req.metadata);
    let display = if req.display_name.is_empty() {
        &req.identifier
    } else {
        &req.display_name
    };
    let sql = format!(
        "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, schema_id, metadata) \
         VALUES ($1, $2, $3, $4, $5, 'human', 'active', $6, {})",
        scoped.json_bind(7),
    );

    let result = sqlx::query(&sql)
        .bind(&id)
        .bind(scoped.instance_id())
        .bind(&org_id)
        .bind(&req.identifier)
        .bind(display)
        .bind(&req.schema_id)
        .bind(&metadata)
        .execute(scoped.pool())
        .await;

    match result {
        Ok(_) => match load_user(&scoped, &id).await {
            Ok(Some(u)) => response::json_created(u),
            Ok(None) => response::internal_error("user created but not found"),
            Err(e) => response::internal_error(format!("load user: {e}")),
        },
        Err(e) => response::bad_request(format!("create user: {e}")),
    }
}

async fn get_user(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = state.db.scoped_default();
    match load_user(&scoped, &id).await {
        Ok(Some(u)) => response::json_ok(u),
        Ok(None) => response::not_found("user not found"),
        Err(e) => response::internal_error(format!("load user: {e}")),
    }
}

async fn list_users(State(state): State<ApiState>, Query(params): Query<ListParams>) -> Response {
    let scoped = state.db.scoped_default();
    let limit = params.limit.min(200);
    let cursor = params.cursor.unwrap_or_default();
    let (created_at, updated_at) = scoped.select_timestamps();
    let sql = format!(
        "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, {created_at}, {updated_at} \
         FROM users WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3",
    );

    let rows: Result<Vec<UserListRow>, _> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(&cursor)
        .bind(limit + 1) // Fetch one extra for next_cursor
        .fetch_all(scoped.pool())
        .await;

    match rows {
        Ok(rows) => {
            let has_more = rows.len() as i64 > limit;
            let items: Vec<UserResponse> = rows
                .into_iter()
                .take(limit as usize)
                .map(|r| UserResponse {
                    id: r.0,
                    org_id: r.1,
                    identifier: r.2,
                    display_name: r.3,
                    user_type: r.4,
                    state: r.5,
                    schema_id: r.6,
                    metadata: serde_json::Value::Null,
                    created_at: r.7,
                    updated_at: r.8,
                })
                .collect();
            let next_cursor = if has_more {
                items.last().map(|u| u.id.clone())
            } else {
                None
            };
            response::json_ok(response::ListResponse {
                items,
                next_cursor,
                total: None,
            })
        }
        Err(e) => response::internal_error(format!("list users: {e}")),
    }
}

async fn update_user(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<UserRequest>,
) -> Response {
    let scoped = state.db.scoped_default();

    let mut sets = Vec::new();
    let mut binds: Vec<String> = Vec::new();
    let mut next = 1usize;
    if !req.display_name.is_empty() {
        sets.push(format!("display_name = {}", scoped.placeholder(next)));
        binds.push(req.display_name.clone());
        next += 1;
    }
    if !req.state.is_empty() {
        sets.push(format!("state = {}", scoped.placeholder(next)));
        binds.push(req.state.clone());
        next += 1;
    }
    if sets.is_empty() {
        return response::bad_request("no fields to update");
    }
    sets.push("updated_at = CURRENT_TIMESTAMP".into());

    let sql = format!(
        "UPDATE users SET {} WHERE instance_id = {} AND id = {}",
        sets.join(", "),
        scoped.placeholder(next),
        scoped.placeholder(next + 1),
    );

    let mut query = sqlx::query(&sql);
    for b in &binds {
        query = query.bind(b);
    }
    query = query.bind(scoped.instance_id()).bind(&id);

    match query.execute(scoped.pool()).await {
        Ok(r) if r.rows_affected() == 0 => response::not_found("user not found"),
        Ok(_) => match load_user(&scoped, &id).await {
            Ok(Some(u)) => response::json_ok(u),
            Ok(None) => response::not_found("user not found"),
            Err(e) => response::internal_error(format!("load user: {e}")),
        },
        Err(e) => response::internal_error(format!("update user: {e}")),
    }
}

async fn delete_user(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    response::delete_by_id(&state.db.scoped_default(), "users", &id, "user").await
}

async fn set_password(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<PasswordRequest>,
) -> Response {
    let scoped = state.db.scoped_default();

    // Hash password.
    let hash = match state.passwords.hash(&req.password) {
        Ok(h) => h,
        Err(e) => return response::internal_error(format!("hash password: {e}")),
    };

    let cred_json = zitadel_authn::password::encode_credential_json(&hash);
    let cred_id = Uuid::new_v4().to_string();

    // Delete existing password, insert new.
    let _ = sqlx::query(
        "DELETE FROM credentials WHERE instance_id = $1 AND user_id = $2 AND type = 'password'",
    )
    .bind(scoped.instance_id())
    .bind(&id)
    .execute(scoped.pool())
    .await;

    let sql = format!(
        "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, $2, $3, 'password', {})",
        scoped.json_bind(4),
    );
    match sqlx::query(&sql)
        .bind(&cred_id)
        .bind(scoped.instance_id())
        .bind(&id)
        .bind(&cred_json)
        .execute(scoped.pool())
        .await
    {
        Ok(_) => response::no_content(),
        Err(e) => response::internal_error(format!("set password: {e}")),
    }
}

async fn load_user(
    scoped: &zitadel_db::scoped::ScopedDb,
    id: &str,
) -> anyhow::Result<Option<UserResponse>> {
    let metadata = scoped.as_text("metadata");
    let (created_at, updated_at) = scoped.select_timestamps();
    let sql = format!(
        "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
         FROM users WHERE instance_id = $1 AND id = $2"
    );
    let row: Option<UserDetailRow> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(id)
        .fetch_optional(scoped.pool())
        .await?;

    Ok(row.map(|r| {
        let metadata = serde_json::from_str(&r.7).unwrap_or(serde_json::Value::Null);
        UserResponse {
            id: r.0,
            org_id: r.1,
            identifier: r.2,
            display_name: r.3,
            user_type: r.4,
            state: r.5,
            schema_id: r.6,
            metadata,
            created_at: r.8,
            updated_at: r.9,
        }
    }))
}
