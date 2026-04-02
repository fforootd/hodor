use axum::{Router, extract::{Query, State}, http::StatusCode, response::{IntoResponse, Redirect, Response}, routing::get};
use serde::Deserialize;
use uuid::Uuid;
use crate::OidcState;

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
}

async fn authorize(
    State(oidc): State<OidcState>,
    Query(params): Query<AuthorizeParams>,
) -> Response {
    let client_id = match &params.client_id {
        Some(id) => id.clone(),
        None => return (StatusCode::BAD_REQUEST, "client_id required").into_response(),
    };
    let redirect_uri = match &params.redirect_uri {
        Some(uri) => uri.clone(),
        None => return (StatusCode::BAD_REQUEST, "redirect_uri required").into_response(),
    };

    // Validate client exists.
    let scoped = oidc.db.scoped_default();
    let client_exists: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM apps WHERE instance_id = ? AND client_id = ?",
    )
    .bind(scoped.instance_id())
    .bind(&client_id)
    .fetch_optional(scoped.pool())
    .await
    .unwrap_or(None);

    if client_exists.is_none() {
        return (StatusCode::BAD_REQUEST, "unknown client_id").into_response();
    }

    // Store auth request state.
    let auth_id = Uuid::new_v4().to_string();
    let state_param = params.state.unwrap_or_default();
    let nonce = params.nonce.unwrap_or_default();
    let code_challenge = params.code_challenge.unwrap_or_default();
    let code_challenge_method = params.code_challenge_method.unwrap_or_default();
    let scopes = params.scope.unwrap_or_else(|| "openid".into());

    let _ = sqlx::query(
        "INSERT INTO auth_states (id, instance_id, type, client_id, redirect_uri, scopes, state, nonce, response_type, code_challenge, code_challenge_method) \
         VALUES (?, ?, 'oidc_auth', ?, ?, ?, ?, ?, 'code', ?, ?)",
    )
    .bind(&auth_id)
    .bind(scoped.instance_id())
    .bind(&client_id)
    .bind(&redirect_uri)
    .bind(&scopes)
    .bind(&state_param)
    .bind(&nonce)
    .bind(&code_challenge)
    .bind(&code_challenge_method)
    .execute(scoped.pool())
    .await;

    // Redirect to login with auth_request_id.
    let login_url = format!("/login?auth_request_id={auth_id}");
    Redirect::temporary(&login_url).into_response()
}
