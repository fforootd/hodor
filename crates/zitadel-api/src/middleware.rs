use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    middleware::Next,
    response::Response,
};
use tracing::Span;
use zitadel_db::current_instance_id;

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
    if let Some(auth) = req.headers().get(header::AUTHORIZATION)
        && let Ok(val) = auth.to_str()
        && let Some(token) = val.strip_prefix("Bearer ")
    {
        return Some(token.to_string());
    }

    // 2. Session cookie (HMAC-verified).
    let cookie_header = req.headers().get(header::COOKIE)?.to_str().ok()?;
    for name in state.cookie_config.all_cookie_names() {
        for part in cookie_header.split(';') {
            let part = part.trim();
            if let Some(value) = part.strip_prefix(name).and_then(|s| s.strip_prefix('='))
                && let Some(token) =
                    zitadel_authn::cookie::verify(value, &state.cookie_config.secrets)
            {
                return Some(token);
            }
        }
    }

    None
}

/// Resolve a raw token (PAT or session token) to an Identity.
async fn resolve_token(state: &ApiState, raw_token: &str) -> anyhow::Result<Option<Identity>> {
    let instance_id = current_instance_id();

    if let Some(identity) = state
        .stateful
        .resolve_pat_token(&instance_id, raw_token)
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
        .find_session_by_token(&instance_id, raw_token)
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use axum::{Router, body::to_bytes, http::Request};
    use tower::util::ServiceExt;
    use uuid::Uuid;
    use zitadel_authn::{
        cookie::{CookieConfig, sign},
        password::{Swapper, encode_credential_json},
        session::hash_token,
    };
    use zitadel_config::{Config, password::PasswordHasherConfig};
    use zitadel_db::{DEFAULT_INSTANCE_ID, Db};
    use zitadel_fga::{FgaService, StoreResolver};
    use zitadel_storage::StorageRuntime;

    async fn test_state() -> ApiState {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db).await.unwrap();

        let mut config = Config::default();
        config.server.public_origin = "http://localhost:18080".into();
        config.server.force_insecure_cookies = true;
        config.password_hasher = PasswordHasherConfig::dev_defaults();

        let cookie_config = Arc::new(CookieConfig::new_with_max_age(
            vec!["test-secret".into()],
            &config.server.external_domain,
            config.server.force_insecure_cookies,
            config.session.max_age_secs as i64,
        ));
        let storage =
            StorageRuntime::from_config(&config.storage, db.clone(), config.session.max_age_secs)
                .await
                .unwrap();
        let fga = Arc::new(FgaService::new(db.clone()));
        fga.initialize_instance(DEFAULT_INSTANCE_ID).await.unwrap();
        let oidc = zitadel_oidc::OidcState::new_with_config(
            db.clone(),
            config.server.public_origin.clone(),
            "/login".into(),
            &config.oidc,
        );

        ApiState {
            db,
            fga,
            stateful: storage.stateful.clone(),
            transient: storage.transient.clone(),
            analytics: storage.analytics.clone(),
            oidc,
            passwords: Arc::new(Swapper::from_config(&config.password_hasher)),
            cookie_config,
            is_dev: true,
        }
    }

    async fn create_user(state: &ApiState, identifier: &str, password: &str) -> (String, String) {
        let scoped = state.db.scoped_default();
        let org_id: (String,) =
            sqlx::query_as("SELECT id FROM orgs WHERE instance_id = $1 LIMIT 1")
                .bind(scoped.instance_id())
                .fetch_one(scoped.pool())
                .await
                .unwrap();
        let user_id = Uuid::new_v4().to_string();
        let password_hash = state.passwords.hash(password).unwrap();
        let credential_json = encode_credential_json(&password_hash);
        let sql = format!(
            "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, $2, $3, 'password', {})",
            scoped.json_bind(4),
        );

        sqlx::query(
            "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state) \
             VALUES ($1, $2, $3, $4, $5, 'human', 'active')",
        )
        .bind(&user_id)
        .bind(scoped.instance_id())
        .bind(&org_id.0)
        .bind(identifier)
        .bind(identifier)
        .execute(scoped.pool())
        .await
        .unwrap();

        sqlx::query(&sql)
            .bind(format!("cred-{user_id}"))
            .bind(scoped.instance_id())
            .bind(&user_id)
            .bind(&credential_json)
            .execute(scoped.pool())
            .await
            .unwrap();

        (user_id, org_id.0)
    }

    async fn create_pat(state: &ApiState, user_id: &str) -> String {
        let scoped = state.db.scoped_default();
        let pat_id = Uuid::new_v4().to_string();
        let token = format!("zit_pat_{}", zitadel_crypto::random_hex(24));
        let token_hash = hash_token(&token);
        let sql = format!(
            "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) VALUES ($1, $2, 'pat', $3, $4, $5, {})",
            scoped.json_bind(6),
        );

        sqlx::query(&sql)
            .bind(&pat_id)
            .bind(scoped.instance_id())
            .bind(&token_hash)
            .bind(user_id)
            .bind("middleware-test")
            .bind("[\"admin\"]")
            .execute(scoped.pool())
            .await
            .unwrap();

        token
    }

    #[tokio::test]
    async fn protected_routes_keep_the_uniform_401_shape_without_credentials() {
        let state = test_state().await;
        let app: Router = crate::routes(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/auth/whoami")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"error": "authentication required", "code": 401})
        );
    }

    #[tokio::test]
    async fn authorization_header_takes_precedence_over_cookie_auth() {
        let state = test_state().await;
        let (user_id, org_id) = create_user(&state, "middleware@example.com", "password123").await;
        let session = state
            .transient
            .create_session(
                DEFAULT_INSTANCE_ID,
                &user_id,
                &org_id,
                "zitadel-api-test",
                "127.0.0.1",
                "",
            )
            .await
            .unwrap();
        let pat = create_pat(&state, &user_id).await;
        let cookie = format!(
            "{}={}",
            state.cookie_config.cookie_name(),
            sign(&session.token, &state.cookie_config.secrets[0])
        );

        let app: Router = crate::routes(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/auth/whoami")
                    .header(header::AUTHORIZATION, format!("Bearer {pat}"))
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["token_type"], "pat");
    }
}
