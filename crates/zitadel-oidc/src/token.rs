use crate::{
    OidcState,
    op::{EndSessionRequest, TokenExchangeRequest, resolve_client_auth},
    protocol_error_response,
};
use axum::{
    Form, Json, Router,
    extract::{Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use zitadel_db::current_instance_id;

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .route("/oauth/token", post(token_endpoint))
        .route("/revoke", post(revoke_endpoint))
        .route("/end_session", get(end_session_get).post(end_session_post))
        .with_state(state)
}

#[derive(Deserialize)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
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

async fn token_endpoint(
    State(state): State<OidcState>,
    headers: HeaderMap,
    Form(req): Form<TokenRequest>,
) -> Response {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let client_auth = match resolve_client_auth(authorization, &req.client_id, &req.client_secret) {
        Ok(auth) => auth,
        Err(error) => return add_token_cache_headers(protocol_error_response(error)),
    };

    match state
        .provider
        .token(&TokenExchangeRequest {
            grant_type: req.grant_type,
            code: req.code,
            redirect_uri: req.redirect_uri,
            client_auth,
            code_verifier: req.code_verifier,
            refresh_token: req.refresh_token,
        })
        .await
    {
        Ok(token) => {
            add_token_cache_headers(Json::<crate::oidc::TokenResponse>(token).into_response())
        }
        Err(error) => add_token_cache_headers(protocol_error_response(error)),
    }
}

fn add_token_cache_headers(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
        .headers_mut()
        .insert(header::PRAGMA, HeaderValue::from_static("no-cache"));
    response
}

#[derive(Deserialize)]
pub struct RevokeRequest {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    pub token: String,
}

async fn revoke_endpoint(
    State(state): State<OidcState>,
    headers: HeaderMap,
    Form(req): Form<RevokeRequest>,
) -> Response {
    let authorization = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    let client_auth = match resolve_client_auth(authorization, &req.client_id, &req.client_secret) {
        Ok(auth) => auth,
        Err(error) => return add_token_cache_headers(protocol_error_response(error)),
    };
    match state
        .provider
        .revoke(&req.token, client_auth.as_ref())
        .await
    {
        Ok(()) => add_token_cache_headers(StatusCode::OK.into_response()),
        Err(error) => add_token_cache_headers(protocol_error_response(error)),
    }
}

#[derive(Deserialize, Default)]
pub struct EndSessionParams {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub id_token_hint: String,
    #[serde(default)]
    pub post_logout_redirect_uri: String,
    #[serde(default)]
    pub state: String,
}

async fn end_session_get(
    State(state): State<OidcState>,
    headers: HeaderMap,
    Query(req): Query<EndSessionParams>,
) -> Response {
    end_session(state, headers, req).await
}

async fn end_session_post(
    State(state): State<OidcState>,
    headers: HeaderMap,
    Form(req): Form<EndSessionParams>,
) -> Response {
    end_session(state, headers, req).await
}

async fn end_session(state: OidcState, headers: HeaderMap, req: EndSessionParams) -> Response {
    let current_session_id = current_session_id(&state, &headers).await;
    let request = EndSessionRequest {
        client_id: req.client_id,
        id_token_hint: req.id_token_hint,
        post_logout_redirect_uri: req.post_logout_redirect_uri,
        state: req.state,
    };

    let outcome = match state
        .provider
        .end_session(&request, current_session_id.as_deref())
        .await
    {
        Ok(outcome) => outcome,
        Err(error) => return protocol_error_response(error),
    };

    if let Some(session_id) = outcome.session_id.as_deref() {
        if let Some(transient) = state.transient.as_ref()
            && let Err(error) = transient
                .revoke_session(&current_instance_id(), session_id)
                .await
        {
            return protocol_error_response(crate::oidc::ProtocolError::server_error(format!(
                "revoke session: {error}"
            )));
        }
        if let Err(error) = state.provider.revoke_session_tokens(session_id).await {
            return protocol_error_response(error);
        }
    }

    redirect_with_cleared_session(&outcome.redirect_uri, state.cookie_config.as_deref())
}

async fn current_session_id(state: &OidcState, headers: &HeaderMap) -> Option<String> {
    let transient = state.transient.as_ref()?;
    let cookie_config = state.cookie_config.as_ref()?;
    let cookie_header = headers.get(header::COOKIE)?.to_str().ok()?;

    for cookie_name in cookie_config.all_cookie_names() {
        for part in cookie_header.split(';').map(str::trim) {
            if let Some(value) = part.strip_prefix(&format!("{cookie_name}=")) {
                let token = zitadel_authn::cookie::verify(value, &cookie_config.secrets)?;
                let session = transient
                    .find_session_by_token(&current_instance_id(), &token)
                    .await
                    .ok()??;
                return Some(session.id);
            }
        }
    }

    None
}

fn redirect_with_cleared_session(
    location: &str,
    cookie_config: Option<&zitadel_authn::cookie::CookieConfig>,
) -> Response {
    let mut response = StatusCode::FOUND.into_response();
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_str(location)
            .unwrap_or_else(|_| HeaderValue::from_static("/login?logged_out=1")),
    );
    if let Some(cookie_config) = cookie_config {
        for cookie_name in cookie_config.all_cookie_names() {
            if let Ok(value) =
                HeaderValue::from_str(&expired_session_cookie(cookie_name, cookie_config.secure))
            {
                response.headers_mut().append(header::SET_COOKIE, value);
            }
        }
    }
    response
}

fn expired_session_cookie(cookie_name: &str, secure: bool) -> String {
    let mut cookie = format!(
        "{cookie_name}=; Path=/; HttpOnly; Max-Age=0; Expires=Thu, 01 Jan 1970 00:00:00 GMT; SameSite=Lax"
    );
    if secure {
        cookie.push_str("; Secure");
    }
    cookie
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_responses_include_cache_headers() {
        let response = add_token_cache_headers(StatusCode::OK.into_response());

        assert_eq!(
            response.headers().get(header::CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-store"))
        );
        assert_eq!(
            response.headers().get(header::PRAGMA),
            Some(&HeaderValue::from_static("no-cache"))
        );
    }
}
