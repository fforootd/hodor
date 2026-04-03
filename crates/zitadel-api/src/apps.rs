use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/apps", get(list).post(create))
        .route("/apps/{id}", get(get_one).patch(update).delete(delete_one))
}

#[derive(Deserialize)]
pub struct CreateRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Serialize)]
pub struct ItemResponse {
    pub id: String,
    pub name: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<(String, String, String, String, String)> for ItemResponse {
    fn from(r: (String, String, String, String, String)) -> Self {
        Self {
            id: r.0,
            name: r.1,
            state: r.2,
            created_at: r.3,
            updated_at: r.4,
        }
    }
}

async fn create(State(s): State<ApiState>, Json(req): Json<CreateRequest>) -> Response {
    if req.name.is_empty() {
        return response::bad_request("name is required");
    }
    let scoped = s.db.scoped_default();
    let id = Uuid::new_v4().to_string();
    match sqlx::query(
        "INSERT INTO apps (id, instance_id, name, state) VALUES ($1, $2, $3, 'active')",
    )
    .bind(&id)
    .bind(scoped.instance_id())
    .bind(&req.name)
    .execute(scoped.pool())
    .await
    {
        Ok(_) => response::json_created(ItemResponse {
            id,
            name: req.name,
            state: "active".into(),
            created_at: String::new(),
            updated_at: String::new(),
        }),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    let (created_at, updated_at) = scoped.select_timestamps();
    let sql = format!(
        "SELECT id, name, state, {created_at}, {updated_at} FROM apps WHERE instance_id = $1 AND id = $2"
    );
    match sqlx::query_as::<_, (String, String, String, String, String)>(&sql)
        .bind(scoped.instance_id())
        .bind(&id)
        .fetch_optional(scoped.pool())
        .await
    {
        Ok(Some(r)) => response::json_ok(ItemResponse::from(r)),
        Ok(None) => response::not_found("not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn list(
    State(s): State<ApiState>,
    Query(p): Query<response::PaginationParams>,
) -> Response {
    let scoped = s.db.scoped_default();
    let cursor = p.cursor.unwrap_or_default();
    let (created_at, updated_at) = scoped.select_timestamps();
    let sql = format!(
        "SELECT id, name, state, {created_at}, {updated_at} FROM apps WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
    );
    match sqlx::query_as::<_, (String, String, String, String, String)>(&sql)
        .bind(scoped.instance_id())
        .bind(&cursor)
        .bind(p.limit.min(200))
        .fetch_all(scoped.pool())
        .await
    {
        Ok(rows) => {
            let items: Vec<ItemResponse> = rows.into_iter().map(ItemResponse::from).collect();
            response::json_ok(response::ListResponse {
                items,
                next_cursor: None,
                total: None,
            })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn update(
    State(s): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<CreateRequest>,
) -> Response {
    let scoped = s.db.scoped_default();
    if req.name.is_empty() {
        return response::bad_request("name required");
    }
    let result = sqlx::query("UPDATE apps SET name = $1, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $2 AND id = $3")
        .bind(&req.name).bind(scoped.instance_id()).bind(&id).execute(scoped.pool()).await;
    response::handle_mutation(result, "app", || {
        response::json_ok(serde_json::json!({"id": id, "updated": true}))
    })
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    response::delete_by_id(&s.db.scoped_default(), "apps", &id, "app").await
}
