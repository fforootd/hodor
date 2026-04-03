use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, State},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_storage::AnalyticsQuery;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/analytics/query", post(query))
        .route("/analytics/schema", get(schema))
        .route("/analytics/queries", get(list_queries).post(create_query))
        .route("/analytics/queries/{id}", axum::routing::delete(delete_query))
}

#[derive(Deserialize)]
struct QueryRequest {
    sql: String,
    #[serde(default)]
    limit: Option<i64>,
}

/// POST /v1/analytics/query — execute a read-only SQL query and return columnar results.
async fn query(State(s): State<ApiState>, Json(req): Json<QueryRequest>) -> Response {
    match s
        .analytics
        .query(&AnalyticsQuery {
            sql: req.sql,
            limit: req.limit,
        })
        .await
    {
        Ok(result) => response::json_ok(result),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

/// GET /v1/analytics/schema — return table metadata for the schema browser.
async fn schema(State(s): State<ApiState>) -> Response {
    match s.analytics.schema().await {
        Ok(schema) => response::json_ok(schema),
        Err(e) => response::internal_error(format!("analytics schema: {e}")),
    }
}

// ─── Saved Queries ───

#[derive(Serialize)]
struct SavedQueryResponse {
    id: String,
    name: String,
    description: String,
    sql: String,
    created_at: String,
}

#[derive(Deserialize)]
struct CreateQueryRequest {
    name: String,
    #[serde(default)]
    description: String,
    sql: String,
}

/// GET /v1/analytics/queries — list saved queries.
async fn list_queries(State(s): State<ApiState>) -> Response {
    let scoped = s.db.scoped_default();
    let created_at = scoped.as_text("created_at");
    let sql = format!(
        "SELECT id, name, COALESCE(description, ''), sql_text, {created_at} \
         FROM saved_queries WHERE instance_id = $1 ORDER BY name"
    );
    match sqlx::query_as::<_, (String, String, String, String, String)>(&sql)
        .bind(scoped.instance_id())
        .fetch_all(scoped.pool())
        .await
    {
        Ok(rows) => {
            let items: Vec<SavedQueryResponse> = rows
                .into_iter()
                .map(|r| SavedQueryResponse {
                    id: r.0,
                    name: r.1,
                    description: r.2,
                    sql: r.3,
                    created_at: r.4,
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

/// POST /v1/analytics/queries — save a new query.
async fn create_query(State(s): State<ApiState>, Json(req): Json<CreateQueryRequest>) -> Response {
    if req.name.is_empty() || req.sql.is_empty() {
        return response::bad_request("name and sql are required");
    }
    let scoped = s.db.scoped_default();
    let id = format!("sq_{}", Uuid::new_v4());
    match sqlx::query(
        "INSERT INTO saved_queries (id, instance_id, name, description, sql_text) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(scoped.instance_id())
    .bind(&req.name)
    .bind(&req.description)
    .bind(&req.sql)
    .execute(scoped.pool())
    .await
    {
        Ok(_) => response::json_created(SavedQueryResponse {
            id,
            name: req.name,
            description: req.description,
            sql: req.sql,
            created_at: String::new(),
        }),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

/// DELETE /v1/analytics/queries/{id} — delete a saved query.
async fn delete_query(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    response::delete_by_id(&s.db.scoped_default(), "saved_queries", &id, "saved query").await
}
