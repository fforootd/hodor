use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};

/// Standard list response with cursor pagination.
#[derive(Serialize)]
pub struct ListResponse<T: Serialize> {
    pub items: Vec<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<i64>,
}

/// Shared pagination query parameters for list endpoints.
#[derive(Deserialize)]
pub struct PaginationParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub cursor: Option<String>,
}

fn default_limit() -> i64 {
    50
}

/// Standard error response.
#[derive(Serialize)]
pub struct ErrorBody {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<u16>,
}

pub fn json_ok<T: Serialize>(data: T) -> Response {
    (StatusCode::OK, Json(data)).into_response()
}

pub fn json_created<T: Serialize>(data: T) -> Response {
    (StatusCode::CREATED, Json(data)).into_response()
}

pub fn no_content() -> Response {
    StatusCode::NO_CONTENT.into_response()
}

pub fn error(status: StatusCode, msg: impl Into<String>) -> Response {
    (
        status,
        Json(ErrorBody {
            error: msg.into(),
            code: Some(status.as_u16()),
        }),
    )
        .into_response()
}

pub fn not_found(msg: impl Into<String>) -> Response {
    error(StatusCode::NOT_FOUND, msg)
}

pub fn bad_request(msg: impl Into<String>) -> Response {
    error(StatusCode::BAD_REQUEST, msg)
}

pub fn internal_error(msg: impl Into<String>) -> Response {
    error(StatusCode::INTERNAL_SERVER_ERROR, msg)
}

/// Handle the result of an UPDATE/INSERT that should affect exactly one row.
pub fn handle_mutation(
    result: Result<sqlx::any::AnyQueryResult, sqlx::Error>,
    entity: &str,
    on_success: impl FnOnce() -> Response,
) -> Response {
    match result {
        Ok(r) if r.rows_affected() == 0 => not_found(format!("{entity} not found")),
        Ok(_) => on_success(),
        Err(e) => internal_error(format!("{e}")),
    }
}

/// Serialize a serde_json::Value to a JSON string, falling back to "{}".
pub fn to_json_string(v: &serde_json::Value) -> String {
    serde_json::to_string(v).unwrap_or_else(|_| "{}".into())
}
