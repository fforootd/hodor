use crate::OidcState;
use axum::{Json, Router, extract::State, response::IntoResponse, routing::get};

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(openid_configuration),
        )
        .with_state(state)
}

async fn openid_configuration(State(state): State<OidcState>) -> impl IntoResponse {
    Json(state.provider.discovery_document())
}
