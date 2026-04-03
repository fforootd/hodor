use axum::{
    Json,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_db::DEFAULT_INSTANCE_ID;
use zitadel_storage::{LoginFlowRuntimeState, NewLoginFlowState};

use crate::LoginState;
use crate::redirect::{build_auth_error_redirect, build_auth_redirect};
use crate::session::extract_session_user;
use crate::ui::{
    UINode, default_branding, identifier_step_nodes, password_step_nodes, session_reuse_nodes,
};

/// Bot protection setting loaded from the settings table.
#[derive(Debug, Clone, serde::Deserialize)]
struct BotProtectionSetting {
    #[serde(default)]
    mode: String,
    #[serde(default = "default_threshold")]
    risk_threshold: f64,
    #[serde(default = "default_action")]
    action: String,
    #[serde(default = "default_provider")]
    provider: String,
    #[serde(default)]
    provider_config: serde_json::Value,
}

fn default_threshold() -> f64 {
    0.5
}
fn default_action() -> String {
    "challenge".into()
}
fn default_provider() -> String {
    "pow".into()
}

impl Default for BotProtectionSetting {
    fn default() -> Self {
        Self {
            mode: "disabled".into(),
            risk_threshold: 0.5,
            action: "challenge".into(),
            provider: "pow".into(),
            provider_config: serde_json::Value::Object(Default::default()),
        }
    }
}

#[allow(dead_code)]
impl BotProtectionSetting {
    fn is_disabled(&self) -> bool {
        self.mode.is_empty() || self.mode == "disabled"
    }
    fn is_observe(&self) -> bool {
        self.mode == "observe"
    }
    fn is_enforce(&self) -> bool {
        self.mode == "enforce"
    }
}

/// Load bot protection setting from the settings table.
async fn load_bot_protection(pool: &sqlx::AnyPool, instance_id: &str) -> BotProtectionSetting {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT data FROM settings WHERE instance_id = $1 AND type = 'bot_protection' LIMIT 1",
    )
    .bind(instance_id)
    .fetch_optional(pool)
    .await
    .unwrap_or(None);

    match row {
        Some((data,)) => serde_json::from_str(&data).unwrap_or_default(),
        None => BotProtectionSetting::default(),
    }
}

/// Emit a bot_detection event to the events table.
async fn emit_bot_detection_event(
    pool: &sqlx::AnyPool,
    instance_id: &str,
    flow_id: &str,
    fingerprint: &str,
    risk: &zitadel_botdetect::RiskScore,
    bp: &BotProtectionSetting,
    action_taken: &str,
) {
    let event_id = Uuid::new_v4().to_string();
    let payload = serde_json::json!({
        "risk_score": risk.score,
        "signals": risk.signals,
        "recommendation": format!("{:?}", risk.recommendation),
        "action_taken": action_taken,
        "provider": bp.provider,
    });
    let metadata = serde_json::json!({
        "mode": bp.mode,
        "threshold": bp.risk_threshold,
    });
    let _ = sqlx::query(
        "INSERT INTO events (id, instance_id, event_type, category, flow_id, fingerprint, payload, metadata) \
         VALUES ($1, $2, 'bot_detection', 'security', $3, $4, $5, $6)",
    )
    .bind(&event_id)
    .bind(instance_id)
    .bind(flow_id)
    .bind(fingerprint)
    .bind(serde_json::to_string(&payload).unwrap_or_default())
    .bind(serde_json::to_string(&metadata).unwrap_or_default())
    .execute(pool)
    .await;
}

/// Extract bot-detection signals from HTTP request headers.
fn extract_request_signals(headers: &axum::http::HeaderMap) -> zitadel_botdetect::RequestSignals {
    let header_keys: Vec<&str> = headers.keys().map(|k| k.as_str()).collect();
    let auth_header = headers.get("authorization").and_then(|v| v.to_str().ok());
    zitadel_botdetect::RequestSignals {
        header_order_hash: zitadel_botdetect::signals::hash_header_order(&header_keys),
        accept_language: headers
            .get("accept-language")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string(),
        accept_encoding: headers
            .get("accept-encoding")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string(),
        user_agent: headers
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default()
            .to_string(),
        http_version: String::new(), // not available from HeaderMap alone
        has_private_access_token: zitadel_botdetect::has_private_access_token(auth_header),
        ..Default::default()
    }
}

/// Build a POW challenge node for the login flow.
fn build_challenge_node(secret: &str, risk_score: f64) -> UINode {
    let difficulty = zitadel_botdetect::Difficulty::from_risk_score(risk_score);
    let challenge = zitadel_botdetect::generate_challenge(secret.as_bytes(), difficulty);
    UINode::CaptchaChallenge {
        algorithm: challenge.algorithm,
        salt: challenge.salt,
        challenge: challenge.challenge,
        maxnumber: challenge.maxnumber,
        signature: challenge.signature,
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
    fn new(flow_id: String, step: String, nodes: Vec<UINode>) -> Self {
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
            } else if allow_reuse {
                // Session exists, prompt allows reuse: show session_reuse step.
                (
                    LoginStep::SessionReuse,
                    session_reuse_nodes(identifier, display_name),
                )
            } else {
                // prompt=login: force fresh login.
                (LoginStep::Identifier, identifier_step_nodes())
            }
        } else {
            // No existing session.
            if prompts.contains(&"none".to_string()) {
                if !req.auth_request_id.is_empty()
                    && let Ok(Some(auth_req)) = state
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
                        "error_description": "prompt=none requires an existing session",
                    })),
                )
                    .into_response();
            }
            (LoginStep::Identifier, identifier_step_nodes())
        };

    let initial_step_str = initial_step.as_str().to_string();

    // Load bot protection setting from DB.
    let bp = load_bot_protection(state.db.pool(), DEFAULT_INSTANCE_ID).await;

    // Conditionally score request based on bot protection mode.
    let (risk_score, risk_signals) = if !bp.is_disabled() {
        let signals = extract_request_signals(&headers);
        let risk = zitadel_botdetect::score_request(&signals);
        tracing::debug!(
            mode = bp.mode,
            risk_score = risk.score,
            signals = ?risk.signals,
            recommendation = ?risk.recommendation,
            "login flow risk assessment"
        );

        // Emit bot_detection event in observe + enforce modes.
        let action_taken = if bp.is_observe() {
            "observe"
        } else if risk.score >= bp.risk_threshold {
            &bp.action
        } else {
            "allow"
        };
        emit_bot_detection_event(
            state.db.pool(),
            DEFAULT_INSTANCE_ID,
            &flow_id,
            &req.fingerprint,
            &risk,
            &bp,
            action_taken,
        )
        .await;

        (risk.score, risk.signals)
    } else {
        (0.0, vec![])
    };

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
                .set_login_flow_step(
                    DEFAULT_INSTANCE_ID,
                    &flow_id,
                    LoginStep::Identifier.as_str(),
                )
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

    // Check if captcha is required based on bot protection setting + risk score.
    let bp_mode = data
        .get("bot_protection_mode")
        .and_then(|v| v.as_str())
        .unwrap_or("disabled");
    let risk_score = data
        .get("risk_score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let threshold = data
        .get("bot_protection_threshold")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    let bp_action = data
        .get("bot_protection_action")
        .and_then(|v| v.as_str())
        .unwrap_or("challenge");
    let bp_provider = data
        .get("bot_protection_provider")
        .and_then(|v| v.as_str())
        .unwrap_or("pow");
    let captcha_verified = data
        .get("captcha_verified")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let needs_captcha = bp_mode == "enforce"
        && risk_score >= threshold
        && !captcha_verified
        && bp_action == "challenge";

    // Block mode: reject outright if score exceeds threshold.
    if bp_mode == "enforce" && risk_score >= threshold && bp_action == "block" {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "error": "request_blocked",
                "error_description": "Request blocked by bot protection",
            })),
        )
            .into_response();
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
        resp.captcha_required = Some(true);
        match bp_provider {
            "pow" => {
                resp.nodes
                    .push(build_challenge_node(&state.pow_secret, risk_score));
            }
            provider @ ("recaptcha" | "hcaptcha" | "turnstile") => {
                // Load site_key from provider_config stored in flow data.
                let site_key = data
                    .get("bot_protection_provider_config")
                    .and_then(|c| c.get("site_key"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default();
                resp.nodes.push(UINode::CaptchaWidget {
                    provider: provider.into(),
                    site_key: site_key.into(),
                });
            }
            _ => {}
        }
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

    let verify_result = match hash.as_deref() {
        Some(h) => state.passwords.verify(h, &req.password).ok(),
        None => None,
    };

    let verify_result = match verify_result {
        Some(result) => result,
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

    // Transparent hash migration: if the stored hash uses an outdated algorithm,
    // re-hash and persist the updated credential.
    if let zitadel_authn::password::VerifyResult::NeedUpdate(new_hash) = verify_result {
        let scoped = state.db.scoped_default();
        let cred_json = zitadel_authn::password::encode_credential_json(&new_hash);
        let sql = format!(
            "UPDATE credentials SET data = {} WHERE instance_id = $1 AND user_id = $2 AND type = 'password'",
            scoped.json_bind(3),
        );
        let _ = sqlx::query(&sql)
            .bind(scoped.instance_id())
            .bind(&user.user_id)
            .bind(&cred_json)
            .execute(scoped.pool())
            .await;
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

    let fingerprint = data
        .get("fingerprint")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    let created_session = match state
        .transient
        .create_session(
            DEFAULT_INSTANCE_ID,
            &user.user_id,
            &user.org_id,
            "",
            "",
            fingerprint,
        )
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
        zitadel_authn::cookie::sign(&created_session.token, &state.cookie_config.secrets[0]);
    let cookie_name = state.cookie_config.cookie_name();
    let secure_flag = if state.cookie_config.secure {
        "; Secure"
    } else {
        ""
    };
    let cookie_value = format!(
        "{cookie_name}={signed}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{secure_flag}",
        state.cookie_config.max_age,
    );

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
        .update_login_flow_data(DEFAULT_INSTANCE_ID, flow_id, &data)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("update flow data: {e}")})),
        )
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

/// Handle "captcha_submit" action: verify POW proof and mark flow as verified.
async fn handle_captcha_submit(
    state: &LoginState,
    flow_id: &str,
    flow: &LoginFlowRuntimeState,
    req: &FlowSubmitRequest,
) -> Response {
    // Accept altcha_payload (PoW solution).
    let altcha = req._extra.get("altcha_payload");
    let has_token = req
        ._extra
        .get("captcha_token")
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.is_empty());

    if altcha.is_none() && !has_token {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "altcha_payload or captcha_token is required"})),
        )
            .into_response();
    }

    // Verify the POW solution using HMAC + SHA-256.
    if let Some(payload) = altcha {
        let solution: zitadel_botdetect::Solution = match serde_json::from_value(payload.clone()) {
            Ok(s) => s,
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": format!("invalid altcha_payload: {e}")})),
                )
                    .into_response();
            }
        };

        // Use the server's cookie secret as the HMAC key for POW challenges.
        let secret_key = state.pow_secret.as_bytes();
        if !zitadel_botdetect::verify_solution(secret_key, &solution) {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "invalid proof-of-work solution"})),
            )
                .into_response();
        }
    }

    // Third-party captcha provider verification.
    if has_token {
        let token = req
            ._extra
            .get("captcha_token")
            .and_then(|v| v.as_str())
            .unwrap_or_default();
        let provider = flow
            .data
            .get("bot_protection_provider")
            .and_then(|v| v.as_str())
            .unwrap_or("pow");
        let secret_key = flow
            .data
            .get("bot_protection_provider_config")
            .and_then(|c| c.get("secret_key"))
            .and_then(|v| v.as_str())
            .unwrap_or_default();

        if matches!(provider, "recaptcha" | "hcaptcha" | "turnstile") && !secret_key.is_empty() {
            let verify_url = match provider {
                "recaptcha" => "https://www.google.com/recaptcha/api/siteverify",
                "hcaptcha" => "https://api.hcaptcha.com/siteverify",
                "turnstile" => "https://challenges.cloudflare.com/turnstile/v0/siteverify",
                _ => "",
            };
            if !verify_url.is_empty() {
                let client = reqwest::Client::new();
                let resp = client
                    .post(verify_url)
                    .form(&[("secret", secret_key), ("response", token)])
                    .send()
                    .await;
                let verified: bool = match resp {
                    Ok(r) => {
                        let body: serde_json::Value = r.json().await.unwrap_or_default();
                        body.get("success")
                            .and_then(|s| s.as_bool())
                            .unwrap_or(false)
                    }
                    Err(e) => {
                        tracing::warn!(provider, %e, "captcha provider verification failed");
                        false
                    }
                };
                if !verified {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({"error": "captcha verification failed"})),
                    )
                        .into_response();
                }
            }
        }
    }

    let mut data = flow.data.clone();
    data["captcha_verified"] = serde_json::Value::Bool(true);

    if let Err(e) = state
        .transient
        .update_login_flow_data(DEFAULT_INSTANCE_ID, flow_id, &data)
        .await
    {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("update flow data: {e}")})),
        )
            .into_response();
    }

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
    let mut resp = FlowStepResponse::new(flow_id.to_string(), flow.step.clone(), nodes);
    resp.captcha_verified = Some(true);
    Json(resp).into_response()
}

/// Handle "use_session" action: reuse the existing trusted session to complete OIDC.
pub(crate) async fn handle_use_session(
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
