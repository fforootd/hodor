use axum::{Json, http::StatusCode, response::IntoResponse, response::Response};
use uuid::Uuid;

use crate::LoginState;
use crate::redirect::build_auth_redirect;

/// Complete an OIDC auth request by generating a code, marking it done, and
/// returning `(redirect_uri, code)`. Returns an error `Response` on failure.
pub(crate) async fn complete_auth_request(
    state: &LoginState,
    instance_id: &str,
    flow_redirect: &str,
    user_id: &str,
    session_id: &str,
    authenticated_at: Option<&str>,
    auth_request_id: &str,
) -> Result<(String, String), Response> {
    if auth_request_id.is_empty() {
        // No OIDC request attached — use the flow-level redirect.
        let redirect = if !flow_redirect.is_empty() {
            flow_redirect.to_string()
        } else {
            "/console".to_string()
        };
        return Ok((redirect, String::new()));
    }

    let code = Uuid::new_v4().to_string();

    let auth_request = match state
        .transient
        .load_auth_request_redirect(instance_id, auth_request_id)
        .await
    {
        Ok(Some(auth_request)) => auth_request,
        Ok(None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "authorization request no longer exists"})),
            )
                .into_response());
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("load auth request: {e}")})),
            )
                .into_response());
        }
    };

    match state
        .transient
        .complete_auth_request(
            instance_id,
            auth_request_id,
            user_id,
            Some(session_id),
            &code,
            authenticated_at,
        )
        .await
    {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "authorization request no longer exists"})),
            )
                .into_response());
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("complete auth request: {e}")})),
            )
                .into_response());
        }
    }

    let redirect = build_auth_redirect(&auth_request.redirect_uri, &auth_request.state, &code);
    Ok((redirect, code))
}
