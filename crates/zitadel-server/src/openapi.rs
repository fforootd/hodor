use std::sync::Arc;

use axum::{Router, extract::State, response::Response, routing::get};

use crate::AppState;

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/openapi.json", get(openapi_json))
        .with_state(state)
}

async fn openapi_json(State(state): State<Arc<AppState>>) -> Response {
    let public_origin = if !state.config.server.public_origin.is_empty() {
        state
            .config
            .server
            .public_origin
            .trim_end_matches('/')
            .to_string()
    } else {
        format!(
            "http://{}:{}",
            state.config.server.external_domain, state.config.server.port
        )
    };

    match zitadel_api::openapi::document(&state.db, &public_origin).await {
        Ok(document) => zitadel_api::response::json_ok(document),
        Err(error) => zitadel_api::response::internal_error(format!("openapi export: {error}")),
    }
}
