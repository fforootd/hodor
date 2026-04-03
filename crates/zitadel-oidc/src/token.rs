use crate::{
    OidcState,
    op::{TokenExchangeRequest, resolve_client_auth},
    protocol_error_response,
};
use axum::{
    Form, Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::post,
};
use serde::Deserialize;

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .route("/oauth/token", post(token_endpoint))
        .route("/revoke", post(revoke_endpoint))
        .with_state(state)
}

#[derive(Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub redirect_uri: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub code_verifier: String,
    #[serde(default)]
    pub refresh_token: String,
}

async fn token_endpoint(
    State(state): State<OidcState>,
    headers: HeaderMap,
    Form(req): Form<TokenRequest>,
) -> Response {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let client_auth = match resolve_client_auth(authorization, &req.client_id, &req.client_secret) {
        Ok(auth) => auth,
        Err(error) => return protocol_error_response(error),
    };

    match state
        .provider
        .token(&TokenExchangeRequest {
            grant_type: req.grant_type,
            code: req.code,
            redirect_uri: req.redirect_uri,
            client_auth,
            code_verifier: req.code_verifier,
            refresh_token: req.refresh_token,
        })
        .await
    {
        Ok(token) => Json(token).into_response(),
        Err(error) => protocol_error_response(error),
    }
}

#[derive(Deserialize)]
pub struct RevokeRequest {
    pub token: String,
}

async fn revoke_endpoint(
    State(_state): State<OidcState>,
    Form(req): Form<RevokeRequest>,
) -> Response {
    let _ = req.token;
    StatusCode::OK.into_response()
}
