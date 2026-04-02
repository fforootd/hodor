use axum::{Router, extract::State, response::IntoResponse, routing::get, Json};
use serde::Serialize;
use crate::OidcState;

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .route("/.well-known/openid-configuration", get(openid_configuration))
        .with_state(state)
}

#[derive(Serialize)]
struct OpenIDConfiguration {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    userinfo_endpoint: String,
    jwks_uri: String,
    revocation_endpoint: String,
    end_session_endpoint: String,
    response_types_supported: Vec<String>,
    grant_types_supported: Vec<String>,
    subject_types_supported: Vec<String>,
    id_token_signing_alg_values_supported: Vec<String>,
    scopes_supported: Vec<String>,
    token_endpoint_auth_methods_supported: Vec<String>,
    code_challenge_methods_supported: Vec<String>,
    claims_supported: Vec<String>,
}

async fn openid_configuration(State(state): State<OidcState>) -> impl IntoResponse {
    let iss = &state.issuer;
    Json(OpenIDConfiguration {
        issuer: iss.clone(),
        authorization_endpoint: format!("{iss}/authorize"),
        token_endpoint: format!("{iss}/oauth/token"),
        userinfo_endpoint: format!("{iss}/userinfo"),
        jwks_uri: format!("{iss}/keys"),
        revocation_endpoint: format!("{iss}/revoke"),
        end_session_endpoint: format!("{iss}/end_session"),
        response_types_supported: vec!["code".into()],
        grant_types_supported: vec!["authorization_code".into(), "refresh_token".into(), "client_credentials".into()],
        subject_types_supported: vec!["public".into()],
        id_token_signing_alg_values_supported: vec!["RS256".into()],
        scopes_supported: vec!["openid".into(), "profile".into(), "email".into(), "offline_access".into()],
        token_endpoint_auth_methods_supported: vec!["client_secret_post".into(), "client_secret_basic".into()],
        code_challenge_methods_supported: vec!["S256".into()],
        claims_supported: vec!["sub".into(), "iss".into(), "aud".into(), "exp".into(), "iat".into(), "name".into(), "email".into(), "locale".into()],
    })
}
