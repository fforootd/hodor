pub mod bot;
pub mod conformance;
pub mod cookie;
pub mod legacy;
pub mod oidc_completion;
pub mod password;
pub mod redirect;
pub mod session;
pub mod sso;
pub mod steps;
pub mod ui;

use axum::{
    Router,
    extract::State,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use std::borrow::Cow;
use std::sync::Arc;
use zitadel_app::ApplicationServices;
use zitadel_authn::password::Swapper;
use zitadel_db::current_request_origin_or;
use zitadel_storage::{DefaultPrimaryStorage, DefaultTransientStorage};

pub(crate) type DefaultRpService = zitadel_oidc::rp::RpService<
    zitadel_oidc::rp::ReqwestHttpClient,
    zitadel_oidc::rp::InMemoryIssuerMetadataCache,
>;

#[derive(Clone)]
pub struct LoginState {
    pub primary: Arc<DefaultPrimaryStorage>,
    pub transient: Arc<DefaultTransientStorage>,
    pub passwords: Arc<Swapper>,
    pub cookie_config: Arc<zitadel_authn::cookie::CookieConfig>,
    pub public_origin: Arc<String>,
    pub public_origin_override: Option<Arc<String>>,
    pub conformance_login_html: bool,
    pub rp: Arc<DefaultRpService>,
    /// Secret key for POW challenge HMAC signatures.
    pub pow_secret: String,
    /// Application services (ADR-032 use cases).
    pub app: Arc<ApplicationServices>,
}

impl LoginState {
    pub fn effective_public_origin(&self) -> Cow<'_, str> {
        if let Some(public_origin_override) = self.public_origin_override.as_deref() {
            Cow::Borrowed(public_origin_override.as_str())
        } else {
            current_request_origin_or(self.public_origin.as_str())
        }
    }
}

pub fn routes(state: LoginState) -> Router {
    Router::new()
        .route("/logout", get(logout))
        // Direct login (legacy/simple)
        .route("/v1/auth/login", post(legacy::login))
        .route("/v1/auth/settings", get(legacy::auth_settings))
        .route("/v1/branding", get(legacy::branding))
        .route(
            "/conformance/login",
            get(conformance::login_get).post(conformance::login_post),
        )
        // Server-driven login flow (what the login SPA actually uses)
        .route("/v1/login/flows", post(steps::flow_create))
        .route("/v1/login/flows/{id}", get(steps::flow_get))
        .route("/v1/login/flows/{id}/submit", post(steps::flow_submit))
        .route(
            "/v1/login/flows/{id}/captcha/challenge",
            get(legacy::captcha_challenge),
        )
        .merge(sso::routes())
        .with_state(state)
}

async fn logout(State(state): State<LoginState>) -> Response {
    let mut response = StatusCode::FOUND.into_response();
    response.headers_mut().insert(
        header::LOCATION,
        HeaderValue::from_static("/login?logged_out=1"),
    );

    for cookie_name in state.cookie_config.all_cookie_names() {
        if let Ok(value) = HeaderValue::from_str(&expired_session_cookie(
            cookie_name,
            state.cookie_config.secure,
        )) {
            response.headers_mut().append(header::SET_COOKIE, value);
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
    use axum::body::to_bytes;
    use axum::http::StatusCode;
    use steps::{FlowSubmitRequest, handle_identifier_step, handle_password_step};
    use tokio::time::{Duration, sleep};
    use uuid::Uuid;
    use zitadel_app::{ApplicationServices, HookPipeline};
    use zitadel_authn::{
        cookie::CookieConfig,
        password::{Swapper, encode_credential_json},
    };
    use zitadel_db::{DEFAULT_INSTANCE_ID, Db, InstanceContext, with_instance_context};
    use zitadel_fga::FgaService;
    use zitadel_storage::NewLoginFlowState;

    async fn test_state() -> LoginState {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db, None).await.unwrap();
        let mut config = zitadel_config::Config::default();
        config.server.public_origin = "http://localhost:8080".into();
        config.server.force_insecure_cookies = false;
        let storage = zitadel_storage::StorageRuntime::from_config(
            &config.storage,
            db.clone(),
            config.session.max_age_secs,
        )
        .await
        .unwrap();
        let fga = Arc::new(FgaService::new(db.clone()));
        let repos = Arc::new(zitadel_server::wiring::build_repositories(
            db.clone(),
            storage.primary.as_ref().replica_db().cloned(),
            storage.transient.clone(),
            fga,
            storage.analytics.clone(),
        ));
        let app = Arc::new(ApplicationServices::new(
            repos,
            Arc::new(HookPipeline::empty()),
            false,
        ));
        LoginState {
            primary: storage.primary.clone(),
            transient: storage.transient.clone(),
            passwords: Arc::new(Swapper::dev()),
            cookie_config: Arc::new(CookieConfig::new(
                vec!["test-secret".into()],
                "localhost",
                false,
            )),
            public_origin: Arc::new("http://localhost:8080".into()),
            public_origin_override: Some(Arc::new("http://localhost:8080".into())),
            conformance_login_html: false,
            rp: Arc::new(zitadel_oidc::rp::RpService::new(
                zitadel_oidc::rp::ReqwestHttpClient::new(),
                zitadel_oidc::rp::InMemoryIssuerMetadataCache::default(),
            )),
            pow_secret: "test-pow-secret".into(),
            app,
        }
    }

    async fn insert_user(
        state: &LoginState,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        identifier: &str,
        password: &str,
    ) {
        let scoped = state.primary.db().scoped(instance_id.to_string());
        // Ensure the instance row exists (no-op if it's "default" which is seeded by migration).
        // Managed children need (parent_instance_id, owner_org_id) pointing to an
        // existing org in the parent — bootstrap creates org "1" in "default".
        sqlx::query(
            "INSERT OR IGNORE INTO instances (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, feature_overrides) \
             VALUES ($1, 'default', '1', 'managed', 'active', 'global', '{}')",
        )
        .bind(scoped.instance_id())
        .execute(scoped.pool())
        .await
        .unwrap();
        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, $3)")
            .bind(org_id)
            .bind(scoped.instance_id())
            .bind(format!("{instance_id}-org"))
            .execute(scoped.pool())
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state) \
             VALUES ($1, $2, $3, $4, $5, 'human', 'active')",
        )
        .bind(user_id)
        .bind(scoped.instance_id())
        .bind(org_id)
        .bind(identifier)
        .bind(identifier)
        .execute(scoped.pool())
        .await
        .unwrap();

        let password_hash = state.passwords.hash(password).unwrap();
        let sql = format!(
            "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, $2, $3, 'password', {})",
            scoped.json_bind(4),
        );
        sqlx::query(&sql)
            .bind(format!("cred-{user_id}"))
            .bind(scoped.instance_id())
            .bind(user_id)
            .bind(encode_credential_json(&password_hash))
            .execute(scoped.pool())
            .await
            .unwrap();
    }

    /// Helper: create a login flow via the transient storage and return its flow_id.
    async fn create_flow(state: &LoginState) -> String {
        let flow_id = Uuid::new_v4().to_string();
        let data = serde_json::json!({"step": "identifier"});
        state
            .transient
            .create_login_flow(
                DEFAULT_INSTANCE_ID,
                &NewLoginFlowState {
                    flow_id: flow_id.clone(),
                    state: String::new(),
                    redirect_uri: String::new(),
                    data,
                },
            )
            .await
            .unwrap();
        flow_id
    }

    async fn wait_for_session_count(state: &LoginState, user_id: &str, expected: i64) {
        let scoped = state.primary.db().scoped_default();
        for _ in 0..40 {
            let count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND user_id = $2",
            )
            .bind(scoped.instance_id())
            .bind(user_id)
            .fetch_one(scoped.pool())
            .await
            .unwrap();

            if count.0 == expected {
                return;
            }

            sleep(Duration::from_millis(25)).await;
        }

        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND user_id = $2")
                .bind(scoped.instance_id())
                .bind(user_id)
                .fetch_one(scoped.pool())
                .await
                .unwrap();
        assert_eq!(
            count.0, expected,
            "session count did not converge to the expected persisted value"
        );
    }

    // ─── Happy Path ───────────────────────────────────────

    #[tokio::test]
    async fn identifier_step_advances_to_password() {
        let state = test_state().await;
        insert_user(&state, "default", "u1", "org1", "alice", "pass123").await;
        let flow_id = create_flow(&state).await;

        let req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: "alice".into(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let resp = handle_identifier_step(&state, &flow_id, &req, &flow.data).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Flow should now be at password step.
        let updated = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.step, "password");
    }

    #[tokio::test]
    async fn correct_password_creates_session() {
        let state = test_state().await;
        insert_user(&state, "default", "u1", "org1", "alice", "pass123").await;
        let flow_id = create_flow(&state).await;

        // Advance to password step first.
        let id_req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: "alice".into(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        handle_identifier_step(&state, &flow_id, &id_req, &flow.data).await;

        // Now submit password.
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let pwd_req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "pass123".into(),
            _extra: serde_json::Value::Null,
        };
        let resp =
            handle_password_step(&state, &flow_id, &pwd_req, &flow.data, &flow.redirect_uri).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Session should exist in the stateful store once the local sink flushes.
        wait_for_session_count(&state, "u1", 1).await;

        // Completed flows are now consume-once and should no longer be readable.
        assert!(
            state
                .transient
                .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
                .await
                .unwrap()
                .is_none()
        );
    }

    // ─── Rejection Cases ──────────────────────────────────

    #[tokio::test]
    async fn wrong_password_rejected_no_session() {
        let state = test_state().await;
        insert_user(&state, "default", "u1", "org1", "alice", "pass123").await;
        let flow_id = create_flow(&state).await;

        // Advance to password.
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let id_req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: "alice".into(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        handle_identifier_step(&state, &flow_id, &id_req, &flow.data).await;

        // Submit wrong password.
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let pwd_req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "WRONG".into(),
            _extra: serde_json::Value::Null,
        };
        let resp =
            handle_password_step(&state, &flow_id, &pwd_req, &flow.data, &flow.redirect_uri).await;
        // Should return 200 with error node (not 401 — server-driven UI).
        assert_eq!(resp.status(), StatusCode::OK);

        // No session should be created.
        let scoped = state.primary.db().scoped_default();
        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND user_id = $2")
                .bind(scoped.instance_id())
                .bind("u1")
                .fetch_one(scoped.pool())
                .await
                .unwrap();
        assert_eq!(count.0, 0, "no session after wrong password");
    }

    #[tokio::test]
    async fn nonexistent_user_rejected() {
        let state = test_state().await;
        // Don't create any user.
        let flow_id = create_flow(&state).await;

        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let id_req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: "nobody".into(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        // Identifier step should still advance (don't reveal user existence).
        let resp = handle_identifier_step(&state, &flow_id, &id_req, &flow.data).await;
        assert_eq!(resp.status(), StatusCode::OK);

        // Password step should fail (user not found → invalid credentials).
        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let pwd_req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "anything".into(),
            _extra: serde_json::Value::Null,
        };
        let resp =
            handle_password_step(&state, &flow_id, &pwd_req, &flow.data, &flow.redirect_uri).await;
        assert_eq!(resp.status(), StatusCode::OK); // returns error in UI nodes

        // No session.
        let scoped = state.primary.db().scoped_default();
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1")
            .bind(scoped.instance_id())
            .fetch_one(scoped.pool())
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }

    #[tokio::test]
    async fn empty_identifier_rejected() {
        let state = test_state().await;
        let flow_id = create_flow(&state).await;

        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: String::new(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        let resp = handle_identifier_step(&state, &flow_id, &req, &flow.data).await;
        assert_eq!(resp.status(), StatusCode::OK); // returns error node

        // Flow should still be at identifier step (not advanced).
        let same = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(same.step, "identifier");
    }

    // ─── Instance Isolation ───────────────────────────────

    #[tokio::test]
    async fn user_from_other_instance_not_found() {
        let state = test_state().await;
        // Insert user in "other" instance, not "default".
        insert_user(&state, "other", "u1", "org1", "alice", "pass123").await;
        let flow_id = create_flow(&state).await;

        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let id_req = FlowSubmitRequest {
            action: "identifier".into(),
            identifier: "alice".into(),
            password: String::new(),
            _extra: serde_json::Value::Null,
        };
        handle_identifier_step(&state, &flow_id, &id_req, &flow.data).await;

        let flow = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        let pwd_req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "pass123".into(),
            _extra: serde_json::Value::Null,
        };
        let resp =
            handle_password_step(&state, &flow_id, &pwd_req, &flow.data, &flow.redirect_uri).await;
        assert_eq!(resp.status(), StatusCode::OK); // error in nodes

        // No session in default instance.
        let scoped = state.primary.db().scoped_default();
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1")
            .bind(scoped.instance_id())
            .fetch_one(scoped.pool())
            .await
            .unwrap();
        assert_eq!(count.0, 0);
    }

    // ─── Original cross-tenant test ───────────────────────

    #[tokio::test]
    async fn password_step_rejects_cross_tenant_auth_request() {
        let state = test_state().await;
        insert_user(&state, "default", "user-1", "org-1", "alice", "secret").await;

        let flow_scoped = state.primary.db().scoped_default();
        let foreign_scoped = state.primary.db().scoped("other".to_string());

        let flow_data = serde_json::json!({
            "identifier": "alice",
            "auth_request_id": "foreign-auth",
        });
        let flow_sql = format!(
            "INSERT INTO auth_states (id, instance_id, type, redirect_uri, data, step) \
             VALUES ($1, $2, 'login_flow', $3, {}, 'password')",
            flow_scoped.json_bind(4),
        );
        sqlx::query(&flow_sql)
            .bind("flow-1")
            .bind(flow_scoped.instance_id())
            .bind("/console")
            .bind(flow_data.to_string())
            .execute(flow_scoped.pool())
            .await
            .unwrap();

        let foreign_auth_sql = "INSERT INTO oidc_auth_requests (id, instance_id, client_id, redirect_uri, state, prompt) \
             VALUES ($1, $2, $3, $4, $5, $6)";
        sqlx::query(foreign_auth_sql)
            .bind("foreign-auth")
            .bind(foreign_scoped.instance_id())
            .bind("client-1")
            .bind("https://rp.example/callback")
            .bind("foreign-state")
            .bind("[]")
            .execute(foreign_scoped.pool())
            .await
            .unwrap();

        let req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "secret".into(),
            _extra: serde_json::Value::Null,
        };

        let response = handle_password_step(&state, "flow-1", &req, &flow_data, "/console").await;

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let foreign_row: (String, String, i64) = sqlx::query_as(&format!(
            "SELECT COALESCE(user_id, ''), COALESCE(code, ''), {} FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2",
            foreign_scoped.bool_as_int("done"),
        ))
        .bind(foreign_scoped.instance_id())
        .bind("foreign-auth")
        .fetch_one(foreign_scoped.pool())
        .await
        .unwrap();
        assert_eq!(foreign_row.0, "");
        assert_eq!(foreign_row.1, "");
        assert_eq!(foreign_row.2, 0);

        let session_count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND user_id = $2")
                .bind(flow_scoped.instance_id())
                .bind("user-1")
                .fetch_one(flow_scoped.pool())
                .await
                .unwrap();
        assert_eq!(session_count.0, 0);
    }

    #[tokio::test]
    async fn password_step_redirects_using_preloaded_auth_request() {
        let state = test_state().await;
        insert_user(&state, "default", "user-1", "org-1", "alice", "secret").await;

        let scoped = state.primary.db().scoped_default();
        let flow_data = serde_json::json!({
            "identifier": "alice",
            "auth_request_id": "auth-1",
        });
        let flow_sql = format!(
            "INSERT INTO auth_states (id, instance_id, type, redirect_uri, data, step) \
             VALUES ($1, $2, 'login_flow', $3, {}, 'password')",
            scoped.json_bind(4),
        );
        sqlx::query(&flow_sql)
            .bind("flow-1")
            .bind(scoped.instance_id())
            .bind("/console")
            .bind(flow_data.to_string())
            .execute(scoped.pool())
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO oidc_auth_requests (id, instance_id, client_id, redirect_uri, state, prompt) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind("auth-1")
        .bind(scoped.instance_id())
        .bind("client-1")
        .bind("https://rp.example/callback")
        .bind("state-1")
        .bind("[]")
        .execute(scoped.pool())
        .await
        .unwrap();

        let req = FlowSubmitRequest {
            action: "password".into(),
            identifier: String::new(),
            password: "secret".into(),
            _extra: serde_json::Value::Null,
        };

        let response = handle_password_step(&state, "flow-1", &req, &flow_data, "/console").await;
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        assert_eq!(status, StatusCode::OK, "{}", String::from_utf8_lossy(&body));
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let redirect_uri = json["redirect_uri"].as_str().unwrap_or_default();
        assert!(redirect_uri.starts_with("https://rp.example/callback?"));
        assert!(redirect_uri.contains("state=state-1"));
        assert!(redirect_uri.contains("code="));

        let auth_row: (String, String, i64) = sqlx::query_as(&format!(
            "SELECT COALESCE(user_id, ''), COALESCE(code, ''), {} FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2",
            scoped.bool_as_int("done"),
        ))
        .bind(scoped.instance_id())
        .bind("auth-1")
        .fetch_one(scoped.pool())
        .await
        .unwrap();
        assert_eq!(auth_row.0, "user-1");
        assert!(!auth_row.1.is_empty());
        assert_eq!(auth_row.2, 1);
        assert!(
            state
                .transient
                .load_auth_request_redirect(DEFAULT_INSTANCE_ID, "auth-1")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn effective_public_origin_uses_request_origin_when_unpinned() {
        let mut state = test_state().await;
        state.public_origin = Arc::new("http://localhost:8080".into());
        state.public_origin_override = None;

        let origin = with_instance_context(
            InstanceContext {
                instance_id: DEFAULT_INSTANCE_ID.to_string(),
                resolved_org_id: None,
                placement_mode: "global".into(),
                region_key: None,
                scheme: "https".into(),
                host: "demo.example.com".into(),
                source: "host".into(),
            },
            async { state.effective_public_origin().into_owned() },
        )
        .await;

        assert_eq!(origin, "https://demo.example.com");
    }
}
