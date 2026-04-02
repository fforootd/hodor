use axum::{Router, extract::State, http::StatusCode, response::{IntoResponse, Response}, routing::post, Form, Json};
use jsonwebtoken::{Header, Algorithm};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;
use crate::OidcState;

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

#[derive(Serialize)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    id_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    refresh_token: Option<String>,
    scope: String,
}

#[derive(Serialize)]
struct IdTokenClaims {
    iss: String,
    sub: String,
    aud: String,
    exp: u64,
    iat: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    nonce: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    email: String,
}

#[derive(Serialize)]
struct AccessTokenClaims {
    iss: String,
    sub: String,
    aud: String,
    exp: u64,
    iat: u64,
    scope: String,
    client_id: String,
}

async fn token_endpoint(State(state): State<OidcState>, Form(req): Form<TokenRequest>) -> Response {
    match req.grant_type.as_str() {
        "authorization_code" => handle_auth_code(&state, &req).await,
        "client_credentials" => handle_client_credentials(&state, &req).await,
        _ => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "unsupported_grant_type"}))).into_response(),
    }
}

async fn handle_auth_code(state: &OidcState, req: &TokenRequest) -> Response {
    let scoped = state.db.scoped_default();

    // Look up auth request by code.
    let row: Option<(String, String, String, String, String, String, String)> = sqlx::query_as(
        "SELECT id, user_id, client_id, redirect_uri, scopes, nonce, code_challenge \
         FROM auth_states WHERE instance_id = ? AND code = ? AND type = 'oidc_auth' AND done = 1",
    )
    .bind(scoped.instance_id())
    .bind(&req.code)
    .fetch_optional(scoped.pool())
    .await
    .unwrap_or(None);

    let (auth_id, user_id, client_id, _redirect_uri, scopes, nonce, code_challenge) = match row {
        Some(r) => r,
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid_grant"}))).into_response(),
    };

    // Verify PKCE if code_challenge was provided.
    if !code_challenge.is_empty() {
        let verifier_hash = {
            let mut hasher = Sha256::new();
            hasher.update(req.code_verifier.as_bytes());
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, hasher.finalize())
        };
        if verifier_hash != code_challenge {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid_grant", "error_description": "PKCE verification failed"}))).into_response();
        }
    }

    // Delete used auth request.
    let _ = sqlx::query("DELETE FROM auth_states WHERE id = ?").bind(&auth_id).execute(scoped.pool()).await;

    // Load user info for claims.
    let user: Option<(String, String)> = sqlx::query_as(
        "SELECT identifier, display_name FROM users WHERE instance_id = ? AND id = ?",
    )
    .bind(scoped.instance_id())
    .bind(&user_id)
    .fetch_optional(scoped.pool())
    .await
    .unwrap_or(None);
    let (email, name) = user.unwrap_or_default();

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let guard = match state.signing_keys().await {
        Ok(g) => g,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "signing keys not ready").into_response(),
    };
    let keys = guard.as_ref().unwrap();

    // Issue ID token.
    let id_token_claims = IdTokenClaims {
        iss: state.issuer.clone(),
        sub: user_id.clone(),
        aud: client_id.clone(),
        exp: now + 3600,
        iat: now,
        nonce,
        name,
        email,
    };
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(keys.kid.clone());
    let id_token = jsonwebtoken::encode(&header, &id_token_claims, &keys.encoding)
        .unwrap_or_default();

    // Issue access token (also a JWT).
    let access_claims = AccessTokenClaims {
        iss: state.issuer.clone(),
        sub: user_id,
        aud: client_id,
        exp: now + 3600,
        iat: now,
        scope: scopes.clone(),
        client_id: req.client_id.clone(),
    };
    let access_token = jsonwebtoken::encode(&header, &access_claims, &keys.encoding)
        .unwrap_or_default();

    Json(TokenResponse {
        access_token,
        token_type: "Bearer".into(),
        expires_in: 3600,
        id_token: Some(id_token),
        refresh_token: None,
        scope: scopes,
    }).into_response()
}

async fn handle_client_credentials(state: &OidcState, req: &TokenRequest) -> Response {
    let scoped = state.db.scoped_default();

    // Validate client credentials.
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM apps WHERE instance_id = ? AND client_id = ? AND client_secret = ?",
    )
    .bind(scoped.instance_id())
    .bind(&req.client_id)
    .bind(&req.client_secret)
    .fetch_optional(scoped.pool())
    .await
    .unwrap_or(None);

    if row.is_none() {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid_client"}))).into_response();
    }

    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let guard = match state.signing_keys().await {
        Ok(g) => g,
        Err(_) => return (StatusCode::SERVICE_UNAVAILABLE, "signing keys not ready").into_response(),
    };
    let keys = guard.as_ref().unwrap();

    let claims = AccessTokenClaims {
        iss: state.issuer.clone(),
        sub: req.client_id.clone(),
        aud: req.client_id.clone(),
        exp: now + 3600,
        iat: now,
        scope: "openid".into(),
        client_id: req.client_id.clone(),
    };
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(keys.kid.clone());
    let token = jsonwebtoken::encode(&header, &claims, &keys.encoding).unwrap_or_default();

    Json(TokenResponse {
        access_token: token,
        token_type: "Bearer".into(),
        expires_in: 3600,
        id_token: None,
        refresh_token: None,
        scope: "openid".into(),
    }).into_response()
}

#[derive(Deserialize)]
pub struct RevokeRequest { pub token: String }

async fn revoke_endpoint(State(_state): State<OidcState>, Form(_req): Form<RevokeRequest>) -> Response {
    // Token revocation — for POC, just return 200.
    StatusCode::OK.into_response()
}
