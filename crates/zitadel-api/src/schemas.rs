use crate::{ApiState, extractors::ResourceId, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_app::{
    repo::{ListParams as AppListParams, SchemaRecord},
    schemas::{RegisterSchemaCommand, UpdateSchemaCommand},
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
        .route("/schemas/{id}/diff", get(schema_diff))
        .route("/schemas/{id}/preview", axum::routing::post(schema_preview))
}

/// GET /v1/schemas/$meta -- returns the full meta-schema catalog.
async fn get_meta_schema() -> Response {
    match serde_json::from_str::<serde_json::Value>(META_SCHEMA) {
        Ok(v) => response::json_ok(v),
        Err(e) => response::internal_error(format!("parse meta-schema: {e}")),
    }
}

#[derive(Deserialize)]
pub struct SchemaListParams {
    #[serde(default = "default_limit")]
    pub limit: u32,
    pub cursor: Option<String>,
    #[serde(rename = "type")]
    pub type_filter: Option<String>,
}
fn default_limit() -> u32 {
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

impl SchemaResponse {
    fn from_record(r: SchemaRecord, include_schema: bool) -> Self {
        Self {
            id: r.id,
            type_: r.schema_type,
            version: r.version,
            is_default: r.is_default,
            visibility: r.visibility,
            created_at: r.created_at,
            schema: if include_schema {
                Some(r.schema_json)
            } else {
                None
            },
        }
    }
}

async fn list_schemas(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(p): Query<SchemaListParams>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let params = AppListParams {
        limit: Some(p.limit.min(200)),
        cursor: p.cursor,
        search: p.type_filter,
    };
    match s.app.runner.run_fn(&ctx, "schema.list", || s.app.list_schemas.execute(&ctx, &params)).await {
        Ok(result) => {
            let items: Vec<SchemaResponse> = result
                .items
                .into_iter()
                .map(|r| SchemaResponse::from_record(r, false))
                .collect();
            response::json_ok(response::ListResponse {
                items,
                next_cursor: result.next_cursor,
                total: result.total_count.map(|c| c as i64),
            })
        }
        Err(e) => response::app_error(e),
    }
}

async fn get_schema(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run_fn(&ctx, "schema.get", || s.app.get_schema.execute(&ctx, &id)).await {
        Ok(schema) => response::json_ok(SchemaResponse::from_record(schema, true)),
        Err(e) => response::app_error(e),
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
    Extension(identity): Extension<Identity>,
    Json(req): Json<CreateSchemaRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let vis = if req.visibility.is_empty() {
        "private".to_string()
    } else {
        req.visibility
    };
    let cmd = RegisterSchemaCommand {
        schema_type: req.type_,
        schema_json: req.schema,
        visibility: vis,
    };
    match s.app.runner.run_fn(&ctx, "schema.register", || s.app.register_schema.execute(&ctx, cmd)).await {
        Ok(schema) => response::json_created(SchemaResponse::from_record(schema, true)),
        Err(e) => response::app_error(e),
    }
}

async fn update_schema(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
    Json(req): Json<CreateSchemaRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = UpdateSchemaCommand {
        schema_id: id,
        schema_json: req.schema,
    };
    match s.app.runner.run_fn(&ctx, "schema.update", || s.app.update_schema.execute(&ctx, cmd)).await {
        Ok(schema) => response::json_ok(SchemaResponse::from_record(schema, false)),
        Err(e) => response::app_error(e),
    }
}

async fn promote_schema(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run_fn(&ctx, "schema.promote", || s.app.promote_schema.execute(&ctx, &id)).await {
        Ok(true) => response::json_ok(serde_json::json!({"id": id, "promoted": true})),
        Ok(false) => response::not_found("schema not found"),
        Err(e) => response::app_error(e),
    }
}

async fn schema_identity_count(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run_fn(&ctx, "schema.count_users", || s.app.count_schema_users.execute(&ctx, &id)).await {
        Ok(count) => response::json_ok(serde_json::json!({"count": count})),
        Err(e) => response::app_error(e),
    }
}

// ─── Diff ───

#[derive(Deserialize)]
struct DiffParams {
    compare: Option<String>,
}

async fn schema_diff(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
    Query(params): Query<DiffParams>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let left = match s.app.get_schema.execute(&ctx, &id).await {
        Ok(schema) => schema,
        Err(e) => return response::app_error(e),
    };

    let right = match &params.compare {
        Some(compare_id) => match s.app.get_schema.execute(&ctx, compare_id).await {
            Ok(schema) => Some(schema),
            Err(e) => return response::app_error(e),
        },
        None => None,
    };

    response::json_ok(serde_json::json!({
        "left": left.schema_json,
        "right": right.map(|r| r.schema_json),
        "changes": [],
    }))
}

// ─── Preview ───

#[derive(Deserialize)]
struct PreviewRequest {
    entity_id: String,
}

async fn schema_preview(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
    Json(req): Json<PreviewRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let schema = match s.app.get_schema.execute(&ctx, &id).await {
        Ok(schema) => schema,
        Err(e) => return response::app_error(e),
    };

    // Load the entity to preview the schema against.
    let entity = match s.app.get_user.execute(&ctx, &req.entity_id).await {
        Ok(user) => user,
        Err(e) => return response::app_error(e),
    };

    response::json_ok(serde_json::json!({
        "entity": entity.id,
        "current_claims": entity.metadata,
        "draft_claims": schema.schema_json,
        "changes": [],
    }))
}
