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
    count_users_for_schema, create_schema_record, get_schema_record, list_schema_registry,
    promote_schema_record, update_schema_record,
};

/// Embedded meta-schema JSON (the console's source of truth for navigation + types).
const META_SCHEMA: &str = include_str!("meta_schema.json");

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/schemas/$meta", get(get_meta_schema))
        .route("/schemas", get(list_schemas).post(create_schema))
        .route("/schemas/{id}", get(get_schema).patch(update_schema))
        .route("/schemas/{id}/promote", axum::routing::post(promote_schema))
        .route("/schemas/{id}/identity-count", get(schema_identity_count))
}

/// GET /v1/schemas/$meta — returns the full meta-schema catalog.
/// This is the FIRST call the console makes to build its sidebar.
async fn get_meta_schema() -> Response {
    // Parse and return the embedded meta-schema.
    match serde_json::from_str::<serde_json::Value>(META_SCHEMA) {
        Ok(v) => response::json_ok(v),
        Err(e) => response::internal_error(format!("parse meta-schema: {e}")),
    }
}

#[derive(Deserialize)]
pub struct SchemaListParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub cursor: Option<String>,
    #[serde(rename = "type")]
    pub type_filter: Option<String>,
}
fn default_limit() -> i64 {
    50
}

#[derive(Serialize)]
pub struct SchemaResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub version: i64,
    pub is_default: bool,
    pub visibility: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,
}

impl From<zitadel_db::SchemaRegistryRecord> for SchemaResponse {
    fn from(record: zitadel_db::SchemaRegistryRecord) -> Self {
        Self {
            id: record.id,
            type_: record.type_,
            version: record.version,
            is_default: record.is_default,
            visibility: record.visibility,
            created_at: record.created_at,
            schema: None,
        }
    }
}

async fn list_schemas(State(s): State<ApiState>, Query(p): Query<SchemaListParams>) -> Response {
    let cursor = p.cursor.unwrap_or_default();
    match list_schema_registry(&s.db, &cursor, p.type_filter.as_deref(), p.limit.min(200)).await {
        Ok(rows) => {
            let items: Vec<SchemaResponse> = rows.into_iter().map(SchemaResponse::from).collect();
            response::json_ok(response::ListResponse {
                items,
                next_cursor: None,
                total: None,
            })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn get_schema(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match get_schema_record(&s.db, &id).await {
        Ok(Some(r)) => {
            let schema_val =
                serde_json::from_str(&r.schema_json).unwrap_or(serde_json::Value::Null);
            response::json_ok(SchemaResponse {
                id: r.id,
                type_: r.type_,
                version: r.version,
                is_default: r.is_default,
                visibility: r.visibility,
                created_at: r.created_at,
                schema: Some(schema_val),
            })
        }
        Ok(None) => response::not_found("schema not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

#[derive(Deserialize)]
pub struct CreateSchemaRequest {
    #[serde(rename = "type")]
    pub type_: String,
    pub schema: serde_json::Value,
    #[serde(default)]
    pub visibility: String,
}

async fn create_schema(
    State(s): State<ApiState>,
    Json(req): Json<CreateSchemaRequest>,
) -> Response {
    let id = Uuid::new_v4().to_string();
    let schema_str = response::to_json_string(&req.schema);
    let vis = if req.visibility.is_empty() {
        "private"
    } else {
        &req.visibility
    };
    match create_schema_record(&s.db, &id, &req.type_, &schema_str, vis).await {
        Ok(_) => response::json_created(SchemaResponse {
            id,
            type_: req.type_,
            version: 1,
            is_default: false,
            visibility: vis.to_string(),
            created_at: String::new(),
            schema: Some(req.schema),
        }),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

async fn update_schema(
    State(s): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<CreateSchemaRequest>,
) -> Response {
    let schema_str = response::to_json_string(&req.schema);
    match update_schema_record(&s.db, &id, &schema_str).await {
        Ok(false) => response::not_found("schema not found"),
        Ok(_) => response::json_ok(serde_json::json!({"id": id, "updated": true})),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn promote_schema(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match promote_schema_record(&s.db, &id).await {
        Ok(true) => response::json_ok(serde_json::json!({"id": id, "promoted": true})),
        Ok(false) => response::not_found("schema not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn schema_identity_count(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match count_users_for_schema(&s.db, zitadel_db::current_instance_id().as_ref(), &id).await {
        Ok(count) => response::json_ok(serde_json::json!({"count": count})),
        Err(e) => response::internal_error(format!("{e}")),
    }
}
