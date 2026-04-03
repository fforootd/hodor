use serde::{Deserialize, Serialize};
use serde_json::Value;
use zitadel_db::Db;

mod auth_request;
mod login_flow;
mod provider;
mod sessions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub user_id: String,
    pub org_id: String,
    pub token_hash: String,
    pub user_agent: String,
    pub ip_address: String,
    pub metadata: Value,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedSession {
    pub session_id: String,
    pub token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewLoginFlowState {
    pub flow_id: String,
    pub state: String,
    pub redirect_uri: String,
    pub data: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginFlowRuntimeState {
    pub flow_id: String,
    pub step: String,
    pub redirect_uri: String,
    pub data: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthRequestRedirect {
    pub redirect_uri: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderAuthState {
    pub provider_id: String,
    pub state: String,
    pub nonce: String,
    pub pkce_verifier: String,
    pub flow_id: String,
    pub redirect_uri: String,
    pub expected_issuer: String,
    pub callback_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransientOp {
    SessionCreated {
        instance_id: String,
        session_id: String,
        user_id: String,
        org_id: String,
    },
    SessionRevoked {
        instance_id: String,
        session_id: String,
    },
    LoginFlowCreated {
        instance_id: String,
        flow_id: String,
    },
    LoginFlowUpdated {
        instance_id: String,
        flow_id: String,
        step: String,
    },
    AuthRequestCompleted {
        instance_id: String,
        auth_request_id: String,
        user_id: String,
    },
}

pub trait EdgeKv: Clone + Send + Sync + 'static {
    async fn create_session(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        user_agent: &str,
        ip_address: &str,
        fingerprint: &str,
    ) -> anyhow::Result<CreatedSession>;

    async fn find_session_by_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<SessionRecord>>;

    async fn list_sessions(&self, instance_id: &str) -> anyhow::Result<Vec<SessionRecord>>;

    async fn get_session(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<Option<SessionRecord>>;

    async fn revoke_session(&self, instance_id: &str, session_id: &str) -> anyhow::Result<bool>;

    async fn create_login_flow(
        &self,
        instance_id: &str,
        input: &NewLoginFlowState,
    ) -> anyhow::Result<()>;

    async fn load_login_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> anyhow::Result<Option<LoginFlowRuntimeState>>;

    async fn set_login_flow_step(
        &self,
        instance_id: &str,
        flow_id: &str,
        step: &str,
    ) -> anyhow::Result<bool>;

    async fn advance_login_flow_to_password(
        &self,
        instance_id: &str,
        flow_id: &str,
        user_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool>;

    async fn update_login_flow_data(
        &self,
        instance_id: &str,
        flow_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool>;

    async fn complete_login_flow(&self, instance_id: &str, flow_id: &str) -> anyhow::Result<bool>;

    async fn load_auth_request_redirect(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Option<AuthRequestRedirect>>;

    async fn complete_auth_request(
        &self,
        instance_id: &str,
        auth_request_id: &str,
        user_id: &str,
        code: &str,
    ) -> anyhow::Result<()>;

    async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Vec<String>>;

    async fn create_provider_auth_state(
        &self,
        instance_id: &str,
        state: &ProviderAuthState,
    ) -> anyhow::Result<()>;

    async fn consume_provider_auth_state(
        &self,
        instance_id: &str,
        state: &str,
    ) -> anyhow::Result<Option<ProviderAuthState>>;
}

pub trait EdgeSink: Clone + Send + Sync + 'static {
    async fn emit(&self, op: TransientOp) -> anyhow::Result<()>;
}

#[derive(Clone, Default)]
pub struct NoopEdgeSink;

impl EdgeSink for NoopEdgeSink {
    async fn emit(&self, _op: TransientOp) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct SqlTransientCompatKv {
    db: Db,
}

impl SqlTransientCompatKv {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub(crate) fn scoped(&self, instance_id: &str) -> zitadel_db::scoped::ScopedDb {
        self.db.scoped(instance_id.to_string())
    }
}

impl EdgeKv for SqlTransientCompatKv {
    async fn create_session(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        user_agent: &str,
        ip_address: &str,
        fingerprint: &str,
    ) -> anyhow::Result<CreatedSession> {
        sessions::create_session_impl(
            self,
            instance_id,
            user_id,
            org_id,
            user_agent,
            ip_address,
            fingerprint,
        )
        .await
    }

    async fn find_session_by_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        sessions::find_session_by_token_impl(self, instance_id, raw_token).await
    }

    async fn list_sessions(&self, instance_id: &str) -> anyhow::Result<Vec<SessionRecord>> {
        sessions::list_sessions_impl(self, instance_id).await
    }

    async fn get_session(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        sessions::get_session_impl(self, instance_id, session_id).await
    }

    async fn revoke_session(&self, instance_id: &str, session_id: &str) -> anyhow::Result<bool> {
        sessions::revoke_session_impl(self, instance_id, session_id).await
    }

    async fn create_login_flow(
        &self,
        instance_id: &str,
        input: &NewLoginFlowState,
    ) -> anyhow::Result<()> {
        login_flow::create_login_flow_impl(self, instance_id, input).await
    }

    async fn load_login_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> anyhow::Result<Option<LoginFlowRuntimeState>> {
        login_flow::load_login_flow_impl(self, instance_id, flow_id).await
    }

    async fn set_login_flow_step(
        &self,
        instance_id: &str,
        flow_id: &str,
        step: &str,
    ) -> anyhow::Result<bool> {
        login_flow::set_login_flow_step_impl(self, instance_id, flow_id, step).await
    }

    async fn advance_login_flow_to_password(
        &self,
        instance_id: &str,
        flow_id: &str,
        user_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        login_flow::advance_login_flow_to_password_impl(self, instance_id, flow_id, user_id, data)
            .await
    }

    async fn update_login_flow_data(
        &self,
        instance_id: &str,
        flow_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        login_flow::update_login_flow_data_impl(self, instance_id, flow_id, data).await
    }

    async fn complete_login_flow(&self, instance_id: &str, flow_id: &str) -> anyhow::Result<bool> {
        login_flow::complete_login_flow_impl(self, instance_id, flow_id).await
    }

    async fn load_auth_request_redirect(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        auth_request::load_auth_request_redirect_impl(self, instance_id, auth_request_id).await
    }

    async fn complete_auth_request(
        &self,
        instance_id: &str,
        auth_request_id: &str,
        user_id: &str,
        code: &str,
    ) -> anyhow::Result<()> {
        auth_request::complete_auth_request_impl(self, instance_id, auth_request_id, user_id, code)
            .await
    }

    async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        auth_request::load_auth_request_prompts_impl(self, instance_id, auth_request_id).await
    }

    async fn create_provider_auth_state(
        &self,
        instance_id: &str,
        state: &ProviderAuthState,
    ) -> anyhow::Result<()> {
        provider::create_provider_auth_state_impl(self, instance_id, state).await
    }

    async fn consume_provider_auth_state(
        &self,
        instance_id: &str,
        state: &str,
    ) -> anyhow::Result<Option<ProviderAuthState>> {
        provider::consume_provider_auth_state_impl(self, instance_id, state).await
    }
}

#[derive(Clone)]
pub struct TransientStorage<K, S> {
    kv: K,
    sink: S,
}

impl<K, S> TransientStorage<K, S> {
    pub fn new(kv: K, sink: S) -> Self {
        Self { kv, sink }
    }

    pub fn kv(&self) -> &K {
        &self.kv
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }
}

impl<K, S> TransientStorage<K, S>
where
    K: EdgeKv,
    S: EdgeSink,
{
    pub async fn create_session(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        user_agent: &str,
        ip_address: &str,
        fingerprint: &str,
    ) -> anyhow::Result<CreatedSession> {
        let session = self
            .kv
            .create_session(
                instance_id,
                user_id,
                org_id,
                user_agent,
                ip_address,
                fingerprint,
            )
            .await?;
        if let Err(error) = self
            .sink
            .emit(TransientOp::SessionCreated {
                instance_id: instance_id.to_string(),
                session_id: session.session_id.clone(),
                user_id: user_id.to_string(),
                org_id: org_id.to_string(),
            })
            .await
        {
            tracing::warn!(
                stream = "event_pusher",
                %error,
                session_id = %session.session_id,
                "transient sink emit failed"
            );
        }
        Ok(session)
    }

    pub async fn find_session_by_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        self.kv.find_session_by_token(instance_id, raw_token).await
    }

    pub async fn list_sessions(&self, instance_id: &str) -> anyhow::Result<Vec<SessionRecord>> {
        self.kv.list_sessions(instance_id).await
    }

    pub async fn get_session(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        self.kv.get_session(instance_id, session_id).await
    }

    pub async fn revoke_session(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<bool> {
        let changed = self.kv.revoke_session(instance_id, session_id).await?;
        if changed {
            if let Err(error) = self
                .sink
                .emit(TransientOp::SessionRevoked {
                    instance_id: instance_id.to_string(),
                    session_id: session_id.to_string(),
                })
                .await
            {
                tracing::warn!(stream = "event_pusher", %error, %session_id, "transient sink emit failed");
            }
        }
        Ok(changed)
    }

    pub async fn create_login_flow(
        &self,
        instance_id: &str,
        input: &NewLoginFlowState,
    ) -> anyhow::Result<()> {
        self.kv.create_login_flow(instance_id, input).await?;
        if let Err(error) = self
            .sink
            .emit(TransientOp::LoginFlowCreated {
                instance_id: instance_id.to_string(),
                flow_id: input.flow_id.clone(),
            })
            .await
        {
            tracing::warn!(
                stream = "event_pusher",
                %error,
                flow_id = %input.flow_id,
                "transient sink emit failed"
            );
        }
        Ok(())
    }

    pub async fn load_login_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> anyhow::Result<Option<LoginFlowRuntimeState>> {
        self.kv.load_login_flow(instance_id, flow_id).await
    }

    pub async fn set_login_flow_step(
        &self,
        instance_id: &str,
        flow_id: &str,
        step: &str,
    ) -> anyhow::Result<bool> {
        let changed = self
            .kv
            .set_login_flow_step(instance_id, flow_id, step)
            .await?;
        if changed {
            if let Err(error) = self
                .sink
                .emit(TransientOp::LoginFlowUpdated {
                    instance_id: instance_id.to_string(),
                    flow_id: flow_id.to_string(),
                    step: step.to_string(),
                })
                .await
            {
                tracing::warn!(stream = "event_pusher", %error, %flow_id, "transient sink emit failed");
            }
        }
        Ok(changed)
    }

    pub async fn advance_login_flow_to_password(
        &self,
        instance_id: &str,
        flow_id: &str,
        user_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        let changed = self
            .kv
            .advance_login_flow_to_password(instance_id, flow_id, user_id, data)
            .await?;
        if changed {
            if let Err(error) = self
                .sink
                .emit(TransientOp::LoginFlowUpdated {
                    instance_id: instance_id.to_string(),
                    flow_id: flow_id.to_string(),
                    step: "password".to_string(),
                })
                .await
            {
                tracing::warn!(stream = "event_pusher", %error, %flow_id, "transient sink emit failed");
            }
        }
        Ok(changed)
    }

    pub async fn update_login_flow_data(
        &self,
        instance_id: &str,
        flow_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        self.kv
            .update_login_flow_data(instance_id, flow_id, data)
            .await
    }

    pub async fn complete_login_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> anyhow::Result<bool> {
        let changed = self.kv.complete_login_flow(instance_id, flow_id).await?;
        if changed {
            if let Err(error) = self
                .sink
                .emit(TransientOp::LoginFlowUpdated {
                    instance_id: instance_id.to_string(),
                    flow_id: flow_id.to_string(),
                    step: "complete".to_string(),
                })
                .await
            {
                tracing::warn!(stream = "event_pusher", %error, %flow_id, "transient sink emit failed");
            }
        }
        Ok(changed)
    }

    pub async fn load_auth_request_redirect(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        self.kv
            .load_auth_request_redirect(instance_id, auth_request_id)
            .await
    }

    pub async fn complete_auth_request(
        &self,
        instance_id: &str,
        auth_request_id: &str,
        user_id: &str,
        code: &str,
    ) -> anyhow::Result<()> {
        self.kv
            .complete_auth_request(instance_id, auth_request_id, user_id, code)
            .await?;
        if let Err(error) = self
            .sink
            .emit(TransientOp::AuthRequestCompleted {
                instance_id: instance_id.to_string(),
                auth_request_id: auth_request_id.to_string(),
                user_id: user_id.to_string(),
            })
            .await
        {
            tracing::warn!(
                stream = "event_pusher",
                %error,
                %auth_request_id,
                "transient sink emit failed"
            );
        }
        Ok(())
    }

    pub async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        self.kv
            .load_auth_request_prompts(instance_id, auth_request_id)
            .await
    }

    pub async fn create_provider_auth_state(
        &self,
        instance_id: &str,
        state: &ProviderAuthState,
    ) -> anyhow::Result<()> {
        self.kv.create_provider_auth_state(instance_id, state).await
    }

    pub async fn consume_provider_auth_state(
        &self,
        instance_id: &str,
        state: &str,
    ) -> anyhow::Result<Option<ProviderAuthState>> {
        self.kv
            .consume_provider_auth_state(instance_id, state)
            .await
    }
}

pub type DefaultTransientStorage = TransientStorage<SqlTransientCompatKv, NoopEdgeSink>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct RecordingEdgeSink {
        ops: Arc<Mutex<Vec<TransientOp>>>,
    }

    impl RecordingEdgeSink {
        fn ops(&self) -> Vec<TransientOp> {
            self.ops.lock().unwrap().clone()
        }
    }

    impl EdgeSink for RecordingEdgeSink {
        async fn emit(&self, op: TransientOp) -> anyhow::Result<()> {
            self.ops.lock().unwrap().push(op);
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct FailingEdgeSink;

    impl EdgeSink for FailingEdgeSink {
        async fn emit(&self, _op: TransientOp) -> anyhow::Result<()> {
            anyhow::bail!("sink unavailable")
        }
    }

    #[tokio::test]
    async fn transient_storage_emits_session_operations() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();

        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, $3)")
            .bind("org-1")
            .bind(scoped.instance_id())
            .bind("Default")
            .execute(scoped.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id, instance_id, org_id, identifier, user_type) VALUES ($1, $2, $3, $4, $5)")
            .bind("user-1")
            .bind(scoped.instance_id())
            .bind("org-1")
            .bind("alice")
            .bind("human")
            .execute(scoped.pool())
            .await
            .unwrap();

        let sink = RecordingEdgeSink::default();
        let storage = TransientStorage::new(SqlTransientCompatKv::new(db.clone()), sink.clone());

        let created = storage
            .create_session(
                zitadel_db::DEFAULT_INSTANCE_ID,
                "user-1",
                "org-1",
                "ua",
                "127.0.0.1",
                "",
            )
            .await
            .unwrap();
        let found = storage
            .find_session_by_token(zitadel_db::DEFAULT_INSTANCE_ID, &created.token)
            .await
            .unwrap();
        assert!(found.is_some());

        let revoked = storage
            .revoke_session(zitadel_db::DEFAULT_INSTANCE_ID, &created.session_id)
            .await
            .unwrap();
        assert!(revoked);

        let ops = sink.ops();
        assert!(ops.iter().any(|op| matches!(op, TransientOp::SessionCreated { session_id, .. } if session_id == &created.session_id)));
        assert!(ops.iter().any(|op| matches!(op, TransientOp::SessionRevoked { session_id, .. } if session_id == &created.session_id)));
    }

    #[tokio::test]
    async fn sink_failure_does_not_fail_session_creation() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();

        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, $3)")
            .bind("org-1")
            .bind(scoped.instance_id())
            .bind("Default")
            .execute(scoped.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id, instance_id, org_id, identifier, user_type) VALUES ($1, $2, $3, $4, $5)")
            .bind("user-1")
            .bind(scoped.instance_id())
            .bind("org-1")
            .bind("alice")
            .bind("human")
            .execute(scoped.pool())
            .await
            .unwrap();

        let storage = TransientStorage::new(SqlTransientCompatKv::new(db.clone()), FailingEdgeSink);
        let created = storage
            .create_session(
                zitadel_db::DEFAULT_INSTANCE_ID,
                "user-1",
                "org-1",
                "ua",
                "127.0.0.1",
                "",
            )
            .await
            .unwrap();

        let found = storage
            .find_session_by_token(zitadel_db::DEFAULT_INSTANCE_ID, &created.token)
            .await
            .unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn provider_auth_state_is_consumed_once() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let storage = TransientStorage::new(SqlTransientCompatKv::new(db), NoopEdgeSink);

        let state = ProviderAuthState {
            provider_id: "provider-1".into(),
            state: "state-1".into(),
            nonce: "nonce-1".into(),
            pkce_verifier: "verifier-1".into(),
            flow_id: "flow-1".into(),
            redirect_uri: "/console".into(),
            expected_issuer: "https://issuer.example".into(),
            callback_uri: "http://localhost:8080/v1/auth/sso/callback".into(),
        };

        storage
            .create_provider_auth_state(zitadel_db::DEFAULT_INSTANCE_ID, &state)
            .await
            .unwrap();

        let first = storage
            .consume_provider_auth_state(zitadel_db::DEFAULT_INSTANCE_ID, "state-1")
            .await
            .unwrap();
        let second = storage
            .consume_provider_auth_state(zitadel_db::DEFAULT_INSTANCE_ID, "state-1")
            .await
            .unwrap();

        assert_eq!(first, Some(state));
        assert!(second.is_none());
    }
}
