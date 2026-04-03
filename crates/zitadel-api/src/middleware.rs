use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::Response,
};
use tracing::Span;
use zitadel_db::DEFAULT_INSTANCE_ID;

use crate::ApiState;
use crate::response;

/// Identity info extracted from token resolution.
#[derive(Clone, Debug)]
pub struct Identity {
    pub user_id: String,
    pub session_id: String,
    pub token_type: String,
    pub org_id: String,
}

/// AuthGate middleware — validates Bearer token or session cookie.
/// Injects `Identity` into request extensions on success.
/// Public routes (healthz, readyz, OIDC discovery) bypass auth.
pub async fn auth_gate(
    State(state): State<ApiState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    // Extract token from Authorization header or cookie.
    let raw_token = extract_token(&req, &state);

    let raw_token = match raw_token {
        Some(t) => t,
        None => {
            return response::error(StatusCode::UNAUTHORIZED, "authentication required");
        }
    };

    // Resolve token against database.
    match resolve_token(&state, &raw_token).await {
        Ok(Some(identity)) => {
            let span = Span::current();
            span.record("actor_id", tracing::field::display(&identity.user_id));
            span.record("session_id", tracing::field::display(&identity.session_id));
            span.record("org_id", tracing::field::display(&identity.org_id));
            req.extensions_mut().insert(identity);
            next.run(req).await
        }
        Ok(None) => response::error(StatusCode::UNAUTHORIZED, "invalid or expired token"),
        Err(e) => {
            tracing::error!(error = %e, "token resolution failed");
            response::error(StatusCode::INTERNAL_SERVER_ERROR, "authentication error")
        }
    }
}

fn extract_token(req: &Request<Body>, state: &ApiState) -> Option<String> {
    // 1. Authorization: Bearer <token>
    if let Some(auth) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    // 2. Session cookie (HMAC-verified).
    let cookie_header = req.headers().get(header::COOKIE)?.to_str().ok()?;
    for name in state.cookie_config.all_cookie_names() {
        for part in cookie_header.split(';') {
            let part = part.trim();
            if let Some(value) = part.strip_prefix(name).and_then(|s| s.strip_prefix('=')) {
                if let Some(token) =
                    zitadel_authn::cookie::verify(value, &state.cookie_config.secrets)
                {
                    return Some(token);
                }
            }
        }
    }

    None
}

/// Resolve a raw token (PAT or session token) to an Identity.
async fn resolve_token(state: &ApiState, raw_token: &str) -> anyhow::Result<Option<Identity>> {
    if let Some(identity) = state
        .stateful
        .resolve_pat_token(DEFAULT_INSTANCE_ID, raw_token)
        .await?
    {
        return Ok(Some(Identity {
            user_id: identity.user_id,
            session_id: identity.session_id,
            token_type: identity.token_type,
            org_id: identity.org_id,
        }));
    }

    if let Some(session) = state
        .transient
        .find_session_by_token(DEFAULT_INSTANCE_ID, raw_token)
        .await?
    {
        return Ok(Some(Identity {
            user_id: session.user_id,
            session_id: session.id,
            token_type: "session".to_string(),
            org_id: session.org_id,
        }));
    }

    if let Ok(claims) = state.oidc.provider.validate_access_token(raw_token).await {
        let scoped = state.db.scoped_default();
        let row: Option<(String,)> =
            sqlx::query_as("SELECT org_id FROM users WHERE instance_id = $1 AND id = $2")
                .bind(scoped.instance_id())
                .bind(&claims.sub)
                .fetch_optional(scoped.pool())
                .await?;
        if let Some((org_id,)) = row {
            return Ok(Some(Identity {
                user_id: claims.sub,
                session_id: String::new(),
                token_type: "oidc".to_string(),
                org_id,
            }));
        }
    }

    Ok(None)
}

/// Extract the Identity from request extensions (set by auth_gate).
pub fn identity_from_request(req: &Request<Body>) -> Option<&Identity> {
    req.extensions().get::<Identity>()
}
