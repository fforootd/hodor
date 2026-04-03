use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

async fn list_schemas(State(s): State<ApiState>, Query(p): Query<SchemaListParams>) -> Response {
    let scoped = s.db.scoped_default();
    let cursor = p.cursor.unwrap_or_default();

    let (sql, bind_type) = if let Some(ref t) = p.type_filter {
        (
            format!(
                "SELECT id, type, version, {}, visibility, {} FROM schemas WHERE id > $1 AND type = $2 ORDER BY type, version DESC LIMIT $3",
                scoped.bool_as_int("is_default"),
                scoped.as_text("created_at"),
            ),
            Some(t.clone()),
        )
    } else {
        (
            format!(
                "SELECT id, type, version, {}, visibility, {} FROM schemas WHERE id > $1 ORDER BY type, version DESC LIMIT $2",
                scoped.bool_as_int("is_default"),
                scoped.as_text("created_at"),
            ),
            None,
        )
    };

    let mut query =
        sqlx::query_as::<_, (String, String, i64, i64, String, String)>(&sql).bind(&cursor);
    if let Some(t) = &bind_type {
        query = query.bind(t);
    }
    query = query.bind(p.limit.min(200));

    match query.fetch_all(scoped.pool()).await {
        Ok(rows) => {
            let items: Vec<SchemaResponse> = rows
                .into_iter()
                .map(|r| SchemaResponse {
                    id: r.0,
                    type_: r.1,
                    version: r.2,
                    is_default: r.3 != 0,
                    visibility: r.4,
                    created_at: r.5,
                    schema: None,
                })
                .collect();
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
    let scoped = s.db.scoped_default();
    let sql = format!(
        "SELECT id, type, {}, version, {}, visibility, {} FROM schemas WHERE id = $1",
        scoped.as_text("schema"),
        scoped.bool_as_int("is_default"),
        scoped.as_text("created_at"),
    );
    match sqlx::query_as::<_, (String, String, String, i64, i64, String, String)>(&sql)
        .bind(&id)
        .fetch_optional(scoped.pool())
        .await
    {
        Ok(Some(r)) => {
            let schema_val = serde_json::from_str(&r.2).unwrap_or(serde_json::Value::Null);
            response::json_ok(SchemaResponse {
                id: r.0,
                type_: r.1,
                version: r.3,
                is_default: r.4 != 0,
                visibility: r.5,
                created_at: r.6,
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
    let scoped = s.db.scoped_default();
    let id = Uuid::new_v4().to_string();
    let schema_str = response::to_json_string(&req.schema);
    let vis = if req.visibility.is_empty() {
        "private"
    } else {
        &req.visibility
    };
    let sql = format!(
        "INSERT INTO schemas (id, type, schema, visibility) VALUES ($1, $2, {}, $3)",
        scoped.json_bind(4),
    );

    match sqlx::query(&sql)
        .bind(&id)
        .bind(&req.type_)
        .bind(vis)
        .bind(&schema_str)
        .execute(scoped.pool())
        .await
    {
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
    let scoped = s.db.scoped_default();
    let schema_str = response::to_json_string(&req.schema);
    let sql = format!(
        "UPDATE schemas SET schema = {}, version = version + 1 WHERE id = $1",
        scoped.json_bind(2),
    );
    match sqlx::query(&sql)
        .bind(&id)
        .bind(&schema_str)
        .execute(scoped.pool())
        .await
    {
        Ok(r) if r.rows_affected() == 0 => response::not_found("schema not found"),
        Ok(_) => response::json_ok(serde_json::json!({"id": id, "updated": true})),
        Err(e) => response::internal_error(format!("{e}")),
    }
}

async fn promote_schema(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    // Get the schema type first.
    let type_: Option<(String,)> = sqlx::query_as("SELECT type FROM schemas WHERE id = $1")
        .bind(&id)
        .fetch_optional(scoped.pool())
        .await
        .unwrap_or(None);
    let Some((type_,)) = type_ else {
        return response::not_found("schema not found");
    };

    // Unset is_default for all schemas of this type, then set for this one.
    let _ = sqlx::query("UPDATE schemas SET is_default = FALSE WHERE type = $1")
        .bind(&type_)
        .execute(scoped.pool())
        .await;
    let _ =
        sqlx::query("UPDATE schemas SET is_default = TRUE, visibility = 'public' WHERE id = $1")
            .bind(&id)
            .execute(scoped.pool())
            .await;

    response::json_ok(serde_json::json!({"id": id, "promoted": true}))
}

async fn schema_identity_count(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    let scoped = s.db.scoped_default();
    let count: i64 = sqlx::query_as::<_, (i64,)>(
        "SELECT COUNT(*) FROM users WHERE instance_id = $1 AND schema_id = $2",
    )
    .bind(scoped.instance_id())
    .bind(&id)
    .fetch_one(scoped.pool())
    .await
    .map(|r| r.0)
    .unwrap_or(0);
    response::json_ok(serde_json::json!({"count": count}))
}
