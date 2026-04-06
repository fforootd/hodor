use crate::{OidcState, protocol_error_response};
use axum::{
    Json, Router,
    body::to_bytes,
    extract::State,
    http::{Method, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use std::borrow::Cow;

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .route("/userinfo", get(userinfo).post(userinfo))
        .with_state(state)
}

async fn userinfo(State(state): State<OidcState>, req: axum::extract::Request) -> Response {
    let method = req.method().clone();
    let headers = req.headers().clone();
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_string);

    let body_token = if method == Method::POST {
        match to_bytes(req.into_body(), 16 * 1024).await {
            Ok(body) => extract_form_access_token(std::str::from_utf8(&body).unwrap_or_default()),
            Err(_) => None,
        }
    } else {
        None
    };
    let token = match auth_header.or(body_token) {
        Some(token) if !token.is_empty() => token,
        _ => return (StatusCode::UNAUTHORIZED, "Bearer token required").into_response(),
    };

    match state.provider.userinfo(&token).await {
        Ok(info) => Json::<crate::oidc::UserInfoResponse>(info).into_response(),
        Err(error) => protocol_error_response(error),
    }
}

fn extract_form_access_token(input: &str) -> Option<String> {
    url::form_urlencoded::parse(input.as_bytes()).find_map(|(key, value)| {
        if key == Cow::Borrowed("access_token") && !value.is_empty() {
            Some(value.into_owned())
        } else {
            None
        }
    })
}
