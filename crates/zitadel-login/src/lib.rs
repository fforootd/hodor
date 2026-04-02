use axum::{Router, extract::State, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post}, Json};
use zitadel_auth::password::{Passwords, decode_credential_json};
use zitadel_db::Db;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct LoginState {
    pub db: Db,
    pub passwords: Arc<Passwords>,
    pub cookie_config: Arc<zitadel_auth::cookie::CookieConfig>,
}

pub fn routes(state: LoginState) -> Router {
    Router::new()
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/settings", get(auth_settings))
        .route("/v1/branding", get(branding))
        .with_state(state)
}

#[derive(Deserialize)]
struct LoginRequest {
    identifier: String,
    password: String,
    #[serde(default)]
    auth_request_id: String,
}

#[derive(Serialize)]
struct LoginResponse {
    session_id: String,
    token: String,
    user_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    redirect_uri: String,
}

async fn login(State(state): State<LoginState>, Json(req): Json<LoginRequest>) -> Response {
    let scoped = state.db.scoped_default();

    // Find user by identifier.
    let user: Option<(String, String)> = sqlx::query_as(
        "SELECT id, org_id FROM users WHERE instance_id = ? AND identifier = ? AND state = 'active'",
    )
    .bind(scoped.instance_id())
    .bind(&req.identifier)
    .fetch_optional(scoped.pool())
    .await
    .unwrap_or(None);

    let (user_id, org_id) = match user {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))).into_response(),
    };

    // Load password credential.
    let cred: Option<(String,)> = sqlx::query_as(
        "SELECT data FROM credentials WHERE instance_id = ? AND user_id = ? AND type = 'password'",
    )
    .bind(scoped.instance_id())
    .bind(&user_id)
    .fetch_optional(scoped.pool())
    .await
    .unwrap_or(None);

    let cred_json = match cred {
        Some(c) => c.0,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))).into_response(),
    };

    let hash = match decode_credential_json(&cred_json) {
        Some(h) => h,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "corrupted credential"}))).into_response(),
    };

    if !state.passwords.verify(&hash, &req.password) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))).into_response();
    }

    // Create session.
    let session_store = zitadel_auth::session::SessionStore::new(state.db.clone());
    let (session_id, token) = match session_store.create(&scoped, &user_id, &org_id, "", "").await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("session: {e}")}))).into_response(),
    };

    // If there's an auth_request_id, complete it and generate a code.
    let mut redirect_uri = String::new();
    if !req.auth_request_id.is_empty() {
        let code = Uuid::new_v4().to_string();
        let _ = sqlx::query(
            "UPDATE auth_states SET user_id = ?, done = 1, auth_time = datetime('now'), code = ? WHERE instance_id = ? AND id = ?",
        )
        .bind(&user_id)
        .bind(&code)
        .bind(scoped.instance_id())
        .bind(&req.auth_request_id)
        .execute(scoped.pool())
        .await;

        // Get redirect_uri from auth_state.
        if let Ok(Some(row)) = sqlx::query_as::<_, (String, String)>(
            "SELECT redirect_uri, COALESCE(state, '') FROM auth_states WHERE id = ?",
        )
        .bind(&req.auth_request_id)
        .fetch_optional(scoped.pool())
        .await
        {
            let state_param = if row.1.is_empty() { String::new() } else { format!("&state={}", row.1) };
            redirect_uri = format!("{}?code={}{}", row.0, code, state_param);
        }
    }

    (StatusCode::OK, Json(LoginResponse {
        session_id,
        token,
        user_id,
        redirect_uri,
    })).into_response()
}

async fn auth_settings(State(state): State<LoginState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "password_enabled": true,
        "passkey_enabled": false,
        "external_idps": [],
        "registration_allowed": false,
    }))
}

async fn branding(State(state): State<LoginState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "org_name": "Zitadel",
        "logo_url": "",
        "primary_color": "#4A90D9",
    }))
}
