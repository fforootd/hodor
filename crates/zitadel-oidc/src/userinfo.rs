use crate::{OidcState, protocol_error_response};
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .route("/userinfo", get(userinfo))
        .with_state(state)
}

async fn userinfo(State(state): State<OidcState>, req: axum::extract::Request) -> Response {
    let auth = match req.headers().get(header::AUTHORIZATION) {
        Some(v) => v.to_str().unwrap_or("").to_string(),
        None => return (StatusCode::UNAUTHORIZED, "Bearer token required").into_response(),
    };
    let token = match auth.strip_prefix("Bearer ") {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, "Bearer token required").into_response(),
    };

    match state.provider.userinfo(token).await {
        Ok(info) => Json(info).into_response(),
        Err(error) => protocol_error_response(error),
    }
}
