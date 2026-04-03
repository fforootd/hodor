use crate::{OidcState, protocol_error_response};
use axum::{
    Json, Router,
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
};

pub fn routes(state: OidcState) -> Router {
    Router::new().route("/keys", get(jwks)).with_state(state)
}

async fn jwks(State(state): State<OidcState>) -> Response {
    match state.provider.jwks().await {
        Ok(set) => Json(set).into_response(),
        Err(error) => protocol_error_response(error),
    }
}
