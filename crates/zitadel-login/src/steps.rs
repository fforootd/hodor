use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_db::current_instance_id;
use zitadel_storage::{LoginFlowRuntimeState, NewLoginFlowState};

use crate::LoginState;
use crate::bot;
use crate::cookie;
use crate::oidc_completion;
use crate::password;
use crate::redirect::{build_auth_error_redirect, build_auth_redirect};
use crate::session::{extract_session_user, now_epoch_seconds, session_satisfies_max_age};
use crate::ui::{
    UINode, default_branding, identifier_step_nodes, password_step_nodes, session_reuse_nodes,
};

use zitadel_app::auth::IssueSessionCommand;

/// Build an ActorContext for unauthenticated login flows.
/// The user isn't authenticated yet, so we use empty identity fields
/// with the current instance_id.
fn login_actor_context() -> zitadel_app::ActorContext {
    let instance_id = current_instance_id().into_owned();
    zitadel_app::ActorContext {
        auth: zitadel_app::context::AuthContext {
            identity: zitadel_app::context::Identity {
                user_id: String::new(),
                principal_ref: String::new(),
                session_id: String::new(),
                token_type: "login_flow".to_string(),
                org_id: String::new(),
                issuer_instance_id: None,
                support_grant: None,
            },
            capabilities: vec![],
        },
        instance: zitadel_app::context::InstanceContext {
            instance_id,
            placement_mode: String::new(),
            region_key: None,
            feature_overrides: Default::default(),
            host: String::new(),
        },
    }
}

// ---------------------------------------------------------------------------
// Login step enum
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum LoginStep {
    Identifier,
    Password,
    SessionReuse,
    Complete,
}

impl LoginStep {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Identifier => "identifier",
            Self::Password => "password",
            Self::SessionReuse => "session_reuse",
            Self::Complete => "complete",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "identifier" => Some(Self::Identifier),
            "password" => Some(Self::Password),
            "session_reuse" => Some(Self::SessionReuse),
            "complete" => Some(Self::Complete),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Default, Serialize)]
pub(crate) struct FlowStepResponse {
    pub flow_id: String,
    pub step: String,
    pub nodes: Vec<UINode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branding: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captcha_required: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub captcha_verified: Option<bool>,
}

impl FlowStepResponse {
    pub(crate) fn new(flow_id: String, step: String, nodes: Vec<UINode>) -> Self {
        Self {
            flow_id,
            step,
            nodes,
            redirect_uri: None,
            branding: Some(default_branding()),
            captcha_required: None,
            captcha_verified: None,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct FlowCreateRequest {
    #[serde(default)]
    pub redirect_uri: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub auth_request_id: String,
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Deserialize)]
pub(crate) struct FlowSubmitRequest {
    pub action: String,
    #[serde(default)]
    pub identifier: String,
    #[serde(default)]
    pub password: String,
    #[serde(flatten)]
    pub _extra: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// POST /v1/login/flows — create a new login flow, returns the first step.
/// If user has an existing session and the OIDC prompt allows reuse,
/// starts at `session_reuse` step instead of `identifier`.
pub(crate) async fn flow_create(
    State(state): State<LoginState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<FlowCreateRequest>,
) -> Response {
    let instance_id = current_instance_id();
    let flow_id = Uuid::new_v4().to_string();

    // Check for existing session from cookie.
    let trusted_user = extract_session_user(&state, &headers).await;

    // Load OIDC prompt from the auth_request if present.
    let requirements = if !req.auth_request_id.is_empty() {
        state
            .transient
            .load_auth_request_prompts(&instance_id, &req.auth_request_id)
            .await
            .unwrap_or_default()
    } else {
        Default::default()
    };
    let prompts = requirements.prompt;
    let max_age = requirements.max_age;
    let now = now_epoch_seconds();

    // Determine initial step based on session + prompt.
    let (initial_step, initial_nodes) = if let Some(ref trusted_user) = trusted_user {
        let allow_reuse = !prompts.contains(&"login".to_string())
            && !prompts.contains(&"select_account".to_string());
        let silent = prompts.contains(&"none".to_string());
        let session_fresh =
            session_satisfies_max_age(trusted_user.authenticated_at_epoch, max_age, now);
        let can_reuse = allow_reuse && session_fresh;

        if silent && can_reuse {
            // prompt=none: silently reuse session, complete the OIDC request immediately.
            if !req.auth_request_id.is_empty() {
                let code = Uuid::new_v4().to_string();
                let auth_request = state
                    .transient
                    .complete_auth_request(
                        &instance_id,
                        &req.auth_request_id,
                        &trusted_user.user_id,
                        Some(&trusted_user.session_id),
                        &code,
                        Some(&trusted_user.authenticated_at),
                    )
                    .await
                    .ok()
                    .flatten();
                if let Some(auth_req) = auth_request {
                    let redirect =
                        build_auth_redirect(&auth_req.redirect_uri, &auth_req.state, &code);
                    return (
                        StatusCode::CREATED,
                        Json(FlowStepResponse {
                            flow_id,
                            step: LoginStep::Complete.as_str().into(),
                            nodes: vec![UINode::Heading {
                                text: "Redirecting...".into(),
                            }],
                            redirect_uri: Some(redirect),
                            branding: Some(default_branding()),
                            ..Default::default()
                        }),
                    )
                        .into_response();
                }
            }
            (LoginStep::Identifier, identifier_step_nodes())
        } else if can_reuse {
            // Session exists, prompt allows reuse: show session_reuse step.
            (
                LoginStep::SessionReuse,
                session_reuse_nodes(&trusted_user.identifier, &trusted_user.display_name),
            )
        } else if silent {
            if !req.auth_request_id.is_empty()
                && let Ok(Some(auth_req)) = state
                    .transient
                    .load_auth_request_redirect(&instance_id, &req.auth_request_id)
                    .await
            {
                let redirect = build_auth_error_redirect(
                    &auth_req.redirect_uri,
                    &auth_req.state,
                    "login_required",
                    "prompt=none requires a recent session",
                );
                return (
                    StatusCode::CREATED,
                    Json(FlowStepResponse {
                        flow_id,
                        step: LoginStep::Complete.as_str().into(),
                        nodes: vec![UINode::Heading {
                            text: "Redirecting...".into(),
                        }],
                        redirect_uri: Some(redirect),
                        branding: Some(default_branding()),
                        ..Default::default()
                    }),
                )
                    .into_response();
            }

            (
                LoginStep::Identifier,
                vec![UINode::Error {
                    message: "A fresh sign-in is required.".into(),
                }],
            )
        } else {
            // prompt=login or stale session: force fresh login.
            (LoginStep::Identifier, identifier_step_nodes())
        }
    } else {
        // No existing session.
        if prompts.contains(&"none".to_string()) {
            if !req.auth_request_id.is_empty()
                && let Ok(Some(auth_req)) = state
                    .transient
                    .load_auth_request_redirect(&instance_id, &req.auth_request_id)
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
                        step: LoginStep::Complete.as_str().into(),
                        nodes: vec![UINode::Heading {
                            text: "Redirecting...".into(),
                        }],
                        redirect_uri: Some(redirect),
                        branding: Some(default_branding()),
                        ..Default::default()
                    }),
                )
                    .into_response();
            }

            // prompt=none but no session: error.
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "login_required",
                    "error_description": "prompt=none requires a recent session",
                })),
            )
                .into_response();
        }
        (LoginStep::Identifier, identifier_step_nodes())
    };

    let initial_step_str = initial_step.as_str().to_string();

    // Load bot protection setting from DB and score the request.
    let bp = bot::load_bot_protection(&state.app.repos, &instance_id).await;
    let (risk_score, risk_signals) = bot::score_and_record(
        &state.app.repos,
        &instance_id,
        &flow_id,
        &req.fingerprint,
        &headers,
        &bp,
    )
    .await;

    let mut data = serde_json::json!({
        "step": initial_step_str,
        "redirect_uri": req.redirect_uri,
        "state": req.state,
        "fingerprint": req.fingerprint,
        "risk_score": risk_score,
        "risk_signals": risk_signals,
        "bot_protection_mode": bp.mode,
        "bot_protection_threshold": bp.risk_threshold,
        "bot_protection_action": bp.action,
        "bot_protection_provider": bp.provider,
        "bot_protection_provider_config": bp.provider_config,
    });
    if !req.auth_request_id.is_empty() {
        data["auth_request_id"] = serde_json::Value::String(req.auth_request_id.clone());
    }
    if let Some(ref trusted_user) = trusted_user {
        data["trusted_user_id"] = serde_json::Value::String(trusted_user.user_id.clone());
    }

    if let Err(e) = state
        .transient
        .create_login_flow(
            &instance_id,
            &NewLoginFlowState {
                flow_id: flow_id.clone(),
                state: req.state.clone(),
                redirect_uri: req.redirect_uri.clone(),
                data,
            },
        )
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, {
            tracing::error!(%e, "create flow");
            Json(serde_json::json!({"error": "internal error"}))
        })
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(FlowStepResponse {
            flow_id,
            step: initial_step_str,
            nodes: initial_nodes,
            redirect_uri: None,
            branding: Some(default_branding()),
            ..Default::default()
        }),
    )
        .into_response()
}

/// GET /v1/login/flows/{id} — get current flow step.
pub(crate) async fn flow_get(
    State(state): State<LoginState>,
    Path(flow_id): Path<String>,
) -> Response {
    let instance_id = current_instance_id();
    let flow = match state
        .transient
        .load_login_flow(&instance_id, &flow_id)
        .await
    {
        Ok(row) => row,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, {
                tracing::error!(%e, "load flow");
                Json(serde_json::json!({"error": "internal error"}))
            })
                .into_response();
        }
    };

    let step_str = match flow {
        Some(ref flow) => flow.step.clone(),
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "flow not found"})),
            )
                .into_response();
        }
    };

    let step = LoginStep::from_str(&step_str);
    let nodes = match step {
        Some(LoginStep::Identifier) => identifier_step_nodes(),
        Some(LoginStep::Password) => password_step_nodes(""),
        Some(LoginStep::Complete) => vec![UINode::Heading {
            text: "Login complete".into(),
        }],
        _ => identifier_step_nodes(),
    };

    Json(FlowStepResponse {
        flow_id,
        step: step_str,
        nodes,
        redirect_uri: None,
        branding: Some(default_branding()),
        ..Default::default()
    })
    .into_response()
}

/// POST /v1/login/flows/{id}/submit — process a login step.
pub(crate) async fn flow_submit(
    State(state): State<LoginState>,
    Path(flow_id): Path<String>,
    headers: axum::http::HeaderMap,
    Json(req): Json<FlowSubmitRequest>,
) -> Response {
    let instance_id = current_instance_id();
    let flow = match state
        .transient
        .load_login_flow(&instance_id, &flow_id)
        .await
    {
        Ok(row) => row,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, {
                tracing::error!(%e, "load flow");
                Json(serde_json::json!({"error": "internal error"}))
            })
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
                .set_login_flow_step(&instance_id, &flow_id, LoginStep::Identifier.as_str())
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
                    return (StatusCode::INTERNAL_SERVER_ERROR, {
                        tracing::error!(%e, "rewind flow");
                        Json(serde_json::json!({"error": "internal error"}))
                    })
                        .into_response();
                }
            }
            Json(FlowStepResponse::new(
                flow_id.clone(),
                LoginStep::Identifier.as_str().into(),
                identifier_step_nodes(),
            ))
            .into_response()
        }
        "use_session" => handle_use_session(&state, &flow_id, &flow.data, &headers).await,
        "fingerprint_submit" => handle_fingerprint_submit(&state, &flow_id, &flow, &req).await,
        "captcha_submit" => handle_captcha_submit(&state, &flow_id, &flow, &req).await,
        _ => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("unknown action: {}", req.action)})),
        )
            .into_response(),
    }
}

pub(crate) async fn handle_identifier_step(
    state: &LoginState,
    flow_id: &str,
    req: &FlowSubmitRequest,
    data: &serde_json::Value,
) -> Response {
    let instance_id = current_instance_id();
    if req.identifier.is_empty() {
        return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: LoginStep::Identifier.as_str().into(),
            nodes: {
                let mut n = vec![UINode::Error {
                    message: "Identifier is required".into(),
                }];
                n.extend(identifier_step_nodes());
                n
            },
            redirect_uri: None,
            branding: Some(default_branding()),
            ..Default::default()
        })
        .into_response();
    }

    let user = match state
        .stateful
        .find_active_user_by_identifier(&instance_id, &req.identifier)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, {
                tracing::error!(%e, "lookup user");
                Json(serde_json::json!({"error": "internal error"}))
            })
                .into_response();
        }
    };

    let mut next_data = data.clone();
    next_data["identifier"] = serde_json::Value::String(req.identifier.clone());
    let user_id = user.map(|u| u.user_id).unwrap_or_default();
    match state
        .transient
        .advance_login_flow_to_password(&instance_id, flow_id, &user_id, &next_data)
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
            return (StatusCode::INTERNAL_SERVER_ERROR, {
                tracing::error!(%e, "advance flow");
                Json(serde_json::json!({"error": "internal error"}))
            })
                .into_response();
        }
    }

    // Check bot enforcement (block / captcha challenge).
    let (needs_captcha, blocked) = bot::check_bot_enforcement(data);
    if let Some(block_response) = blocked {
        return block_response;
    }

    let mut resp = FlowStepResponse {
        flow_id: flow_id.to_string(),
        step: LoginStep::Password.as_str().into(),
        nodes: password_step_nodes(&req.identifier),
        redirect_uri: None,
        branding: Some(default_branding()),
        ..Default::default()
    };

    if needs_captcha {
        bot::append_captcha_nodes(&mut resp, data, &state.pow_secret);
    }

    Json(resp).into_response()
}

pub(crate) async fn handle_password_step(
    state: &LoginState,
    flow_id: &str,
    req: &FlowSubmitRequest,
    data: &serde_json::Value,
    flow_redirect: &str,
) -> Response {
    let instance_id = current_instance_id();
    let identifier = data
        .get("identifier")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    let user = match state
        .stateful
        .find_active_user_by_identifier(&instance_id, identifier)
        .await
    {
        Ok(user) => user,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, {
                tracing::error!(%e, "lookup user");
                Json(serde_json::json!({"error": "internal error"}))
            })
                .into_response();
        }
    };

    let user = match user {
        Some(user) => user,
        None => {
            return Json(FlowStepResponse {
                flow_id: flow_id.to_string(),
                step: LoginStep::Password.as_str().into(),
                nodes: {
                    let mut n = vec![UINode::Error {
                        message: "Invalid credentials".into(),
                    }];
                    n.extend(password_step_nodes(identifier));
                    n
                },
                redirect_uri: None,
                branding: Some(default_branding()),
                ..Default::default()
            })
            .into_response();
        }
    };

    // Verify password and transparently migrate hash if needed.
    let ctx = login_actor_context();
    if let Err(resp) = password::verify_and_migrate(
        state,
        &ctx,
        &user.user_id,
        &req.password,
        identifier,
        flow_id,
    )
    .await
    {
        return resp;
    }

    let auth_request_id = data
        .get("auth_request_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    // Issue a new session via the use case instead of direct transient storage.
    let created_session = match state
        .app
        .issue_session
        .execute(
            &ctx,
            IssueSessionCommand {
                user_id: user.user_id.clone(),
                auth_method: "password".to_string(),
                user_agent: String::new(),
                ip_address: String::new(),
                fingerprint: String::new(),
            },
        )
        .await
    {
        Ok(session) => session,
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, {
                tracing::error!(%e, "session creation");
                Json(serde_json::json!({"error": "internal error"}))
            })
                .into_response();
        }
    };

    // Complete OIDC auth request if present, or use flow redirect.
    let redirect_uri = match oidc_completion::complete_auth_request(
        state,
        &instance_id,
        flow_redirect,
        &user.user_id,
        &created_session.session_id,
        None,
        auth_request_id,
    )
    .await
    {
        Ok((uri, _code)) => uri,
        Err(resp) => return resp,
    };

    match state
        .transient
        .complete_login_flow(&instance_id, flow_id)
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
            return (StatusCode::INTERNAL_SERVER_ERROR, {
                tracing::error!(%e, "complete flow");
                Json(serde_json::json!({"error": "internal error"}))
            })
                .into_response();
        }
    }

    let cookie_value = cookie::build_session_cookie(&state.cookie_config, &created_session.token);

    let mut response = Json(FlowStepResponse {
        flow_id: flow_id.to_string(),
        step: LoginStep::Complete.as_str().into(),
        nodes: vec![UINode::Heading {
            text: "Login successful".into(),
        }],
        redirect_uri: Some(redirect_uri),
        branding: Some(default_branding()),
        ..Default::default()
    })
    .into_response();

    if let Ok(header_value) = cookie_value.parse() {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, header_value);
    }

    response
}

/// Handle "fingerprint_submit" action: store the device fingerprint in flow data.
async fn handle_fingerprint_submit(
    state: &LoginState,
    flow_id: &str,
    flow: &LoginFlowRuntimeState,
    req: &FlowSubmitRequest,
) -> Response {
    let instance_id = current_instance_id();
    let visitor_id = req
        ._extra
        .get("visitor_id")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    if visitor_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "visitor_id is required"})),
        )
            .into_response();
    }

    let mut data = flow.data.clone();
    data["fingerprint"] = serde_json::Value::String(visitor_id.to_string());

    if let Err(e) = state
        .transient
        .update_login_flow_data(&instance_id, flow_id, &data)
        .await
    {
        return (StatusCode::INTERNAL_SERVER_ERROR, {
            tracing::error!(%e, "update flow data");
            Json(serde_json::json!({"error": "internal error"}))
        })
            .into_response();
    }

    // Return the current step unchanged — fingerprint collection is invisible to the user.
    let nodes = match flow.step.as_str() {
        "password" => {
            let identifier = data
                .get("identifier")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            password_step_nodes(identifier)
        }
        _ => identifier_step_nodes(),
    };
    Json(FlowStepResponse::new(
        flow_id.to_string(),
        flow.step.clone(),
        nodes,
    ))
    .into_response()
}

/// Handle "captcha_submit" action: delegates to bot::verify_captcha.
async fn handle_captcha_submit(
    state: &LoginState,
    flow_id: &str,
    flow: &LoginFlowRuntimeState,
    req: &FlowSubmitRequest,
) -> Response {
    bot::verify_captcha(state, flow_id, flow, req).await
}

/// Handle "use_session" action: reuse the existing trusted session to complete OIDC.
pub(crate) async fn handle_use_session(
    state: &LoginState,
    flow_id: &str,
    data: &serde_json::Value,
    headers: &axum::http::HeaderMap,
) -> Response {
    let instance_id = current_instance_id();
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
        .map(|session| session.user_id == trusted_user_id)
        .unwrap_or(false);

    if trusted_user_id.is_empty() || !session_valid {
        return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: LoginStep::Identifier.as_str().into(),
            nodes: {
                let mut n = vec![UINode::Error {
                    message: "Session no longer available. Please sign in again.".into(),
                }];
                n.extend(identifier_step_nodes());
                n
            },
            redirect_uri: None,
            branding: Some(default_branding()),
            ..Default::default()
        })
        .into_response();
    }

    if auth_request_id.is_empty() {
        // No OIDC request — just redirect to console.
        return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: LoginStep::Complete.as_str().into(),
            nodes: vec![UINode::Heading {
                text: "Already signed in".into(),
            }],
            redirect_uri: Some("/console".into()),
            branding: Some(default_branding()),
            ..Default::default()
        })
        .into_response();
    }

    let requirements = state
        .transient
        .load_auth_request_prompts(&instance_id, auth_request_id)
        .await
        .unwrap_or_default();
    let current_user = current_user.expect("validated current session");
    if !session_satisfies_max_age(
        current_user.authenticated_at_epoch,
        requirements.max_age,
        now_epoch_seconds(),
    ) {
        return Json(FlowStepResponse {
            flow_id: flow_id.to_string(),
            step: LoginStep::Identifier.as_str().into(),
            nodes: {
                let mut n = vec![UINode::Error {
                    message: "A fresh sign-in is required.".into(),
                }];
                n.extend(identifier_step_nodes());
                n
            },
            redirect_uri: None,
            branding: Some(default_branding()),
            ..Default::default()
        })
        .into_response();
    }

    // Complete the OIDC auth request with the trusted user.
    let redirect = match oidc_completion::complete_auth_request(
        state,
        &instance_id,
        "",
        trusted_user_id,
        &current_user.session_id,
        Some(&current_user.authenticated_at),
        auth_request_id,
    )
    .await
    {
        Ok((uri, _code)) => uri,
        Err(resp) => return resp,
    };

    Json(FlowStepResponse {
        flow_id: flow_id.to_string(),
        step: LoginStep::Complete.as_str().into(),
        nodes: vec![UINode::Heading {
            text: "Redirecting...".into(),
        }],
        redirect_uri: Some(redirect),
        branding: Some(default_branding()),
        ..Default::default()
    })
    .into_response()
}
