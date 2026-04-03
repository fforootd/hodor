use crate::{OidcState, op::AuthorizeRequest, protocol_error_response};
use axum::{
    Router,
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use serde::Deserialize;

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .route("/authorize", get(authorize))
        .with_state(state)
}

#[derive(Deserialize)]
pub struct AuthorizeParams {
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub response_type: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub prompt: Option<String>,
    pub login_hint: Option<String>,
}

async fn authorize(
    State(oidc): State<OidcState>,
    Query(params): Query<AuthorizeParams>,
) -> Response {
    let request = AuthorizeRequest {
        client_id: params.client_id.unwrap_or_default(),
        redirect_uri: params.redirect_uri.unwrap_or_default(),
        response_type: params.response_type.unwrap_or_else(|| "code".to_string()),
        scope: params.scope.unwrap_or_else(|| "openid".to_string()),
        state: params.state.unwrap_or_default(),
        nonce: params.nonce.unwrap_or_default(),
        code_challenge: params.code_challenge.unwrap_or_default(),
        code_challenge_method: params.code_challenge_method.unwrap_or_default(),
        prompt: params
            .prompt
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect(),
        login_hint: params.login_hint.unwrap_or_default(),
    };

    match oidc.provider.authorize(&request).await {
        Ok(redirect) => Redirect::temporary(&redirect.location).into_response(),
        Err(error) => protocol_error_response(error),
    }
}
