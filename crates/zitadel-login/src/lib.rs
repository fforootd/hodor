use axum::{Router, extract::{Path, State}, http::StatusCode, response::{IntoResponse, Response}, routing::{get, post}, Json};
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
        // Direct login (legacy/simple)
        .route("/v1/auth/login", post(login))
        .route("/v1/auth/settings", get(auth_settings))
        .route("/v1/branding", get(branding))
        // Server-driven login flow (what the login SPA actually uses)
        .route("/v1/login/flows", post(flow_create))
        .route("/v1/login/flows/{id}", get(flow_get))
        .route("/v1/login/flows/{id}/submit", post(flow_submit))
        .route("/v1/login/flows/{id}/captcha/challenge", get(captcha_challenge))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Server-driven login flow state machine
// ---------------------------------------------------------------------------

/// UINode types that the login SPA renders.
#[derive(Serialize, Clone)]
#[serde(tag = "type")]
enum UINode {
    #[serde(rename = "heading")]
    Heading { text: String },
    #[serde(rename = "description")]
    Description { text: String },
    #[serde(rename = "input")]
    Input { name: String, label: String, input_type: String, #[serde(skip_serializing_if = "Option::is_none")] value: Option<String>, required: bool },
    #[serde(rename = "submit")]
    Submit { label: String, action: String },
    #[serde(rename = "divider")]
    Divider {},
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "hidden")]
    Hidden { name: String, value: String },
}

#[derive(Serialize)]
struct FlowStepResponse {
    flow_id: String,
    step: String,
    nodes: Vec<UINode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    redirect_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    branding: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct FlowCreateRequest {
    #[serde(default)]
    redirect_uri: String,
    #[serde(default)]
    state: String,
    #[serde(default)]
    auth_request_id: String,
    #[serde(default)]
    fingerprint: String,
}

/// POST /v1/login/flows — create a new login flow, returns the first step (identifier).
async fn flow_create(State(state): State<LoginState>, Json(req): Json<FlowCreateRequest>) -> Response {
    let scoped = state.db.scoped_default();
    let flow_id = Uuid::new_v4().to_string();

    // Store flow state in auth_states table.
    let data = serde_json::json!({
        "step": "identifier",
        "redirect_uri": req.redirect_uri,
        "state": req.state,
        "fingerprint": req.fingerprint,
    });
    let _ = sqlx::query(
        "INSERT INTO auth_states (id, instance_id, type, state, redirect_uri, data, step) \
         VALUES (?, ?, 'login_flow', ?, ?, ?, 'identifier')")
        .bind(&flow_id)
        .bind(scoped.instance_id())
        .bind(&req.state)
        .bind(&req.redirect_uri)
        .bind(serde_json::to_string(&data).unwrap_or_default())
        .execute(scoped.pool())
        .await;

    // If there's an auth_request_id, link it.
    if !req.auth_request_id.is_empty() {
        let _ = sqlx::query("UPDATE auth_states SET data = json_set(data, '$.auth_request_id', ?) WHERE id = ?")
            .bind(&req.auth_request_id)
            .bind(&flow_id)
            .execute(scoped.pool())
            .await;
    }

    let nodes = identifier_step_nodes();
    (StatusCode::CREATED, Json(FlowStepResponse {
        flow_id,
        step: "identifier".into(),
        nodes,
        redirect_uri: None,
        branding: Some(default_branding()),
    })).into_response()
}

/// GET /v1/login/flows/{id} — get current flow step.
async fn flow_get(State(state): State<LoginState>, Path(flow_id): Path<String>) -> Response {
    let scoped = state.db.scoped_default();
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT COALESCE(step, 'identifier'), COALESCE(data, '{}') FROM auth_states WHERE instance_id = ? AND id = ? AND type = 'login_flow'")
        .bind(scoped.instance_id())
        .bind(&flow_id)
        .fetch_optional(scoped.pool())
        .await
        .unwrap_or(None);

    let (step, _data) = match row {
        Some(r) => r,
        None => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "flow not found"}))).into_response(),
    };

    let nodes = match step.as_str() {
        "identifier" => identifier_step_nodes(),
        "password" => password_step_nodes(""),
        "complete" => vec![UINode::Heading { text: "Login complete".into() }],
        _ => identifier_step_nodes(),
    };

    Json(FlowStepResponse {
        flow_id,
        step,
        nodes,
        redirect_uri: None,
        branding: Some(default_branding()),
    }).into_response()
}

#[derive(Deserialize)]
struct FlowSubmitRequest {
    action: String,
    #[serde(default)]
    identifier: String,
    #[serde(default)]
    password: String,
    #[serde(flatten)]
    extra: serde_json::Value,
}

/// POST /v1/login/flows/{id}/submit — process a login step.
async fn flow_submit(State(state): State<LoginState>, Path(flow_id): Path<String>, Json(req): Json<FlowSubmitRequest>) -> Response {
    let scoped = state.db.scoped_default();

    // Load current flow state.
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT COALESCE(step, 'identifier'), COALESCE(data, '{}'), COALESCE(redirect_uri, '') \
         FROM auth_states WHERE instance_id = ? AND id = ? AND type = 'login_flow'")
        .bind(scoped.instance_id())
        .bind(&flow_id)
        .fetch_optional(scoped.pool())
        .await
        .unwrap_or(None);

    let (current_step, data_str, flow_redirect) = match row {
        Some(r) => r,
        None => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "flow not found"}))).into_response(),
    };

    let flow_data: serde_json::Value = serde_json::from_str(&data_str).unwrap_or_default();

    match req.action.as_str() {
        "identifier" => handle_identifier_step(&state, &scoped, &flow_id, &req, &flow_data).await,
        "password" => handle_password_step(&state, &scoped, &flow_id, &req, &flow_data, &flow_redirect).await,
        "back" => {
            let _ = sqlx::query("UPDATE auth_states SET step = 'identifier' WHERE id = ?")
                .bind(&flow_id).execute(scoped.pool()).await;
            Json(FlowStepResponse {
                flow_id: flow_id.clone(),
                step: "identifier".into(),
                nodes: identifier_step_nodes(),
                redirect_uri: None,
                branding: Some(default_branding()),
            }).into_response()
        }
        _ => (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("unknown action: {}", req.action)}))).into_response(),
    }
}

async fn handle_identifier_step(
    state: &LoginState, scoped: &zitadel_db::scoped::ScopedDb, flow_id: &str,
    req: &FlowSubmitRequest, _data: &serde_json::Value,
) -> Response {
    if req.identifier.is_empty() {
        return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: "identifier".into(),
            nodes: {
                let mut n = vec![UINode::Error { message: "Identifier is required".into() }];
                n.extend(identifier_step_nodes());
                n
            },
            redirect_uri: None,
            branding: Some(default_branding()),
        }).into_response();
    }

    // Check if user exists.
    let user: Option<(String,)> = sqlx::query_as(
        "SELECT id FROM users WHERE instance_id = ? AND identifier = ? AND state = 'active'")
        .bind(scoped.instance_id())
        .bind(&req.identifier)
        .fetch_optional(scoped.pool())
        .await
        .unwrap_or(None);

    if user.is_none() {
        // Don't reveal if user exists — still go to password step.
    }

    // Update flow state: store identifier, advance to password step.
    let _ = sqlx::query("UPDATE auth_states SET step = 'password', user_id = COALESCE((SELECT id FROM users WHERE instance_id = ? AND identifier = ?), ''), data = json_set(data, '$.identifier', ?) WHERE id = ?")
        .bind(scoped.instance_id())
        .bind(&req.identifier)
        .bind(&req.identifier)
        .bind(flow_id)
        .execute(scoped.pool())
        .await;

    Json(FlowStepResponse {
        flow_id: flow_id.to_string(),
        step: "password".into(),
        nodes: password_step_nodes(&req.identifier),
        redirect_uri: None,
        branding: Some(default_branding()),
    }).into_response()
}

async fn handle_password_step(
    state: &LoginState, scoped: &zitadel_db::scoped::ScopedDb, flow_id: &str,
    req: &FlowSubmitRequest, data: &serde_json::Value, flow_redirect: &str,
) -> Response {
    let identifier = data.get("identifier").and_then(|v| v.as_str()).unwrap_or_default();

    // Find user.
    let user: Option<(String, String)> = sqlx::query_as(
        "SELECT id, org_id FROM users WHERE instance_id = ? AND identifier = ? AND state = 'active'")
        .bind(scoped.instance_id())
        .bind(identifier)
        .fetch_optional(scoped.pool())
        .await
        .unwrap_or(None);

    let (user_id, org_id) = match user {
        Some(u) => u,
        None => return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: "password".into(),
            nodes: {
                let mut n = vec![UINode::Error { message: "Invalid credentials".into() }];
                n.extend(password_step_nodes(identifier));
                n
            },
            redirect_uri: None,
            branding: Some(default_branding()),
        }).into_response(),
    };

    // Verify password.
    let cred: Option<(String,)> = sqlx::query_as(
        "SELECT data FROM credentials WHERE instance_id = ? AND user_id = ? AND type = 'password'")
        .bind(scoped.instance_id())
        .bind(&user_id)
        .fetch_optional(scoped.pool())
        .await
        .unwrap_or(None);

    let valid = if let Some(c) = cred {
        if let Some(hash) = decode_credential_json(&c.0) {
            state.passwords.verify(&hash, &req.password)
        } else {
            false
        }
    } else {
        false
    };

    if !valid {
        return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: "password".into(),
            nodes: {
                let mut n = vec![UINode::Error { message: "Invalid credentials".into() }];
                n.extend(password_step_nodes(identifier));
                n
            },
            redirect_uri: None,
            branding: Some(default_branding()),
        }).into_response();
    }

    // Password correct — create session.
    let session_store = zitadel_auth::session::SessionStore::new(state.db.clone());
    let (session_id, token) = match session_store.create(scoped, &user_id, &org_id, "", "").await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("session: {e}")}))).into_response(),
    };

    // Complete auth_request if linked (OIDC flow).
    let auth_request_id = data.get("auth_request_id").and_then(|v| v.as_str()).unwrap_or_default();
    let mut redirect_uri = if !flow_redirect.is_empty() { flow_redirect.to_string() } else { "/console".to_string() };

    if !auth_request_id.is_empty() {
        let code = Uuid::new_v4().to_string();
        let _ = sqlx::query("UPDATE auth_states SET user_id = ?, done = 1, auth_time = datetime('now'), code = ? WHERE instance_id = ? AND id = ?")
            .bind(&user_id).bind(&code).bind(scoped.instance_id()).bind(auth_request_id)
            .execute(scoped.pool()).await;

        if let Ok(Some(row)) = sqlx::query_as::<_, (String, String)>(
            "SELECT redirect_uri, COALESCE(state, '') FROM auth_states WHERE id = ?")
            .bind(auth_request_id).fetch_optional(scoped.pool()).await
        {
            let state_param = if row.1.is_empty() { String::new() } else { format!("&state={}", row.1) };
            redirect_uri = format!("{}?code={}{}", row.0, code, state_param);
        }
    }

    // Update flow to complete.
    let _ = sqlx::query("UPDATE auth_states SET step = 'complete', done = 1 WHERE id = ?")
        .bind(flow_id).execute(scoped.pool()).await;

    Json(FlowStepResponse {
        flow_id: flow_id.to_string(),
        step: "complete".into(),
        nodes: vec![
            UINode::Heading { text: "Login successful".into() },
        ],
        redirect_uri: Some(redirect_uri),
        branding: Some(default_branding()),
    }).into_response()
}

// ---------------------------------------------------------------------------
// UINode builders for each step
// ---------------------------------------------------------------------------

fn identifier_step_nodes() -> Vec<UINode> {
    vec![
        UINode::Heading { text: "Sign in".into() },
        UINode::Description { text: "Enter your email or username".into() },
        UINode::Input {
            name: "identifier".into(),
            label: "Email or username".into(),
            input_type: "text".into(),
            value: None,
            required: true,
        },
        UINode::Submit { label: "Continue".into(), action: "identifier".into() },
    ]
}

fn password_step_nodes(identifier: &str) -> Vec<UINode> {
    vec![
        UINode::Heading { text: "Enter your password".into() },
        UINode::Description { text: format!("Signing in as {identifier}") },
        UINode::Hidden { name: "identifier".into(), value: identifier.to_string() },
        UINode::Input {
            name: "password".into(),
            label: "Password".into(),
            input_type: "password".into(),
            value: None,
            required: true,
        },
        UINode::Submit { label: "Sign in".into(), action: "password".into() },
        UINode::Submit { label: "Back".into(), action: "back".into() },
    ]
}

fn default_branding() -> serde_json::Value {
    serde_json::json!({
        "org_name": "Zitadel",
        "primary_color": "#4A90D9",
        "logo_url": "",
    })
}

// ---------------------------------------------------------------------------
// Legacy direct login + utility endpoints
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LoginRequest { identifier: String, password: String, #[serde(default)] auth_request_id: String }

#[derive(Serialize)]
struct LoginResponse { session_id: String, token: String, user_id: String, #[serde(skip_serializing_if = "String::is_empty")] redirect_uri: String }

async fn login(State(state): State<LoginState>, Json(req): Json<LoginRequest>) -> Response {
    let scoped = state.db.scoped_default();
    let user: Option<(String, String)> = sqlx::query_as(
        "SELECT id, org_id FROM users WHERE instance_id = ? AND identifier = ? AND state = 'active'")
        .bind(scoped.instance_id()).bind(&req.identifier).fetch_optional(scoped.pool()).await.unwrap_or(None);
    let (user_id, org_id) = match user {
        Some(u) => u,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))).into_response(),
    };
    let cred: Option<(String,)> = sqlx::query_as(
        "SELECT data FROM credentials WHERE instance_id = ? AND user_id = ? AND type = 'password'")
        .bind(scoped.instance_id()).bind(&user_id).fetch_optional(scoped.pool()).await.unwrap_or(None);
    let hash = match cred.and_then(|c| decode_credential_json(&c.0)) {
        Some(h) => h,
        None => return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))).into_response(),
    };
    if !state.passwords.verify(&hash, &req.password) {
        return (StatusCode::UNAUTHORIZED, Json(serde_json::json!({"error": "invalid credentials"}))).into_response();
    }
    let session_store = zitadel_auth::session::SessionStore::new(state.db.clone());
    let (session_id, token) = match session_store.create(&scoped, &user_id, &org_id, "", "").await {
        Ok(r) => r,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("session: {e}")}))).into_response(),
    };
    let mut redirect_uri = String::new();
    if !req.auth_request_id.is_empty() {
        let code = Uuid::new_v4().to_string();
        let _ = sqlx::query("UPDATE auth_states SET user_id = ?, done = 1, auth_time = datetime('now'), code = ? WHERE instance_id = ? AND id = ?")
            .bind(&user_id).bind(&code).bind(scoped.instance_id()).bind(&req.auth_request_id)
            .execute(scoped.pool()).await;
        if let Ok(Some(row)) = sqlx::query_as::<_, (String, String)>(
            "SELECT redirect_uri, COALESCE(state, '') FROM auth_states WHERE id = ?")
            .bind(&req.auth_request_id).fetch_optional(scoped.pool()).await {
            let sp = if row.1.is_empty() { String::new() } else { format!("&state={}", row.1) };
            redirect_uri = format!("{}?code={}{}", row.0, code, sp);
        }
    }
    (StatusCode::OK, Json(LoginResponse { session_id, token, user_id, redirect_uri })).into_response()
}

async fn auth_settings(State(_state): State<LoginState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "password_enabled": true,
        "passkey_enabled": false,
        "external_idps": [],
        "registration_allowed": false,
    }))
}

async fn branding(State(_state): State<LoginState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "org_name": "Zitadel",
        "logo_url": "",
        "primary_color": "#4A90D9",
    }))
}

async fn captcha_challenge(State(_state): State<LoginState>, Path(_flow_id): Path<String>) -> impl IntoResponse {
    // POC: no captcha required.
    Json(serde_json::json!({"required": false}))
}

