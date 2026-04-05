use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_db::{
    create_named_resource, current_instance_id, delete_instance_row, get_named_resource,
    list_named_resources, update_named_resource_name,
};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/projects", get(list).post(create))
        .route(
            "/projects/{id}",
            get(get_one).patch(update).delete(delete_one),
        )
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

impl From<zitadel_db::NamedResourceRecord> for ItemResponse {
    fn from(r: zitadel_db::NamedResourceRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            state: r.state,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

async fn create(State(s): State<ApiState>, Json(req): Json<CreateRequest>) -> Response {
    if req.name.is_empty() {
        return response::bad_request("name is required");
    }
    let id = Uuid::new_v4().to_string();
    match create_named_resource(&s.db, current_instance_id().as_ref(), "projects", &id, &req.name)
        .await
    {
        Ok(record) => response::json_created(ItemResponse::from(record)),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match get_named_resource(&s.db, current_instance_id().as_ref(), "projects", &id).await {
        Ok(Some(r)) => response::json_ok(ItemResponse::from(r)),
        Ok(None) => response::not_found("not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn list(State(s): State<ApiState>, Query(p): Query<response::PaginationParams>) -> Response {
    let cursor = p.cursor.unwrap_or_default();
    match list_named_resources(
        &s.db,
        current_instance_id().as_ref(),
        "projects",
        &cursor,
        p.limit.min(200),
    )
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
    if req.name.is_empty() {
        return response::bad_request("name required");
    }
    match update_named_resource_name(
        &s.db,
        current_instance_id().as_ref(),
        "projects",
        &id,
        &req.name,
    )
    .await
    {
        Ok(true) => response::json_ok(serde_json::json!({"id": id, "updated": true})),
        Ok(false) => response::not_found("project not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match delete_instance_row(&s.db, current_instance_id().as_ref(), "projects", &id).await {
        Ok(true) => response::no_content(),
        Ok(false) => response::not_found("project not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}
