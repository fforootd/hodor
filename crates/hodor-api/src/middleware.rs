use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::Response,
};

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
    let scoped = state.db.scoped_default();
    match resolve_token(&scoped, &raw_token).await {
        Ok(Some(identity)) => {
            req.extensions_mut().insert(identity);
            next.run(req).await
        }
        Ok(None) => {
            response::error(StatusCode::UNAUTHORIZED, "invalid or expired token")
        }
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
                if let Some(token) = hodor_auth::cookie::verify(value, &state.cookie_config.secrets) {
                    return Some(token);
                }
            }
        }
    }

    None
}

/// Resolve a raw token (PAT or session token) to an Identity.
async fn resolve_token(
    scoped: &hodor_db::scoped::ScopedDb,
    raw_token: &str,
) -> anyhow::Result<Option<Identity>> {
    let token_hash = hodor_auth::session::hash_token(raw_token);

    // Check tokens table (PATs and session tokens).
    let row: Option<(String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT t.user_id, t.type, t.session_id, COALESCE(u.org_id, '') \
         FROM tokens t \
         JOIN users u ON u.id = t.user_id AND u.instance_id = t.instance_id \
         WHERE t.instance_id = ? AND t.token_hash = ? AND t.revoked_at IS NULL",
    )
    .bind(scoped.instance_id())
    .bind(&token_hash)
    .fetch_optional(scoped.pool())
    .await?;

    if let Some((user_id, token_type, session_id, org_id)) = row {
        return Ok(Some(Identity {
            user_id,
            session_id: session_id.unwrap_or_default(),
            token_type,
            org_id,
        }));
    }

    // Check sessions table (direct session token lookup).
    let row: Option<(String, String, String)> = sqlx::query_as(
        "SELECT s.id, s.user_id, COALESCE(s.org_id, '') \
         FROM sessions s \
         WHERE s.instance_id = ? AND s.token_hash = ? AND s.revoked_at IS NULL",
    )
    .bind(scoped.instance_id())
    .bind(&token_hash)
    .fetch_optional(scoped.pool())
    .await?;

    if let Some((session_id, user_id, org_id)) = row {
        return Ok(Some(Identity {
            user_id,
            session_id,
            token_type: "session".to_string(),
            org_id,
        }));
    }

    Ok(None)
}

/// Extract the Identity from request extensions (set by auth_gate).
pub fn identity_from_request(req: &Request<Body>) -> Option<&Identity> {
    req.extensions().get::<Identity>()
}
