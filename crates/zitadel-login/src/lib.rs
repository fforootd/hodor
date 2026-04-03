pub mod conformance;
pub mod legacy;
pub mod redirect;
pub mod session;
pub mod sso;
pub mod steps;
pub mod ui;

use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;
use zitadel_authn::password::Swapper;
use zitadel_db::Db;
use zitadel_storage::{DefaultStatefulStorage, DefaultTransientStorage};

pub(crate) type DefaultRpService = zitadel_oidc::rp::RpService<
    zitadel_oidc::rp::ReqwestHttpClient,
    zitadel_oidc::rp::InMemoryIssuerMetadataCache,
>;

#[derive(Clone)]
pub struct LoginState {
    pub db: Db,
    pub stateful: Arc<DefaultStatefulStorage>,
    pub transient: Arc<DefaultTransientStorage>,
    pub passwords: Arc<Swapper>,
    pub cookie_config: Arc<zitadel_authn::cookie::CookieConfig>,
    pub public_origin: Arc<String>,
    pub conformance_login_html: bool,
    pub rp: Arc<DefaultRpService>,
}

pub fn routes(state: LoginState) -> Router {
    Router::new()
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use steps::{FlowSubmitRequest, handle_identifier_step, handle_password_step};
    use tokio::time::{Duration, sleep};
    use uuid::Uuid;
    use zitadel_authn::{
        cookie::CookieConfig,
        password::{Swapper, encode_credential_json},
    };
    use zitadel_db::DEFAULT_INSTANCE_ID;
    use zitadel_storage::NewLoginFlowState;

    async fn test_state() -> LoginState {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db).await.unwrap();
        let mut config = zitadel_config::Config::default();
        config.server.public_origin = "http://localhost:8080".into();
        config.server.force_insecure_cookies = false;
        let storage = zitadel_storage::StorageRuntime::from_config(&config.storage, db.clone())
            .await
            .unwrap();
        LoginState {
            db,
            stateful: storage.stateful.clone(),
            transient: storage.transient.clone(),
            passwords: Arc::new(Swapper::dev()),
            cookie_config: Arc::new(CookieConfig::new(
                vec!["test-secret".into()],
                "localhost",
                false,
            )),
            public_origin: Arc::new("http://localhost:8080".into()),
            conformance_login_html: false,
            rp: Arc::new(zitadel_oidc::rp::RpService::new(
                zitadel_oidc::rp::ReqwestHttpClient::new(),
                zitadel_oidc::rp::InMemoryIssuerMetadataCache::default(),
            )),
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
        let scoped = state.db.scoped(instance_id.to_string());
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
        let scoped = state.db.scoped_default();
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

        // Flow should be complete.
        let completed = state
            .transient
            .load_login_flow(DEFAULT_INSTANCE_ID, &flow_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(completed.step, "complete");
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
        let scoped = state.db.scoped_default();
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
        let scoped = state.db.scoped_default();
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
        let scoped = state.db.scoped_default();
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

        let flow_scoped = state.db.scoped_default();
        let foreign_scoped = state.db.scoped("other".to_string());

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
}
