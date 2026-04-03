pub mod sso;

use axum::{
    Json, Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use zitadel_auth::password::Passwords;
use zitadel_db::{DEFAULT_INSTANCE_ID, Db};
use zitadel_storage::{DefaultStatefulStorage, DefaultTransientStorage, NewLoginFlowState};

type DefaultRpService = zitadel_oidc::rp::RpService<
    zitadel_oidc::rp::ReqwestHttpClient,
    zitadel_oidc::rp::InMemoryIssuerMetadataCache,
>;

#[derive(Clone)]
pub struct LoginState {
    pub db: Db,
    pub stateful: Arc<DefaultStatefulStorage>,
    pub transient: Arc<DefaultTransientStorage>,
    pub passwords: Arc<Passwords>,
    pub cookie_config: Arc<zitadel_auth::cookie::CookieConfig>,
    pub public_origin: Arc<String>,
    pub rp: Arc<DefaultRpService>,
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
        .route(
            "/v1/login/flows/{id}/captcha/challenge",
            get(captcha_challenge),
        )
        .merge(sso::routes())
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
    Input {
        name: String,
        label: String,
        input_type: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        value: Option<String>,
        required: bool,
    },
    #[serde(rename = "submit")]
    Submit { label: String, action: String },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "hidden")]
    Hidden { name: String, value: String },
    #[serde(rename = "avatar")]
    Avatar { initial: String, text: String },
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

/// POST /v1/login/flows — create a new login flow, returns the first step.
/// If user has an existing session and the OIDC prompt allows reuse,
/// starts at `session_reuse` step instead of `identifier`.
async fn flow_create(
    State(state): State<LoginState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<FlowCreateRequest>,
) -> Response {
    let flow_id = Uuid::new_v4().to_string();

    // Check for existing session from cookie.
    let trusted_user = extract_session_user(&state, &headers).await;

    // Load OIDC prompt from the auth_request if present.
    let prompts = if !req.auth_request_id.is_empty() {
        state
            .transient
            .load_auth_request_prompts(DEFAULT_INSTANCE_ID, &req.auth_request_id)
            .await
            .unwrap_or_default()
    } else {
        vec![]
    };

    // Determine initial step based on session + prompt.
    let (initial_step, initial_nodes) =
        if let Some((ref user_id, ref identifier, ref display_name)) = trusted_user {
            let allow_reuse = !prompts.contains(&"login".to_string())
                && !prompts.contains(&"select_account".to_string());
            let silent = prompts.contains(&"none".to_string());

            if silent && allow_reuse {
                // prompt=none: silently reuse session, complete the OIDC request immediately.
                if !req.auth_request_id.is_empty() {
                    let code = Uuid::new_v4().to_string();
                    let _ = state
                        .transient
                        .complete_auth_request(
                            DEFAULT_INSTANCE_ID,
                            &req.auth_request_id,
                            user_id,
                            &code,
                        )
                        .await;
                    if let Ok(Some(auth_req)) = state
                        .transient
                        .load_auth_request_redirect(DEFAULT_INSTANCE_ID, &req.auth_request_id)
                        .await
                    {
                        let redirect =
                            build_auth_redirect(&auth_req.redirect_uri, &auth_req.state, &code);
                        return (
                            StatusCode::CREATED,
                            Json(FlowStepResponse {
                                flow_id,
                                step: "complete".into(),
                                nodes: vec![UINode::Heading {
                                    text: "Redirecting...".into(),
                                }],
                                redirect_uri: Some(redirect),
                                branding: Some(default_branding()),
                            }),
                        )
                            .into_response();
                    }
                }
                ("identifier".to_string(), identifier_step_nodes())
            } else if allow_reuse {
                // Session exists, prompt allows reuse: show session_reuse step.
                (
                    "session_reuse".to_string(),
                    session_reuse_nodes(identifier, display_name),
                )
            } else {
                // prompt=login: force fresh login.
                ("identifier".to_string(), identifier_step_nodes())
            }
        } else {
            // No existing session.
            if prompts.contains(&"none".to_string()) {
                if !req.auth_request_id.is_empty() {
                    if let Ok(Some(auth_req)) = state
                        .transient
                        .load_auth_request_redirect(DEFAULT_INSTANCE_ID, &req.auth_request_id)
                        .await
                    {
                        let redirect = build_auth_error_redirect(
                            &auth_req.redirect_uri,
                            &auth_req.state,
                            "login_required",
                            "prompt=none requires an existing session",
                        );
                        return (
                            StatusCode::CREATED,
                            Json(FlowStepResponse {
                                flow_id,
                                step: "complete".into(),
                                nodes: vec![UINode::Heading {
                                    text: "Redirecting...".into(),
                                }],
                                redirect_uri: Some(redirect),
                                branding: Some(default_branding()),
                            }),
                        )
                            .into_response();
                    }
                }

                // prompt=none but no session: error.
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "login_required",
                        "error_description": "prompt=none requires an existing session",
                    })),
                )
                    .into_response();
            }
            ("identifier".to_string(), identifier_step_nodes())
        };

    let mut data = serde_json::json!({
        "step": initial_step,
        "redirect_uri": req.redirect_uri,
        "state": req.state,
        "fingerprint": req.fingerprint,
    });
    if !req.auth_request_id.is_empty() {
        data["auth_request_id"] = serde_json::Value::String(req.auth_request_id.clone());
    }
    if let Some((ref user_id, _, _)) = trusted_user {
        data["trusted_user_id"] = serde_json::Value::String(user_id.clone());
    }

    if let Err(e) = state
        .transient
        .create_login_flow(
            DEFAULT_INSTANCE_ID,
            &NewLoginFlowState {
                flow_id: flow_id.clone(),
                state: req.state.clone(),
                redirect_uri: req.redirect_uri.clone(),
                data,
            },
        )
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("create flow: {e}")})),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(FlowStepResponse {
            flow_id,
            step: initial_step,
            nodes: initial_nodes,
            redirect_uri: None,
            branding: Some(default_branding()),
        }),
    )
        .into_response()
}

/// GET /v1/login/flows/{id} — get current flow step.
async fn flow_get(State(state): State<LoginState>, Path(flow_id): Path<String>) -> Response {
    let flow = match state
        .transient
        .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
        .await
    {
        Ok(row) => row,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("load flow: {e}")})),
            )
                .into_response();
        }
    };

    let step = match flow {
        Some(flow) => flow.step,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "flow not found"})),
            )
                .into_response();
        }
    };

    let nodes = match step.as_str() {
        "identifier" => identifier_step_nodes(),
        "password" => password_step_nodes(""),
        "complete" => vec![UINode::Heading {
            text: "Login complete".into(),
        }],
        _ => identifier_step_nodes(),
    };

    Json(FlowStepResponse {
        flow_id,
        step,
        nodes,
        redirect_uri: None,
        branding: Some(default_branding()),
    })
    .into_response()
}

#[derive(Deserialize)]
struct FlowSubmitRequest {
    action: String,
    #[serde(default)]
    identifier: String,
    #[serde(default)]
    password: String,
    #[serde(flatten)]
    _extra: serde_json::Value,
}

/// POST /v1/login/flows/{id}/submit — process a login step.
async fn flow_submit(
    State(state): State<LoginState>,
    Path(flow_id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<FlowSubmitRequest>,
) -> Response {
    let flow = match state
        .transient
        .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
        .await
    {
        Ok(row) => row,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("load flow: {e}")})),
            )
                .into_response();
        }
    };

    let flow = match flow {
        Some(flow) => flow,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "flow not found"})),
            )
                .into_response();
        }
    };

    match req.action.as_str() {
        "identifier" => handle_identifier_step(&state, &flow_id, &req, &flow.data).await,
        "password" => {
            handle_password_step(&state, &flow_id, &req, &flow.data, &flow.redirect_uri).await
        }
        "back" => {
            let result = state
                .transient
                .set_login_flow_step(DEFAULT_INSTANCE_ID, &flow_id, "identifier")
                .await;
            match result {
                Ok(true) => {}
                Ok(false) => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({"error": "flow not found"})),
                    )
                        .into_response();
                }
                Err(e) => {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": format!("rewind flow: {e}")})),
                    )
                        .into_response();
                }
            }
            Json(FlowStepResponse {
                flow_id: flow_id.clone(),
                step: "identifier".into(),
                nodes: identifier_step_nodes(),
                redirect_uri: None,
                branding: Some(default_branding()),
            })
            .into_response()
        }
        "use_session" => handle_use_session(&state, &flow_id, &flow.data, &headers).await,
        _ => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("unknown action: {}", req.action)})),
        )
            .into_response(),
    }
}

async fn handle_identifier_step(
    state: &LoginState,
    flow_id: &str,
    req: &FlowSubmitRequest,
    data: &serde_json::Value,
) -> Response {
    if req.identifier.is_empty() {
        return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: "identifier".into(),
            nodes: {
                let mut n = vec![UINode::Error {
                    message: "Identifier is required".into(),
                }];
                n.extend(identifier_step_nodes());
                n
            },
            redirect_uri: None,
            branding: Some(default_branding()),
        })
        .into_response();
    }

    let user = match state
        .stateful
        .find_active_user_by_identifier(DEFAULT_INSTANCE_ID, &req.identifier)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("lookup user: {e}")})),
            )
                .into_response();
        }
    };

    let mut next_data = data.clone();
    next_data["identifier"] = serde_json::Value::String(req.identifier.clone());
    let user_id = user.map(|u| u.user_id).unwrap_or_default();
    match state
        .transient
        .advance_login_flow_to_password(DEFAULT_INSTANCE_ID, flow_id, &user_id, &next_data)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "advance flow: flow not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("advance flow: {e}")})),
            )
                .into_response();
        }
    }

    Json(FlowStepResponse {
        flow_id: flow_id.to_string(),
        step: "password".into(),
        nodes: password_step_nodes(&req.identifier),
        redirect_uri: None,
        branding: Some(default_branding()),
    })
    .into_response()
}

async fn handle_password_step(
    state: &LoginState,
    flow_id: &str,
    req: &FlowSubmitRequest,
    data: &serde_json::Value,
    flow_redirect: &str,
) -> Response {
    let identifier = data
        .get("identifier")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    let user = match state
        .stateful
        .find_active_user_by_identifier(DEFAULT_INSTANCE_ID, identifier)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("lookup user: {e}")})),
            )
                .into_response();
        }
    };

    let user = match user {
        Some(user) => user,
        None => {
            return Json(FlowStepResponse {
                flow_id: flow_id.to_string(),
                step: "password".into(),
                nodes: {
                    let mut n = vec![UINode::Error {
                        message: "Invalid credentials".into(),
                    }];
                    n.extend(password_step_nodes(identifier));
                    n
                },
                redirect_uri: None,
                branding: Some(default_branding()),
            })
            .into_response();
        }
    };

    let hash = match state
        .stateful
        .load_password_hash(DEFAULT_INSTANCE_ID, &user.user_id)
        .await
    {
        Ok(hash) => hash,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("load password: {e}")})),
            )
                .into_response();
        }
    };

    let valid = hash
        .as_deref()
        .map(|hash| state.passwords.verify(hash, &req.password))
        .unwrap_or(false);

    if !valid {
        return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: "password".into(),
            nodes: {
                let mut n = vec![UINode::Error {
                    message: "Invalid credentials".into(),
                }];
                n.extend(password_step_nodes(identifier));
                n
            },
            redirect_uri: None,
            branding: Some(default_branding()),
        })
        .into_response();
    }

    let auth_request_id = data
        .get("auth_request_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let auth_request = match state
        .transient
        .load_auth_request_redirect(DEFAULT_INSTANCE_ID, auth_request_id)
        .await
    {
        Ok(auth_request) => auth_request,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("load auth request: {e}")})),
            )
                .into_response();
        }
    };

    let created_session = match state
        .transient
        .create_session(DEFAULT_INSTANCE_ID, &user.user_id, &user.org_id, "", "")
        .await
    {
        Ok(session) => session,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("session: {e}")})),
            )
                .into_response();
        }
    };

    let mut redirect_uri = if !flow_redirect.is_empty() {
        flow_redirect.to_string()
    } else {
        "/console".to_string()
    };

    if let Some(auth_request) = auth_request {
        let code = Uuid::new_v4().to_string();
        if let Err(e) = state
            .transient
            .complete_auth_request(DEFAULT_INSTANCE_ID, auth_request_id, &user.user_id, &code)
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("complete auth request: {e}")})),
            )
                .into_response();
        }
        redirect_uri = build_auth_redirect(&auth_request.redirect_uri, &auth_request.state, &code);
    }

    match state
        .transient
        .complete_login_flow(DEFAULT_INSTANCE_ID, flow_id)
        .await
    {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "complete flow: flow not found"})),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("complete flow: {e}")})),
            )
                .into_response();
        }
    }

    let signed =
        zitadel_auth::cookie::sign(&created_session.token, &state.cookie_config.secrets[0]);
    let cookie_name = state.cookie_config.cookie_name();
    let secure_flag = if state.cookie_config.secure {
        "; Secure"
    } else {
        ""
    };
    let cookie_value = format!(
        "{cookie_name}={signed}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{secure_flag}",
        zitadel_auth::cookie::MAX_AGE,
    );

    let mut response = Json(FlowStepResponse {
        flow_id: flow_id.to_string(),
        step: "complete".into(),
        nodes: vec![UINode::Heading {
            text: "Login successful".into(),
        }],
        redirect_uri: Some(redirect_uri),
        branding: Some(default_branding()),
    })
    .into_response();

    response.headers_mut().insert(
        header::SET_COOKIE,
        cookie_value.parse().expect("valid cookie header"),
    );

    response
}

// ---------------------------------------------------------------------------
// UINode builders for each step
// ---------------------------------------------------------------------------

fn identifier_step_nodes() -> Vec<UINode> {
    vec![
        UINode::Heading {
            text: "Sign in".into(),
        },
        UINode::Description {
            text: "Enter your email or username".into(),
        },
        UINode::Input {
            name: "identifier".into(),
            label: "Email or username".into(),
            input_type: "text".into(),
            value: None,
            required: true,
        },
        UINode::Submit {
            label: "Continue".into(),
            action: "identifier".into(),
        },
    ]
}

fn password_step_nodes(identifier: &str) -> Vec<UINode> {
    vec![
        UINode::Heading {
            text: "Enter your password".into(),
        },
        UINode::Description {
            text: format!("Signing in as {identifier}"),
        },
        UINode::Hidden {
            name: "identifier".into(),
            value: identifier.to_string(),
        },
        UINode::Input {
            name: "password".into(),
            label: "Password".into(),
            input_type: "password".into(),
            value: None,
            required: true,
        },
        UINode::Submit {
            label: "Sign in".into(),
            action: "password".into(),
        },
        UINode::Submit {
            label: "Back".into(),
            action: "back".into(),
        },
    ]
}

fn default_branding() -> serde_json::Value {
    serde_json::json!({
        "org_name": "Zitadel",
        "primary_color": "#4A90D9",
        "logo_url": "",
    })
}

fn build_auth_redirect(redirect_uri: &str, state: &str, code: &str) -> String {
    let state_param = if state.is_empty() {
        String::new()
    } else {
        format!("&state={state}")
    };
    format!("{redirect_uri}?code={code}{state_param}")
}

fn build_auth_error_redirect(
    redirect_uri: &str,
    state: &str,
    error: &str,
    description: &str,
) -> String {
    let mut url = format!(
        "{redirect_uri}?error={}&error_description={}",
        urlencoding_encode(error),
        urlencoding_encode(description),
    );
    if !state.is_empty() {
        url.push_str("&state=");
        url.push_str(&urlencoding_encode(state));
    }
    url
}

fn urlencoding_encode(s: &str) -> String {
    url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

// ---------------------------------------------------------------------------
// Session reuse helpers
// ---------------------------------------------------------------------------

fn session_reuse_nodes(identifier: &str, display_name: &str) -> Vec<UINode> {
    let initial = display_name
        .chars()
        .next()
        .or(identifier.chars().next())
        .unwrap_or('?')
        .to_string()
        .to_uppercase();
    let avatar_text = if !display_name.is_empty() && !identifier.is_empty() {
        format!("{} · {}", display_name, identifier)
    } else if !identifier.is_empty() {
        identifier.to_string()
    } else {
        display_name.to_string()
    };
    vec![
        UINode::Heading {
            text: "Use your existing session?".into(),
        },
        UINode::Description {
            text: "You're already signed in. Continue with that session or choose a different account.".into(),
        },
        UINode::Avatar {
            initial,
            text: avatar_text,
        },
        UINode::Submit {
            label: "Continue with this session".into(),
            action: "use_session".into(),
        },
        UINode::Submit {
            label: "Use a different account".into(),
            action: "back".into(),
        },
    ]
}

/// Extract user_id from the session cookie if present.
async fn extract_session_user(
    state: &LoginState,
    headers: &axum::http::HeaderMap,
) -> Option<(String, String, String)> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;

    // Parse cookies to find the session cookie.
    let cookie_name = state.cookie_config.cookie_name();
    for part in cookie_header.split(';') {
        let part = part.trim();
        if let Some(value) = part.strip_prefix(&format!("{cookie_name}=")) {
            let token = zitadel_auth::cookie::verify(value, &state.cookie_config.secrets)?;
            let scoped = state.db.scoped_default();
            let session_store = zitadel_auth::session::SessionStore::new(state.db.clone());
            let session = session_store.find_by_token(&scoped, &token).await.ok()??;

            // Load user details.
            let user: Option<(String, String)> = sqlx::query_as(
                "SELECT identifier, display_name FROM users WHERE instance_id = $1 AND id = $2",
            )
            .bind(scoped.instance_id())
            .bind(&session.user_id)
            .fetch_optional(scoped.pool())
            .await
            .ok()?;

            let (identifier, display_name) = user?;
            return Some((session.user_id, identifier, display_name));
        }
    }
    None
}

/// Load prompt values from an OIDC auth_request.
/// Handle "use_session" action: reuse the existing trusted session to complete OIDC.
async fn handle_use_session(
    state: &LoginState,
    flow_id: &str,
    data: &serde_json::Value,
    headers: &axum::http::HeaderMap,
) -> Response {
    let trusted_user_id = data
        .get("trusted_user_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let auth_request_id = data
        .get("auth_request_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    // Re-verify the session is still valid at submission time.
    let current_user = extract_session_user(state, headers).await;
    let session_valid = current_user
        .as_ref()
        .map(|(uid, _, _)| uid == trusted_user_id)
        .unwrap_or(false);

    if trusted_user_id.is_empty() || !session_valid {
        return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: "identifier".into(),
            nodes: {
                let mut n = vec![UINode::Error {
                    message: "Session no longer available. Please sign in again.".into(),
                }];
                n.extend(identifier_step_nodes());
                n
            },
            redirect_uri: None,
            branding: Some(default_branding()),
        })
        .into_response();
    }

    if auth_request_id.is_empty() {
        // No OIDC request — just redirect to console.
        return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: "complete".into(),
            nodes: vec![UINode::Heading {
                text: "Already signed in".into(),
            }],
            redirect_uri: Some("/console".into()),
            branding: Some(default_branding()),
        })
        .into_response();
    }

    // Complete the OIDC auth request with the trusted user.
    let code = Uuid::new_v4().to_string();
    if let Err(e) = state
        .transient
        .complete_auth_request(DEFAULT_INSTANCE_ID, auth_request_id, trusted_user_id, &code)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("complete auth: {e}")})),
        )
            .into_response();
    }

    let redirect = match state
        .transient
        .load_auth_request_redirect(DEFAULT_INSTANCE_ID, auth_request_id)
        .await
    {
        Ok(Some(auth_req)) => build_auth_redirect(&auth_req.redirect_uri, &auth_req.state, &code),
        _ => "/console".to_string(),
    };

    Json(FlowStepResponse {
        flow_id: flow_id.to_string(),
        step: "complete".into(),
        nodes: vec![UINode::Heading {
            text: "Redirecting...".into(),
        }],
        redirect_uri: Some(redirect),
        branding: Some(default_branding()),
    })
    .into_response()
}

// ---------------------------------------------------------------------------
// Legacy direct login + utility endpoints
// ---------------------------------------------------------------------------

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
    let user = match state
        .stateful
        .find_active_user_by_identifier(DEFAULT_INSTANCE_ID, &req.identifier)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("lookup user: {e}")})),
            )
                .into_response();
        }
    };
    let user = match user {
        Some(user) => user,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid credentials"})),
            )
                .into_response();
        }
    };
    let hash = match state
        .stateful
        .load_password_hash(DEFAULT_INSTANCE_ID, &user.user_id)
        .await
    {
        Ok(hash) => hash,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("load password: {e}")})),
            )
                .into_response();
        }
    };
    let hash = match hash {
        Some(h) => h,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid credentials"})),
            )
                .into_response();
        }
    };
    if !state.passwords.verify(&hash, &req.password) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "invalid credentials"})),
        )
            .into_response();
    }
    let auth_request = match state
        .transient
        .load_auth_request_redirect(DEFAULT_INSTANCE_ID, &req.auth_request_id)
        .await
    {
        Ok(auth_request) => auth_request,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("load auth request: {e}")})),
            )
                .into_response();
        }
    };
    let created_session = match state
        .transient
        .create_session(DEFAULT_INSTANCE_ID, &user.user_id, &user.org_id, "", "")
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("session: {e}")})),
            )
                .into_response();
        }
    };
    let mut redirect_uri = String::new();
    if let Some(auth_request) = auth_request {
        let code = Uuid::new_v4().to_string();
        if let Err(e) = state
            .transient
            .complete_auth_request(
                DEFAULT_INSTANCE_ID,
                &req.auth_request_id,
                &user.user_id,
                &code,
            )
            .await
        {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("complete auth request: {e}")})),
            )
                .into_response();
        }
        redirect_uri = build_auth_redirect(&auth_request.redirect_uri, &auth_request.state, &code);
    }
    // Sign session token and set as cookie.
    let signed =
        zitadel_auth::cookie::sign(&created_session.token, &state.cookie_config.secrets[0]);
    let cookie_name = state.cookie_config.cookie_name();
    let secure_flag = if state.cookie_config.secure {
        "; Secure"
    } else {
        ""
    };
    let cookie_value = format!(
        "{cookie_name}={signed}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{secure_flag}",
        zitadel_auth::cookie::MAX_AGE,
    );

    let mut response = (
        StatusCode::OK,
        Json(LoginResponse {
            session_id: created_session.session_id,
            token: created_session.token,
            user_id: user.user_id,
            redirect_uri,
        }),
    )
        .into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        cookie_value.parse().expect("valid cookie header"),
    );
    response
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

async fn captcha_challenge(
    State(_state): State<LoginState>,
    Path(_flow_id): Path<String>,
) -> impl IntoResponse {
    // POC: no captcha required.
    Json(serde_json::json!({"required": false}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zitadel_auth::{
        cookie::CookieConfig,
        password::{Passwords, encode_credential_json},
    };

    async fn test_state() -> LoginState {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let stateful = Arc::new(zitadel_storage::DefaultStatefulStorage::new(
            zitadel_storage::SqlStateDb::new(db.clone()),
            zitadel_storage::SqlEdgeReadDb::new(db.clone()),
        ));
        let transient = Arc::new(zitadel_storage::DefaultTransientStorage::new(
            zitadel_storage::SqlTransientCompatKv::new(db.clone()),
            zitadel_storage::NoopEdgeSink,
        ));
        LoginState {
            db,
            stateful,
            transient,
            passwords: Arc::new(Passwords::new_dev()),
            cookie_config: Arc::new(CookieConfig::new(
                vec!["test-secret".into()],
                "localhost",
                false,
            )),
            public_origin: Arc::new("http://localhost:8080".into()),
            rp: Arc::new(zitadel_oidc::rp::RpService::new(
                zitadel_oidc::rp::ReqwestHttpClient::new(),
                zitadel_oidc::rp::InMemoryIssuerMetadataCache::default(),
            )),
        }
    }

    async fn insert_user(
        state: &LoginState,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        identifier: &str,
        password: &str,
    ) {
        let scoped = state.db.scoped(instance_id.to_string());
        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, $3)")
            .bind(org_id)
            .bind(scoped.instance_id())
            .bind(format!("{instance_id}-org"))
            .execute(scoped.pool())
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state) \
             VALUES ($1, $2, $3, $4, $5, 'human', 'active')",
        )
        .bind(user_id)
        .bind(scoped.instance_id())
        .bind(org_id)
        .bind(identifier)
        .bind(identifier)
        .execute(scoped.pool())
        .await
        .unwrap();

        let password_hash = state.passwords.hash(password).unwrap();
        let sql = format!(
            "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, $2, $3, 'password', {})",
            scoped.json_bind(4),
        );
        sqlx::query(&sql)
            .bind(format!("cred-{user_id}"))
            .bind(scoped.instance_id())
            .bind(user_id)
            .bind(encode_credential_json(&password_hash))
            .execute(scoped.pool())
            .await
            .unwrap();
    }

    /// Helper: create a login flow via the transient storage and return its flow_id.
    async fn create_flow(state: &LoginState) -> String {
        let flow_id = Uuid::new_v4().to_string();
        let data = serde_json::json!({"step": "identifier"});
        state
            .transient
            .create_login_flow(
                DEFAULT_INSTANCE_ID,
                &NewLoginFlowState {
                    flow_id: flow_id.clone(),
                    state: String::new(),
                    redirect_uri: String::new(),
                    data,
                },
            )
            .await
            .unwrap();
        flow_id
    }

    // ─── Happy Path ───────────────────────────────────────

    #[tokio::test]
    async fn identifier_step_advances_to_password() {
        let state = test_state().await;
        insert_user(&state, "default", "u1", "org1", "alice", "pass123").await;
        let flow_id = create_flow(&state).await;

        let req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: "alice".into(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let resp = handle_identifier_step(&state, &flow_id, &req, &flow.data).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Flow should now be at password step.
        let updated = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.step, "password");
    }

    #[tokio::test]
    async fn correct_password_creates_session() {
        let state = test_state().await;
        insert_user(&state, "default", "u1", "org1", "alice", "pass123").await;
        let flow_id = create_flow(&state).await;

        // Advance to password step first.
        let id_req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: "alice".into(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        handle_identifier_step(&state, &flow_id, &id_req, &flow.data).await;

        // Now submit password.
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let pwd_req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "pass123".into(),
            _extra: serde_json::Value::Null,
        };
        let resp =
            handle_password_step(&state, &flow_id, &pwd_req, &flow.data, &flow.redirect_uri).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Session should exist.
        let scoped = state.db.scoped_default();
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND user_id = $2")
                .bind(scoped.instance_id())
                .bind("u1")
                .fetch_one(scoped.pool())
                .await
                .unwrap();
        assert_eq!(count.0, 1, "session should be created after correct password");

        // Flow should be complete.
        let completed = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(completed.step, "complete");
    }

    // ─── Rejection Cases ──────────────────────────────────

    #[tokio::test]
    async fn wrong_password_rejected_no_session() {
        let state = test_state().await;
        insert_user(&state, "default", "u1", "org1", "alice", "pass123").await;
        let flow_id = create_flow(&state).await;

        // Advance to password.
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let id_req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: "alice".into(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        handle_identifier_step(&state, &flow_id, &id_req, &flow.data).await;

        // Submit wrong password.
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let pwd_req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "WRONG".into(),
            _extra: serde_json::Value::Null,
        };
        let resp =
            handle_password_step(&state, &flow_id, &pwd_req, &flow.data, &flow.redirect_uri).await;
        // Should return 200 with error node (not 401 — server-driven UI).
        assert_eq!(resp.status(), StatusCode::OK);

        // No session should be created.
        let scoped = state.db.scoped_default();
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND user_id = $2")
                .bind(scoped.instance_id())
                .bind("u1")
                .fetch_one(scoped.pool())
                .await
                .unwrap();
        assert_eq!(count.0, 0, "no session after wrong password");
    }

    #[tokio::test]
    async fn nonexistent_user_rejected() {
        let state = test_state().await;
        // Don't create any user.
        let flow_id = create_flow(&state).await;

        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let id_req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: "nobody".into(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        // Identifier step should still advance (don't reveal user existence).
        let resp = handle_identifier_step(&state, &flow_id, &id_req, &flow.data).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Password step should fail (user not found → invalid credentials).
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let pwd_req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "anything".into(),
            _extra: serde_json::Value::Null,
        };
        let resp =
            handle_password_step(&state, &flow_id, &pwd_req, &flow.data, &flow.redirect_uri).await;
        assert_eq!(resp.status(), StatusCode::OK); // returns error in UI nodes

        // No session.
        let scoped = state.db.scoped_default();
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1")
                .bind(scoped.instance_id())
                .fetch_one(scoped.pool())
                .await
                .unwrap();
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn empty_identifier_rejected() {
        let state = test_state().await;
        let flow_id = create_flow(&state).await;

        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: String::new(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        let resp = handle_identifier_step(&state, &flow_id, &req, &flow.data).await;
        assert_eq!(resp.status(), StatusCode::OK); // returns error node

        // Flow should still be at identifier step (not advanced).
        let same = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(same.step, "identifier");
    }

    // ─── Instance Isolation ───────────────────────────────

    #[tokio::test]
    async fn user_from_other_instance_not_found() {
        let state = test_state().await;
        // Insert user in "other" instance, not "default".
        insert_user(&state, "other", "u1", "org1", "alice", "pass123").await;
        let flow_id = create_flow(&state).await;

        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let id_req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: "alice".into(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        handle_identifier_step(&state, &flow_id, &id_req, &flow.data).await;

        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let pwd_req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "pass123".into(),
            _extra: serde_json::Value::Null,
        };
        let resp =
            handle_password_step(&state, &flow_id, &pwd_req, &flow.data, &flow.redirect_uri).await;
        assert_eq!(resp.status(), StatusCode::OK); // error in nodes

        // No session in default instance.
        let scoped = state.db.scoped_default();
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1")
                .bind(scoped.instance_id())
                .fetch_one(scoped.pool())
                .await
                .unwrap();
        assert_eq!(count.0, 0);
    }

    // ─── Original cross-tenant test ───────────────────────

    #[tokio::test]
    async fn password_step_rejects_cross_tenant_auth_request() {
        let state = test_state().await;
        insert_user(&state, "default", "user-1", "org-1", "alice", "secret").await;

        let flow_scoped = state.db.scoped_default();
        let foreign_scoped = state.db.scoped("other".to_string());

        let flow_data = serde_json::json!({
            "identifier": "alice",
            "auth_request_id": "foreign-auth",
        });
        let flow_sql = format!(
            "INSERT INTO auth_states (id, instance_id, type, redirect_uri, data, step) \
             VALUES ($1, $2, 'login_flow', $3, {}, 'password')",
            flow_scoped.json_bind(4),
        );
        sqlx::query(&flow_sql)
            .bind("flow-1")
            .bind(flow_scoped.instance_id())
            .bind("/console")
            .bind(flow_data.to_string())
            .execute(flow_scoped.pool())
            .await
            .unwrap();

        let foreign_auth_sql =
            "INSERT INTO oidc_auth_requests (id, instance_id, client_id, redirect_uri, state, prompt) \
             VALUES ($1, $2, $3, $4, $5, $6)";
        sqlx::query(&foreign_auth_sql)
            .bind("foreign-auth")
            .bind(foreign_scoped.instance_id())
            .bind("client-1")
            .bind("https://rp.example/callback")
            .bind("foreign-state")
            .bind("[]")
            .execute(foreign_scoped.pool())
            .await
            .unwrap();

        let req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "secret".into(),
            _extra: serde_json::Value::Null,
        };

        let response = handle_password_step(&state, "flow-1", &req, &flow_data, "/console").await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let foreign_row: (String, String, i64) = sqlx::query_as(&format!(
            "SELECT COALESCE(user_id, ''), COALESCE(code, ''), {} FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2",
            foreign_scoped.bool_as_int("done"),
        ))
        .bind(foreign_scoped.instance_id())
        .bind("foreign-auth")
        .fetch_one(foreign_scoped.pool())
        .await
        .unwrap();
        assert_eq!(foreign_row.0, "");
        assert_eq!(foreign_row.1, "");
        assert_eq!(foreign_row.2, 0);

        let session_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND user_id = $2")
                .bind(flow_scoped.instance_id())
                .bind("user-1")
                .fetch_one(flow_scoped.pool())
                .await
                .unwrap();
        assert_eq!(session_count.0, 0);
    }
}
