use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use zitadel_app::AppError;

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
    #[serde(default = "default_limit", deserialize_with = "clamp_limit")]
    pub limit: i64,
    pub cursor: Option<String>,
}

fn default_limit() -> i64 {
    50
}

fn clamp_limit<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<i64, D::Error> {
    let raw = i64::deserialize(deserializer)?;
    Ok(raw.clamp(1, 500))
}

/// Maximum length for user-supplied name fields (255 UTF-8 chars).
pub const MAX_NAME_LENGTH: usize = 255;

/// Validate that a name is non-empty and within length limits.
/// Returns a 400 Response on failure, Ok(()) on success.
pub fn validate_name(field: &str, value: &str) -> Result<(), Response> {
    if value.trim().is_empty() {
        return Err(bad_request(format!("{field} is required")));
    }
    if value.len() > MAX_NAME_LENGTH {
        return Err(bad_request(format!(
            "{field} exceeds maximum length of {MAX_NAME_LENGTH}"
        )));
    }
    Ok(())
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

pub fn forbidden(msg: impl Into<String>) -> Response {
    error(StatusCode::FORBIDDEN, msg)
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

/// Map an `AppError` to an HTTP response.
pub fn app_error(e: AppError) -> Response {
    let status = StatusCode::from_u16(e.status_code()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    error(status, e.to_string())
}

/// Build an `ActorContext` from middleware-provided identity and the current instance ID.
pub fn build_actor_context(identity: &crate::middleware::Identity) -> zitadel_app::ActorContext {
    let instance_id = zitadel_db::current_instance_id();
    let capabilities = if identity.operator_admin {
        vec![zitadel_app::Capability::OperatorAdmin]
    } else {
        vec![]
    };
    zitadel_app::ActorContext {
        auth: zitadel_app::AuthContext {
            identity: zitadel_app::Identity {
                user_id: identity.user_id.clone(),
                session_id: identity.session_id.clone(),
                token_type: identity.token_type.clone(),
                org_id: identity.org_id.clone(),
            },
            capabilities,
        },
        instance: zitadel_app::InstanceContext {
            instance_id: instance_id.into_owned(),
            placement_mode: String::new(),
            region_key: None,
            feature_overrides: std::collections::HashMap::new(),
            host: String::new(),
        },
    }
}
