use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Path, State},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_db::{create_saved_query, delete_saved_query, list_saved_queries};
use zitadel_storage::AnalyticsQuery;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/analytics/query", post(query))
        .route("/analytics/schema", get(schema))
        .route("/analytics/queries", get(list_queries).post(create_query))
        .route(
            "/analytics/queries/{id}",
            axum::routing::delete(
                |state: State<ApiState>, path: Path<String>| async move {
                    delete_query(state, path).await
                },
            ),
        )
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
    match list_saved_queries(&s.db, zitadel_db::current_instance_id().as_ref()).await {
        Ok(rows) => {
            let items: Vec<SavedQueryResponse> = rows
                .into_iter()
                .map(|row| SavedQueryResponse {
                    id: row.id,
                    name: row.name,
                    description: row.description,
                    sql: row.sql,
                    created_at: row.created_at,
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
    let id = format!("sq_{}", Uuid::new_v4());
    match create_saved_query(
        &s.db,
        zitadel_db::current_instance_id().as_ref(),
        &id,
        &req.name,
        &req.description,
        &req.sql,
    )
    .await
    {
        Ok(row) => response::json_created(SavedQueryResponse {
            id: row.id,
            name: row.name,
            description: row.description,
            sql: row.sql,
            created_at: row.created_at,
        }),
        Err(e) => response::bad_request(format!("{e}")),
    }
}

/// DELETE /v1/analytics/queries/{id} — delete a saved query.
async fn delete_query(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    match delete_saved_query(&s.db, zitadel_db::current_instance_id().as_ref(), &id).await {
        Ok(true) => response::no_content(),
        Ok(false) => response::not_found("saved query not found"),
        Err(error) => response::internal_error(format!("{error}")),
    }
}
