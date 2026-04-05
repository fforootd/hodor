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
    create_org, current_instance_id, delete_instance_row, get_org, list_org_records, update_org,
};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/orgs", get(list).post(create))
        .route("/orgs/{id}", get(get_one).patch(update).delete(delete_one))
}

#[derive(Deserialize)]
pub struct OrgRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Serialize)]
pub struct OrgResponse {
    pub id: String,
    pub name: String,
    pub state: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl From<zitadel_db::OrgRecord> for OrgResponse {
    fn from(record: zitadel_db::OrgRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            state: record.state,
            metadata: serde_json::from_str(&record.metadata_json).unwrap_or_default(),
            created_at: record.created_at,
            updated_at: record.updated_at,
        }
    }
}

async fn create(State(s): State<ApiState>, Json(req): Json<OrgRequest>) -> Response {
    if req.name.is_empty() {
        return response::bad_request("name is required");
    }
    let id = Uuid::new_v4().to_string();
    let meta = response::to_json_string(&req.metadata);
    match create_org(&s.db, current_instance_id().as_ref(), &id, &req.name, &meta).await {
        Ok(record) => response::json_created(OrgResponse::from(record)),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

async fn get_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match get_org(&s.db, current_instance_id().as_ref(), &id).await {
        Ok(Some(o)) => response::json_ok(OrgResponse::from(o)),
        Ok(None) => response::not_found("org not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn list(State(s): State<ApiState>, Query(p): Query<response::PaginationParams>) -> Response {
    let cursor = p.cursor.unwrap_or_default();
    match list_org_records(
        &s.db,
        current_instance_id().as_ref(),
        &cursor,
        p.limit.min(200) + 1,
    )
    .await
    {
        Ok(rows) => {
            let limit = p.limit.min(200);
            let has_more = rows.len() as i64 > limit;
            let items: Vec<OrgResponse> = rows
                .into_iter()
                .take(limit as usize)
                .map(OrgResponse::from)
                .collect();
            let next_cursor = if has_more {
                items.last().map(|o| o.id.clone())
            } else {
                None
            };
            response::json_ok(response::ListResponse {
                items,
                next_cursor,
                total: None,
            })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn update(
    State(s): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<OrgRequest>,
) -> Response {
    match update_org(
        &s.db,
        current_instance_id().as_ref(),
        &id,
        (!req.name.is_empty()).then_some(req.name.as_str()),
        (!req.state.is_empty()).then_some(req.state.as_str()),
    )
    .await
    {
        Ok(false) => response::not_found("org not found"),
        Ok(true) => match get_org(&s.db, current_instance_id().as_ref(), &id).await {
            Ok(Some(o)) => response::json_ok(OrgResponse::from(o)),
            _ => response::not_found("org not found"),
        },
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match delete_instance_row(&s.db, current_instance_id().as_ref(), "orgs", &id).await {
        Ok(true) => response::no_content(),
        Ok(false) => response::not_found("org not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}
