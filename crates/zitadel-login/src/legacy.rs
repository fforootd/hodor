use axum::{
    Json,
    extract::State,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_db::DEFAULT_INSTANCE_ID;

use crate::LoginState;
use crate::redirect::build_auth_redirect;

#[derive(Deserialize)]
pub(crate) struct LoginRequest {
    identifier: String,
    password: String,
    #[serde(default)]
    auth_request_id: String,
}

#[derive(Serialize)]
pub(crate) struct LoginResponse {
    session_id: String,
    token: String,
    user_id: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    redirect_uri: String,
}

pub(crate) async fn login(
    State(state): State<LoginState>,
    Json(req): Json<LoginRequest>,
) -> Response {
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
    let verify_result = match state.passwords.verify(&hash, &req.password) {
        Ok(result) => result,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({"error": "invalid credentials"})),
            )
                .into_response();
        }
    };

    // Transparent hash migration.
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
        .create_session(DEFAULT_INSTANCE_ID, &user.user_id, &user.org_id, "", "", "")
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
    if let Ok(header_value) = cookie_value.parse() {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, header_value);
    }
    response
}

pub(crate) async fn auth_settings(State(_state): State<LoginState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "password_enabled": true,
        "passkey_enabled": false,
        "external_idps": [],
        "registration_allowed": false,
    }))
}

pub(crate) async fn branding(State(_state): State<LoginState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "org_name": "Zitadel",
        "logo_url": "",
        "primary_color": "#4A90D9",
    }))
}

pub(crate) async fn captcha_challenge(
    State(_state): State<LoginState>,
    axum::extract::Path(_flow_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    // POC: no captcha required.
    Json(serde_json::json!({"required": false}))
}
