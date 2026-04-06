use axum::{Json, http::StatusCode, response::IntoResponse, response::Response};
use zitadel_app::repo::Repositories;
use zitadel_db::current_instance_id;
use zitadel_storage::LoginFlowRuntimeState;

use crate::LoginState;
use crate::steps::FlowStepResponse;
use crate::ui::{UINode, identifier_step_nodes, password_step_nodes};

/// Bot protection setting loaded from the settings table.
#[derive(Debug, Clone, serde::Deserialize)]
pub(crate) struct BotProtectionSetting {
    #[serde(default)]
    pub mode: String,
    #[serde(default = "default_threshold")]
    pub risk_threshold: f64,
    #[serde(default = "default_action")]
    pub action: String,
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default)]
    pub provider_config: serde_json::Value,
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
    pub fn is_disabled(&self) -> bool {
        self.mode.is_empty() || self.mode == "disabled"
    }
    pub fn is_observe(&self) -> bool {
        self.mode == "observe"
    }
    pub fn is_enforce(&self) -> bool {
        self.mode == "enforce"
    }
}

/// Load bot protection setting from the settings table.
pub(crate) async fn load_bot_protection(
    repos: &Repositories,
    instance_id: &str,
) -> BotProtectionSetting {
    match repos.settings.get(instance_id, "bot_protection", "").await {
        Ok(Some(record)) => serde_json::from_value(record.data).unwrap_or_default(),
        Ok(None) | Err(_) => BotProtectionSetting::default(),
    }
}

/// Emit a bot_detection event to the events table.
pub(crate) async fn emit_bot_detection_event(
    repos: &Repositories,
    instance_id: &str,
    flow_id: &str,
    fingerprint: &str,
    risk: &zitadel_botdetect::RiskScore,
    bp: &BotProtectionSetting,
    action_taken: &str,
) {
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
    let event = zitadel_app::DomainEvent::BotDetection {
        fingerprint: fingerprint.to_string(),
        payload,
        metadata,
    };
    let _ = repos
        .events
        .append(instance_id, &event, None, None, Some(flow_id))
        .await;
}

/// Extract bot-detection signals from HTTP request headers.
pub(crate) fn extract_request_signals(
    headers: &axum::http::HeaderMap,
) -> zitadel_botdetect::RequestSignals {
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
pub(crate) fn build_challenge_node(secret: &str, risk_score: f64) -> UINode {
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

/// Score an incoming request for bot-like behaviour and build corresponding
/// flow data fields. Returns `(risk_score, risk_signals)`.
pub(crate) async fn score_and_record(
    repos: &Repositories,
    instance_id: &str,
    flow_id: &str,
    fingerprint: &str,
    headers: &axum::http::HeaderMap,
    bp: &BotProtectionSetting,
) -> (f64, Vec<String>) {
    if bp.is_disabled() {
        return (0.0, vec![]);
    }

    let signals = extract_request_signals(headers);
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
    emit_bot_detection_event(repos, instance_id, flow_id, fingerprint, &risk, bp, action_taken)
        .await;

    (risk.score, risk.signals)
}

/// Check whether bot protection enforcement should block or challenge the
/// request. Returns `Some(Response)` if the request should be blocked outright,
/// or `None` to let it proceed (challenge nodes are appended separately).
pub(crate) fn check_bot_enforcement(
    data: &serde_json::Value,
) -> (bool, Option<Response>) {
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
            needs_captcha,
            Some(
                (
                    StatusCode::FORBIDDEN,
                    Json(serde_json::json!({
                        "error": "request_blocked",
                        "error_description": "Request blocked by bot protection",
                    })),
                )
                    .into_response(),
            ),
        );
    }

    (needs_captcha, None)
}

/// Append captcha/POW challenge nodes to the response when captcha is required.
pub(crate) fn append_captcha_nodes(
    resp: &mut FlowStepResponse,
    data: &serde_json::Value,
    pow_secret: &str,
) {
    let risk_score = data
        .get("risk_score")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let bp_provider = data
        .get("bot_protection_provider")
        .and_then(|v| v.as_str())
        .unwrap_or("pow");

    resp.captcha_required = Some(true);
    match bp_provider {
        "pow" => {
            resp.nodes.push(build_challenge_node(pow_secret, risk_score));
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

/// Handle "captcha_submit" action: verify POW proof or third-party captcha
/// token, then mark the flow as verified.
pub(crate) async fn verify_captcha(
    state: &LoginState,
    flow_id: &str,
    flow: &LoginFlowRuntimeState,
    req: &crate::steps::FlowSubmitRequest,
) -> Response {
    let instance_id = current_instance_id();
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
        .update_login_flow_data(&instance_id, flow_id, &data)
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
