use crate::{OidcState, op::AuthorizeRequest, protocol_error_response};
use axum::{
    Router,
    body::to_bytes,
    extract::{Request, State},
    response::{IntoResponse, Redirect, Response},
    routing::get,
};
use base64::Engine;
use serde::Deserialize;
use std::borrow::Cow;

pub fn routes(state: OidcState) -> Router {
    Router::new()
        .route("/authorize", get(authorize).post(authorize))
        .with_state(state)
}

#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
pub struct AuthorizeParams {
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub response_type: Option<String>,
    pub scope: Option<String>,
    pub state: Option<String>,
    pub nonce: Option<String>,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub prompt: Option<String>,
    pub login_hint: Option<String>,
    pub request: Option<String>,
    pub max_age: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct RequestObjectClaims {
    client_id: Option<String>,
    redirect_uri: Option<String>,
    response_type: Option<String>,
    scope: Option<String>,
    state: Option<String>,
    nonce: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    prompt: Option<String>,
    login_hint: Option<String>,
    max_age: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct RequestObjectHeader {
    alg: String,
}

async fn authorize(State(oidc): State<OidcState>, req: Request) -> Response {
    let params = match extract_authorize_params(req).await {
        Ok(params) => params,
        Err(error) => return protocol_error_response(error),
    };

    let request = build_authorize_request(params);

    match oidc.provider.authorize(&request).await {
        Ok(redirect) => Redirect::to(&redirect.location).into_response(),
        Err(error) => match authorization_error_redirect(&oidc, &request, &error).await {
            Some(response) => response,
            None => protocol_error_response(error),
        },
    }
}

fn build_authorize_request(params: AuthorizeParams) -> AuthorizeRequest {
    let prompt_string = params.prompt.unwrap_or_default();
    let prompt: Vec<String> = prompt_string
        .split_whitespace()
        .map(str::to_string)
        .collect();

    AuthorizeRequest {
        client_id: params.client_id.unwrap_or_default(),
        redirect_uri: params.redirect_uri.unwrap_or_default(),
        response_type: params.response_type.unwrap_or_default(),
        scope: params.scope.unwrap_or_else(|| "openid".to_string()),
        state: params.state.unwrap_or_default(),
        nonce: params.nonce.unwrap_or_default(),
        code_challenge: params.code_challenge.unwrap_or_default(),
        code_challenge_method: params.code_challenge_method.unwrap_or_default(),
        prompt,
        login_hint: params.login_hint.unwrap_or_default(),
        max_age: params.max_age.and_then(|value| value.parse().ok()),
    }
}

async fn authorization_error_redirect(
    oidc: &OidcState,
    request: &AuthorizeRequest,
    error: &crate::oidc::ProtocolError,
) -> Option<Response> {
    if !matches!(
        error.body.error.as_str(),
        "invalid_request" | "unsupported_response_type"
    ) {
        return None;
    }

    if !oidc
        .provider
        .allows_authorization_error_redirect(&request.client_id, &request.redirect_uri)
        .await
    {
        return None;
    }

    let mut redirect = url::Url::parse(&request.redirect_uri).ok()?;
    {
        let mut query = redirect.query_pairs_mut();
        query.append_pair("error", &error.body.error);
        if let Some(description) = error.body.error_description.as_deref() {
            query.append_pair("error_description", description);
        }
        if !request.state.is_empty() {
            query.append_pair("state", &request.state);
        }
    }

    Some(Redirect::to(redirect.as_ref()).into_response())
}

async fn extract_authorize_params(
    req: Request,
) -> Result<AuthorizeParams, crate::oidc::ProtocolError> {
    let query_params = req
        .uri()
        .query()
        .map(parse_authorize_params)
        .unwrap_or_default();
    let body_params = if req.method() == axum::http::Method::POST {
        let body = to_bytes(req.into_body(), 64 * 1024)
            .await
            .map_err(|_| crate::oidc::ProtocolError::invalid_request("invalid authorize body"))?;
        parse_authorize_params(std::str::from_utf8(&body).unwrap_or_default())
    } else {
        AuthorizeParams::default()
    };

    let mut params = query_params;
    merge_authorize_params(&mut params, body_params);
    if let Some(request_object) = params.request.clone() {
        let request_params = decode_request_object(&request_object)?;
        merge_authorize_params(&mut params, request_params);
    }
    Ok(params)
}

pub fn parse_authorize_params(input: &str) -> AuthorizeParams {
    let mut params = AuthorizeParams::default();
    for (key, value) in url::form_urlencoded::parse(input.as_bytes()) {
        apply_param(&mut params, key, value);
    }
    params
}

fn apply_param(params: &mut AuthorizeParams, key: Cow<'_, str>, value: Cow<'_, str>) {
    let value = value.into_owned();
    match key.as_ref() {
        "client_id" => params.client_id = Some(value),
        "redirect_uri" => params.redirect_uri = Some(value),
        "response_type" => params.response_type = Some(value),
        "scope" => params.scope = Some(value),
        "state" => params.state = Some(value),
        "nonce" => params.nonce = Some(value),
        "code_challenge" => params.code_challenge = Some(value),
        "code_challenge_method" => params.code_challenge_method = Some(value),
        "prompt" => params.prompt = Some(value),
        "login_hint" => params.login_hint = Some(value),
        "request" => params.request = Some(value),
        "max_age" => params.max_age = Some(value),
        _ => {}
    }
}

fn merge_authorize_params(target: &mut AuthorizeParams, source: AuthorizeParams) {
    if source.client_id.is_some() {
        target.client_id = source.client_id;
    }
    if source.redirect_uri.is_some() {
        target.redirect_uri = source.redirect_uri;
    }
    if source.response_type.is_some() {
        target.response_type = source.response_type;
    }
    if source.scope.is_some() {
        target.scope = source.scope;
    }
    if source.state.is_some() {
        target.state = source.state;
    }
    if source.nonce.is_some() {
        target.nonce = source.nonce;
    }
    if source.code_challenge.is_some() {
        target.code_challenge = source.code_challenge;
    }
    if source.code_challenge_method.is_some() {
        target.code_challenge_method = source.code_challenge_method;
    }
    if source.prompt.is_some() {
        target.prompt = source.prompt;
    }
    if source.login_hint.is_some() {
        target.login_hint = source.login_hint;
    }
    if source.request.is_some() {
        target.request = source.request;
    }
    if source.max_age.is_some() {
        target.max_age = source.max_age;
    }
}

pub fn decode_request_object(token: &str) -> Result<AuthorizeParams, crate::oidc::ProtocolError> {
    let mut segments = token.splitn(3, '.');
    let header = segments
        .next()
        .ok_or_else(|| crate::oidc::ProtocolError::invalid_request("invalid request object"))?;
    let payload = segments
        .next()
        .ok_or_else(|| crate::oidc::ProtocolError::invalid_request("invalid request object"))?;
    let signature = segments.next().unwrap_or_default();

    let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(header)
        .map_err(|_| crate::oidc::ProtocolError::invalid_request("invalid request object"))?;
    let header: RequestObjectHeader = serde_json::from_slice(&header)
        .map_err(|_| crate::oidc::ProtocolError::invalid_request("invalid request object"))?;
    if header.alg != "none" || !signature.is_empty() {
        return Err(crate::oidc::ProtocolError::invalid_request(
            "only unsigned request objects are supported",
        ));
    }

    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| crate::oidc::ProtocolError::invalid_request("invalid request object"))?;
    let claims: RequestObjectClaims = serde_json::from_slice(&payload)
        .map_err(|_| crate::oidc::ProtocolError::invalid_request("invalid request object"))?;

    Ok(AuthorizeParams {
        client_id: claims.client_id,
        redirect_uri: claims.redirect_uri,
        response_type: claims.response_type,
        scope: claims.scope,
        state: claims.state,
        nonce: claims.nonce,
        code_challenge: claims.code_challenge,
        code_challenge_method: claims.code_challenge_method,
        prompt: claims.prompt,
        login_hint: claims.login_hint,
        request: None,
        max_age: claims.max_age.map(|value| value.to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;

    #[test]
    fn request_object_redirect_uri_overrides_outer_param() {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(
            r#"{"client_id":"client","redirect_uri":"https://good.example/callback","response_type":"code","scope":"openid","state":"state","nonce":"nonce"}"#,
        );
        let request = format!("{header}.{payload}.");

        let mut params = parse_authorize_params(
            "client_id=client&redirect_uri=https%3A%2F%2Fevil.example%2Fcallback_invalid&request=",
        );
        params.request = Some(request);
        let merged = decode_request_object(params.request.as_deref().unwrap()).unwrap();
        merge_authorize_params(&mut params, merged);

        assert_eq!(
            params.redirect_uri.as_deref(),
            Some("https://good.example/callback")
        );
    }

    #[test]
    fn max_age_is_captured_from_request_object() {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none"}"#);
        let payload = URL_SAFE_NO_PAD.encode(
            r#"{"client_id":"client","redirect_uri":"https://good.example/callback","response_type":"code","scope":"openid","state":"state","nonce":"nonce","max_age":1}"#,
        );
        let params = decode_request_object(&format!("{header}.{payload}.")).unwrap();

        assert_eq!(params.max_age.as_deref(), Some("1"));
    }

    #[test]
    fn build_authorize_request_does_not_default_response_type() {
        let request = build_authorize_request(AuthorizeParams {
            client_id: Some("client".to_string()),
            redirect_uri: Some("https://good.example/callback".to_string()),
            scope: Some("openid".to_string()),
            state: Some("state".to_string()),
            nonce: Some("nonce".to_string()),
            ..AuthorizeParams::default()
        });

        assert!(request.response_type.is_empty());
    }

    #[test]
    fn build_authorize_request_preserves_max_age_without_forcing_login_prompt() {
        let request = build_authorize_request(AuthorizeParams {
            prompt: Some("consent".to_string()),
            max_age: Some("1".to_string()),
            ..AuthorizeParams::default()
        });

        assert_eq!(request.prompt, vec!["consent".to_string()]);
        assert_eq!(request.max_age, Some(1));
    }

    #[test]
    fn authorize_redirect_uses_see_other() {
        let response = Redirect::to("/conformance/login?auth_request_id=test").into_response();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
    }

    #[tokio::test]
    async fn authorization_error_redirect_points_back_to_client() {
        let db = zitadel_db::Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let repo: std::sync::Arc<dyn zitadel_app::repo::OidcRepository> =
            std::sync::Arc::new(zitadel_db::repo_impls::DbOidcRepository::new(db.clone()));
        let oidc = OidcState::new(
            repo,
            "https://issuer.example".to_string(),
            "/conformance/login".to_string(),
        );

        let scoped = db.scoped_default();

        // Create the org that the app references.
        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, 'Test Org')")
            .bind("org-1")
            .bind(scoped.instance_id())
            .execute(scoped.pool())
            .await
            .unwrap();

        let sql = format!(
            "INSERT INTO apps (id, instance_id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, {}, {}, $11)",
            scoped.json_bind(8),
            scoped.json_bind(9),
            scoped.json_bind(10),
        );
        sqlx::query(&sql)
            .bind("app-1")
            .bind(scoped.instance_id())
            .bind("org-1")
            .bind("Example App")
            .bind("web")
            .bind("client")
            .bind("secret")
            .bind(r#"["https://client.example/callback"]"#)
            .bind(r#"["authorization_code"]"#)
            .bind(r#"["code"]"#)
            .bind("active")
            .execute(scoped.pool())
            .await
            .unwrap();

        let response = authorization_error_redirect(
            &oidc,
            &AuthorizeRequest {
                client_id: "client".to_string(),
                redirect_uri: "https://client.example/callback".to_string(),
                response_type: String::new(),
                scope: "openid".to_string(),
                state: "state-123".to_string(),
                nonce: String::new(),
                code_challenge: String::new(),
                code_challenge_method: String::new(),
                prompt: Vec::new(),
                login_hint: String::new(),
                max_age: None,
            },
            &crate::oidc::ProtocolError::unsupported_response_type("missing response_type"),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get("location").unwrap(),
            "https://client.example/callback?error=unsupported_response_type&error_description=missing+response_type&state=state-123"
        );
    }
}
