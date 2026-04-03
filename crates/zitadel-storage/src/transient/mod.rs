use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    sync::{RwLock, mpsc},
    time::{self, Duration},
};
use uuid::Uuid;
use zitadel_crypto::token_hash;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSessionRecord {
    pub id: String,
    pub user_id: String,
    pub org_id: String,
    pub token_hash: String,
    pub user_agent: String,
    pub ip_address: String,
    pub metadata: Value,
    pub created_at: String,
    pub expires_at: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TransientRecord {
    SessionCreated {
        instance_id: String,
        session: PersistedSessionRecord,
    },
    SessionRevoked {
        instance_id: String,
        session_id: String,
    },
    LoginFlowCreated {
        instance_id: String,
        flow: NewLoginFlowState,
    },
    LoginFlowStepSet {
        instance_id: String,
        flow_id: String,
        step: String,
    },
    LoginFlowPromotedToPassword {
        instance_id: String,
        flow_id: String,
        user_id: String,
        data: Value,
    },
    LoginFlowDataUpdated {
        instance_id: String,
        flow_id: String,
        data: Value,
    },
    LoginFlowCompleted {
        instance_id: String,
        flow_id: String,
    },
    AuthRequestCompleted {
        instance_id: String,
        auth_request_id: String,
        user_id: String,
        code: String,
    },
}

pub trait KvStore: Clone + Send + Sync + 'static {
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

pub trait Sink: Clone + Send + Sync + 'static {
    async fn emit(&self, record: TransientRecord) -> anyhow::Result<()>;
}

#[derive(Clone, Default)]
pub struct NoopSink;

impl Sink for NoopSink {
    async fn emit(&self, _record: TransientRecord) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct SqlKvStore {
    db: Db,
}

impl SqlKvStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    pub(crate) fn scoped(&self, instance_id: &str) -> zitadel_db::scoped::ScopedDb {
        self.db.scoped(instance_id.to_string())
    }
}

impl KvStore for SqlKvStore {
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
struct MemorySessionEntry {
    record: SessionRecord,
}

#[derive(Clone)]
struct MemoryLoginFlowEntry {
    _state: String,
    redirect_uri: String,
    step: String,
    data: Value,
    user_id: String,
    done: bool,
}

#[derive(Clone, Default)]
struct MemoryState {
    sessions: HashMap<String, HashMap<String, MemorySessionEntry>>,
    login_flows: HashMap<String, HashMap<String, MemoryLoginFlowEntry>>,
    provider_states: HashMap<String, HashMap<String, ProviderAuthState>>,
}

#[derive(Clone)]
pub struct MemoryKvStore {
    db: Db,
    state: Arc<RwLock<MemoryState>>,
}

impl MemoryKvStore {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            state: Arc::new(RwLock::new(MemoryState::default())),
        }
    }

    fn sql(&self) -> SqlKvStore {
        SqlKvStore::new(self.db.clone())
    }
}

impl KvStore for MemoryKvStore {
    async fn create_session(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        user_agent: &str,
        ip_address: &str,
        _fingerprint: &str,
    ) -> anyhow::Result<CreatedSession> {
        let session_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let (created_at, expires_at) = session_timestamps(&self.db, instance_id).await?;
        let record = SessionRecord {
            id: session_id.clone(),
            user_id: user_id.to_string(),
            org_id: if org_id.is_empty() {
                "_global".to_string()
            } else {
                org_id.to_string()
            },
            token_hash: token_hash(&token),
            user_agent: user_agent.to_string(),
            ip_address: ip_address.to_string(),
            metadata: Value::Object(Default::default()),
            created_at,
            expires_at,
            revoked_at: None,
        };

        self.state
            .write()
            .await
            .sessions
            .entry(instance_id.to_string())
            .or_default()
            .insert(session_id.clone(), MemorySessionEntry { record });

        Ok(CreatedSession { session_id, token })
    }

    async fn find_session_by_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        let hashed = token_hash(raw_token);
        if let Some(found) =
            self.state
                .read()
                .await
                .sessions
                .get(instance_id)
                .and_then(|sessions| {
                    sessions
                        .values()
                        .find(|entry| {
                            entry.record.token_hash == hashed && entry.record.revoked_at.is_none()
                        })
                        .map(|entry| entry.record.clone())
                })
        {
            return Ok(Some(found));
        }

        self.sql()
            .find_session_by_token(instance_id, raw_token)
            .await
    }

    async fn list_sessions(&self, instance_id: &str) -> anyhow::Result<Vec<SessionRecord>> {
        let mut merged = self
            .sql()
            .list_sessions(instance_id)
            .await?
            .into_iter()
            .map(|session| (session.id.clone(), session))
            .collect::<HashMap<_, _>>();

        if let Some(memory_sessions) = self.state.read().await.sessions.get(instance_id) {
            for (session_id, entry) in memory_sessions {
                merged.insert(session_id.clone(), entry.record.clone());
            }
        }

        let mut sessions = merged.into_values().collect::<Vec<_>>();
        sessions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(sessions)
    }

    async fn get_session(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        if let Some(found) = self
            .state
            .read()
            .await
            .sessions
            .get(instance_id)
            .and_then(|sessions| sessions.get(session_id).map(|entry| entry.record.clone()))
        {
            return Ok(Some(found));
        }

        self.sql().get_session(instance_id, session_id).await
    }

    async fn revoke_session(&self, instance_id: &str, session_id: &str) -> anyhow::Result<bool> {
        let revoked_at = current_timestamp(&self.db, instance_id).await?;
        let mut changed = false;
        if let Some(entry) = self
            .state
            .write()
            .await
            .sessions
            .get_mut(instance_id)
            .and_then(|sessions| sessions.get_mut(session_id))
        {
            entry.record.revoked_at = Some(revoked_at.clone());
            changed = true;
        }
        Ok(self.sql().revoke_session(instance_id, session_id).await? || changed)
    }

    async fn create_login_flow(
        &self,
        instance_id: &str,
        input: &NewLoginFlowState,
    ) -> anyhow::Result<()> {
        self.state
            .write()
            .await
            .login_flows
            .entry(instance_id.to_string())
            .or_default()
            .insert(
                input.flow_id.clone(),
                MemoryLoginFlowEntry {
                    _state: input.state.clone(),
                    redirect_uri: input.redirect_uri.clone(),
                    step: "identifier".to_string(),
                    data: input.data.clone(),
                    user_id: String::new(),
                    done: false,
                },
            );
        Ok(())
    }

    async fn load_login_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> anyhow::Result<Option<LoginFlowRuntimeState>> {
        if let Some(flow) = self
            .state
            .read()
            .await
            .login_flows
            .get(instance_id)
            .and_then(|flows| flows.get(flow_id).cloned())
        {
            return Ok(Some(LoginFlowRuntimeState {
                flow_id: flow_id.to_string(),
                step: flow.step,
                redirect_uri: flow.redirect_uri,
                data: flow.data,
            }));
        }

        self.sql().load_login_flow(instance_id, flow_id).await
    }

    async fn set_login_flow_step(
        &self,
        instance_id: &str,
        flow_id: &str,
        step: &str,
    ) -> anyhow::Result<bool> {
        if let Some(flow) = self
            .state
            .write()
            .await
            .login_flows
            .get_mut(instance_id)
            .and_then(|flows| flows.get_mut(flow_id))
        {
            flow.step = step.to_string();
            return Ok(true);
        }

        self.sql()
            .set_login_flow_step(instance_id, flow_id, step)
            .await
    }

    async fn advance_login_flow_to_password(
        &self,
        instance_id: &str,
        flow_id: &str,
        user_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        if let Some(flow) = self
            .state
            .write()
            .await
            .login_flows
            .get_mut(instance_id)
            .and_then(|flows| flows.get_mut(flow_id))
        {
            flow.step = "password".to_string();
            flow.user_id = user_id.to_string();
            flow.data = data.clone();
            return Ok(true);
        }

        self.sql()
            .advance_login_flow_to_password(instance_id, flow_id, user_id, data)
            .await
    }

    async fn update_login_flow_data(
        &self,
        instance_id: &str,
        flow_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        if let Some(flow) = self
            .state
            .write()
            .await
            .login_flows
            .get_mut(instance_id)
            .and_then(|flows| flows.get_mut(flow_id))
        {
            flow.data = data.clone();
            return Ok(true);
        }

        self.sql()
            .update_login_flow_data(instance_id, flow_id, data)
            .await
    }

    async fn complete_login_flow(&self, instance_id: &str, flow_id: &str) -> anyhow::Result<bool> {
        if let Some(flow) = self
            .state
            .write()
            .await
            .login_flows
            .get_mut(instance_id)
            .and_then(|flows| flows.get_mut(flow_id))
        {
            flow.step = "complete".to_string();
            flow.done = true;
            return Ok(true);
        }

        self.sql().complete_login_flow(instance_id, flow_id).await
    }

    async fn load_auth_request_redirect(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        self.sql()
            .load_auth_request_redirect(instance_id, auth_request_id)
            .await
    }

    async fn complete_auth_request(
        &self,
        instance_id: &str,
        auth_request_id: &str,
        user_id: &str,
        code: &str,
    ) -> anyhow::Result<()> {
        self.sql()
            .complete_auth_request(instance_id, auth_request_id, user_id, code)
            .await
    }

    async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        self.sql()
            .load_auth_request_prompts(instance_id, auth_request_id)
            .await
    }

    async fn create_provider_auth_state(
        &self,
        instance_id: &str,
        state: &ProviderAuthState,
    ) -> anyhow::Result<()> {
        self.state
            .write()
            .await
            .provider_states
            .entry(instance_id.to_string())
            .or_default()
            .insert(state.state.clone(), state.clone());
        Ok(())
    }

    async fn consume_provider_auth_state(
        &self,
        instance_id: &str,
        state: &str,
    ) -> anyhow::Result<Option<ProviderAuthState>> {
        Ok(self
            .state
            .write()
            .await
            .provider_states
            .get_mut(instance_id)
            .and_then(|states| states.remove(state)))
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
    K: KvStore,
    S: Sink,
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
        let persisted = self
            .kv
            .get_session(instance_id, &session.session_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "created session {} not readable from kv",
                    session.session_id
                )
            })?;
        if let Err(error) = self
            .sink
            .emit(TransientRecord::SessionCreated {
                instance_id: instance_id.to_string(),
                session: PersistedSessionRecord {
                    id: persisted.id,
                    user_id: persisted.user_id,
                    org_id: persisted.org_id,
                    token_hash: persisted.token_hash,
                    user_agent: persisted.user_agent,
                    ip_address: persisted.ip_address,
                    metadata: persisted.metadata,
                    created_at: persisted.created_at,
                    expires_at: persisted.expires_at,
                },
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
        if changed
            && let Err(error) = self
                .sink
                .emit(TransientRecord::SessionRevoked {
                    instance_id: instance_id.to_string(),
                    session_id: session_id.to_string(),
                })
                .await
        {
            tracing::warn!(stream = "event_pusher", %error, %session_id, "transient sink emit failed");
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
            .emit(TransientRecord::LoginFlowCreated {
                instance_id: instance_id.to_string(),
                flow: input.clone(),
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
        if changed
            && let Err(error) = self
                .sink
                .emit(TransientRecord::LoginFlowStepSet {
                    instance_id: instance_id.to_string(),
                    flow_id: flow_id.to_string(),
                    step: step.to_string(),
                })
                .await
        {
            tracing::warn!(stream = "event_pusher", %error, %flow_id, "transient sink emit failed");
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
        if changed
            && let Err(error) = self
                .sink
                .emit(TransientRecord::LoginFlowPromotedToPassword {
                    instance_id: instance_id.to_string(),
                    flow_id: flow_id.to_string(),
                    user_id: user_id.to_string(),
                    data: data.clone(),
                })
                .await
        {
            tracing::warn!(stream = "event_pusher", %error, %flow_id, "transient sink emit failed");
        }
        Ok(changed)
    }

    pub async fn update_login_flow_data(
        &self,
        instance_id: &str,
        flow_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        let changed = self
            .kv
            .update_login_flow_data(instance_id, flow_id, data)
            .await?;
        if changed
            && let Err(error) = self
                .sink
                .emit(TransientRecord::LoginFlowDataUpdated {
                    instance_id: instance_id.to_string(),
                    flow_id: flow_id.to_string(),
                    data: data.clone(),
                })
                .await
        {
            tracing::warn!(stream = "event_pusher", %error, %flow_id, "transient sink emit failed");
        }
        Ok(changed)
    }

    pub async fn complete_login_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> anyhow::Result<bool> {
        let changed = self.kv.complete_login_flow(instance_id, flow_id).await?;
        if changed
            && let Err(error) = self
                .sink
                .emit(TransientRecord::LoginFlowCompleted {
                    instance_id: instance_id.to_string(),
                    flow_id: flow_id.to_string(),
                })
                .await
        {
            tracing::warn!(stream = "event_pusher", %error, %flow_id, "transient sink emit failed");
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
            .emit(TransientRecord::AuthRequestCompleted {
                instance_id: instance_id.to_string(),
                auth_request_id: auth_request_id.to_string(),
                user_id: user_id.to_string(),
                code: code.to_string(),
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

#[derive(Clone)]
pub enum DefaultKvStore {
    Memory(MemoryKvStore),
    Sql(SqlKvStore),
}

impl KvStore for DefaultKvStore {
    async fn create_session(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        user_agent: &str,
        ip_address: &str,
        fingerprint: &str,
    ) -> anyhow::Result<CreatedSession> {
        match self {
            Self::Memory(store) => {
                store
                    .create_session(
                        instance_id,
                        user_id,
                        org_id,
                        user_agent,
                        ip_address,
                        fingerprint,
                    )
                    .await
            }
            Self::Sql(store) => {
                store
                    .create_session(
                        instance_id,
                        user_id,
                        org_id,
                        user_agent,
                        ip_address,
                        fingerprint,
                    )
                    .await
            }
        }
    }

    async fn find_session_by_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        match self {
            Self::Memory(store) => store.find_session_by_token(instance_id, raw_token).await,
            Self::Sql(store) => store.find_session_by_token(instance_id, raw_token).await,
        }
    }

    async fn list_sessions(&self, instance_id: &str) -> anyhow::Result<Vec<SessionRecord>> {
        match self {
            Self::Memory(store) => store.list_sessions(instance_id).await,
            Self::Sql(store) => store.list_sessions(instance_id).await,
        }
    }

    async fn get_session(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        match self {
            Self::Memory(store) => store.get_session(instance_id, session_id).await,
            Self::Sql(store) => store.get_session(instance_id, session_id).await,
        }
    }

    async fn revoke_session(&self, instance_id: &str, session_id: &str) -> anyhow::Result<bool> {
        match self {
            Self::Memory(store) => store.revoke_session(instance_id, session_id).await,
            Self::Sql(store) => store.revoke_session(instance_id, session_id).await,
        }
    }

    async fn create_login_flow(
        &self,
        instance_id: &str,
        input: &NewLoginFlowState,
    ) -> anyhow::Result<()> {
        match self {
            Self::Memory(store) => store.create_login_flow(instance_id, input).await,
            Self::Sql(store) => store.create_login_flow(instance_id, input).await,
        }
    }

    async fn load_login_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> anyhow::Result<Option<LoginFlowRuntimeState>> {
        match self {
            Self::Memory(store) => store.load_login_flow(instance_id, flow_id).await,
            Self::Sql(store) => store.load_login_flow(instance_id, flow_id).await,
        }
    }

    async fn set_login_flow_step(
        &self,
        instance_id: &str,
        flow_id: &str,
        step: &str,
    ) -> anyhow::Result<bool> {
        match self {
            Self::Memory(store) => store.set_login_flow_step(instance_id, flow_id, step).await,
            Self::Sql(store) => store.set_login_flow_step(instance_id, flow_id, step).await,
        }
    }

    async fn advance_login_flow_to_password(
        &self,
        instance_id: &str,
        flow_id: &str,
        user_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        match self {
            Self::Memory(store) => {
                store
                    .advance_login_flow_to_password(instance_id, flow_id, user_id, data)
                    .await
            }
            Self::Sql(store) => {
                store
                    .advance_login_flow_to_password(instance_id, flow_id, user_id, data)
                    .await
            }
        }
    }

    async fn update_login_flow_data(
        &self,
        instance_id: &str,
        flow_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        match self {
            Self::Memory(store) => {
                store
                    .update_login_flow_data(instance_id, flow_id, data)
                    .await
            }
            Self::Sql(store) => {
                store
                    .update_login_flow_data(instance_id, flow_id, data)
                    .await
            }
        }
    }

    async fn complete_login_flow(&self, instance_id: &str, flow_id: &str) -> anyhow::Result<bool> {
        match self {
            Self::Memory(store) => store.complete_login_flow(instance_id, flow_id).await,
            Self::Sql(store) => store.complete_login_flow(instance_id, flow_id).await,
        }
    }

    async fn load_auth_request_redirect(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        match self {
            Self::Memory(store) => {
                store
                    .load_auth_request_redirect(instance_id, auth_request_id)
                    .await
            }
            Self::Sql(store) => {
                store
                    .load_auth_request_redirect(instance_id, auth_request_id)
                    .await
            }
        }
    }

    async fn complete_auth_request(
        &self,
        instance_id: &str,
        auth_request_id: &str,
        user_id: &str,
        code: &str,
    ) -> anyhow::Result<()> {
        match self {
            Self::Memory(store) => {
                store
                    .complete_auth_request(instance_id, auth_request_id, user_id, code)
                    .await
            }
            Self::Sql(store) => {
                store
                    .complete_auth_request(instance_id, auth_request_id, user_id, code)
                    .await
            }
        }
    }

    async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        match self {
            Self::Memory(store) => {
                store
                    .load_auth_request_prompts(instance_id, auth_request_id)
                    .await
            }
            Self::Sql(store) => {
                store
                    .load_auth_request_prompts(instance_id, auth_request_id)
                    .await
            }
        }
    }

    async fn create_provider_auth_state(
        &self,
        instance_id: &str,
        state: &ProviderAuthState,
    ) -> anyhow::Result<()> {
        match self {
            Self::Memory(store) => store.create_provider_auth_state(instance_id, state).await,
            Self::Sql(store) => store.create_provider_auth_state(instance_id, state).await,
        }
    }

    async fn consume_provider_auth_state(
        &self,
        instance_id: &str,
        state: &str,
    ) -> anyhow::Result<Option<ProviderAuthState>> {
        match self {
            Self::Memory(store) => store.consume_provider_auth_state(instance_id, state).await,
            Self::Sql(store) => store.consume_provider_auth_state(instance_id, state).await,
        }
    }
}

#[derive(Clone)]
pub enum DefaultSink {
    Noop(NoopSink),
    Channel(ChannelSink),
    Sql(SqlSink),
}

impl Sink for DefaultSink {
    async fn emit(&self, record: TransientRecord) -> anyhow::Result<()> {
        match self {
            Self::Noop(sink) => sink.emit(record).await,
            Self::Channel(sink) => sink.emit(record).await,
            Self::Sql(sink) => sink.emit(record).await,
        }
    }
}

pub type DefaultTransientStorage = TransientStorage<DefaultKvStore, DefaultSink>;

#[derive(Clone)]
pub struct ChannelSink {
    tx: mpsc::Sender<TransientRecord>,
}

impl ChannelSink {
    pub fn new(db: Db, buffer_size: usize, batch_size: usize, flush_interval: Duration) -> Self {
        let (tx, mut rx) = mpsc::channel(buffer_size.max(1));
        let db_clone = db.clone();
        tokio::spawn(async move {
            let mut ticker = time::interval(flush_interval);
            let mut pending = Vec::new();
            loop {
                tokio::select! {
                    maybe_record = rx.recv() => {
                        match maybe_record {
                            Some(record) => {
                                pending.push(record);
                                if pending.len() >= batch_size.max(1)
                                    && apply_channel_batch(&db_clone, &mut pending).await.is_err()
                                {
                                    ticker.tick().await;
                                }
                            }
                            None => {
                                let _ = apply_channel_batch(&db_clone, &mut pending).await;
                                break;
                            }
                        }
                    }
                    _ = ticker.tick() => {
                        let _ = apply_channel_batch(&db_clone, &mut pending).await;
                    }
                }
            }
        });
        Self { tx }
    }
}

impl Sink for ChannelSink {
    async fn emit(&self, record: TransientRecord) -> anyhow::Result<()> {
        self.tx.send(record).await?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct SqlSink {
    db: Db,
}

impl SqlSink {
    pub async fn new(db: Db, batch_size: usize, flush_interval: Duration) -> anyhow::Result<Self> {
        ensure_sink_inbox_table(&db).await?;
        let db_clone = db.clone();
        tokio::spawn(async move {
            let mut ticker = time::interval(flush_interval);
            loop {
                ticker.tick().await;
                if let Err(error) = drain_sink_inbox(&db_clone, batch_size.max(1)).await {
                    tracing::warn!(stream = "event_pusher", %error, "sql sink drain failed");
                }
            }
        });
        Ok(Self { db })
    }
}

impl Sink for SqlSink {
    async fn emit(&self, record: TransientRecord) -> anyhow::Result<()> {
        insert_sink_record(&self.db, &record).await
    }
}

#[cfg(test)]
fn parse_duration(raw: &str) -> Duration {
    if let Some(value) = raw.strip_suffix("ms") {
        return Duration::from_millis(value.parse().unwrap_or(100));
    }
    if let Some(value) = raw.strip_suffix('s') {
        return Duration::from_secs(value.parse().unwrap_or(1));
    }
    if let Some(value) = raw.strip_suffix('m') {
        return Duration::from_secs(value.parse::<u64>().unwrap_or(1) * 60);
    }
    Duration::from_millis(100)
}

async fn apply_channel_batch(db: &Db, pending: &mut Vec<TransientRecord>) -> anyhow::Result<()> {
    if pending.is_empty() {
        return Ok(());
    }
    let batch = pending.clone();
    apply_transient_records(db, &batch).await?;
    pending.clear();
    Ok(())
}

async fn session_timestamps(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<(String, Option<String>)> {
    let scoped = db.scoped(instance_id.to_string());
    let sql = match db.dialect() {
        zitadel_db::Dialect::Postgres => {
            "SELECT CURRENT_TIMESTAMP::text, (CURRENT_TIMESTAMP + INTERVAL '24 hours')::text"
        }
        zitadel_db::Dialect::Sqlite => "SELECT datetime('now'), datetime('now', '+24 hours')",
    };
    let row: (String, String) = sqlx::query_as(sql).fetch_one(scoped.pool()).await?;
    Ok((row.0, Some(row.1)))
}

async fn current_timestamp(db: &Db, instance_id: &str) -> anyhow::Result<String> {
    let scoped = db.scoped(instance_id.to_string());
    let sql = match db.dialect() {
        zitadel_db::Dialect::Postgres => "SELECT CURRENT_TIMESTAMP::text",
        zitadel_db::Dialect::Sqlite => "SELECT datetime('now')",
    };
    let row: (String,) = sqlx::query_as(sql).fetch_one(scoped.pool()).await?;
    Ok(row.0)
}

async fn ensure_sink_inbox_table(db: &Db) -> anyhow::Result<()> {
    let sql = match db.dialect() {
        zitadel_db::Dialect::Postgres => {
            "CREATE TABLE IF NOT EXISTS storage_sink_inbox (
                id TEXT PRIMARY KEY,
                record_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"
        }
        zitadel_db::Dialect::Sqlite => {
            "CREATE TABLE IF NOT EXISTS storage_sink_inbox (
                id TEXT PRIMARY KEY,
                record_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )"
        }
    };
    sqlx::query(sql).execute(db.pool()).await?;
    Ok(())
}

fn record_type(record: &TransientRecord) -> &'static str {
    match record {
        TransientRecord::SessionCreated { .. } => "session.created",
        TransientRecord::SessionRevoked { .. } => "session.revoked",
        TransientRecord::LoginFlowCreated { .. } => "login_flow.created",
        TransientRecord::LoginFlowStepSet { .. } => "login_flow.step_set",
        TransientRecord::LoginFlowPromotedToPassword { .. } => "login_flow.password",
        TransientRecord::LoginFlowDataUpdated { .. } => "login_flow.data",
        TransientRecord::LoginFlowCompleted { .. } => "login_flow.completed",
        TransientRecord::AuthRequestCompleted { .. } => "auth_request.completed",
    }
}

async fn insert_sink_record(db: &Db, record: &TransientRecord) -> anyhow::Result<()> {
    let payload = serde_json::to_string(record)?;
    sqlx::query("INSERT INTO storage_sink_inbox (id, record_type, payload) VALUES ($1, $2, $3)")
        .bind(Uuid::new_v4().to_string())
        .bind(record_type(record))
        .bind(payload)
        .execute(db.pool())
        .await?;
    Ok(())
}

async fn drain_sink_inbox(db: &Db, batch_size: usize) -> anyhow::Result<()> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, payload FROM storage_sink_inbox ORDER BY created_at ASC LIMIT $1",
    )
    .bind(batch_size as i64)
    .fetch_all(db.pool())
    .await?;

    if rows.is_empty() {
        return Ok(());
    }

    let mut parsed = Vec::with_capacity(rows.len());
    for (id, payload) in &rows {
        parsed.push((
            id.clone(),
            serde_json::from_str::<TransientRecord>(payload)?,
        ));
    }

    let records = parsed
        .iter()
        .map(|(_, record)| record.clone())
        .collect::<Vec<_>>();
    apply_transient_records(db, &records).await?;

    let mut tx = db.pool().begin().await?;
    for (id, _) in parsed {
        sqlx::query("DELETE FROM storage_sink_inbox WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

async fn apply_transient_records(db: &Db, records: &[TransientRecord]) -> anyhow::Result<()> {
    let mut tx = db.pool().begin().await?;

    for record in records {
        match record {
            TransientRecord::SessionCreated {
                instance_id,
                session,
            } => {
                let scoped = db.scoped(instance_id.to_string());
                let sql = format!(
                    "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, {}, $9, $10) \
                     ON CONFLICT(id) DO UPDATE SET user_id = EXCLUDED.user_id, org_id = EXCLUDED.org_id, token_hash = EXCLUDED.token_hash, user_agent = EXCLUDED.user_agent, ip_address = EXCLUDED.ip_address, metadata = EXCLUDED.metadata, expires_at = EXCLUDED.expires_at",
                    scoped.json_bind(8),
                );
                sqlx::query(&sql)
                    .bind(&session.id)
                    .bind(scoped.instance_id())
                    .bind(&session.user_id)
                    .bind(&session.org_id)
                    .bind(&session.token_hash)
                    .bind(&session.user_agent)
                    .bind(&session.ip_address)
                    .bind(serde_json::to_string(&session.metadata).unwrap_or_else(|_| "{}".into()))
                    .bind(&session.created_at)
                    .bind(&session.expires_at)
                    .execute(&mut *tx)
                    .await?;
            }
            TransientRecord::SessionRevoked {
                instance_id,
                session_id,
            } => {
                sqlx::query(
                    "UPDATE sessions SET revoked_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND id = $2",
                )
                .bind(instance_id)
                .bind(session_id)
                .execute(&mut *tx)
                .await?;
            }
            TransientRecord::LoginFlowCreated { instance_id, flow } => {
                let scoped = db.scoped(instance_id.to_string());
                let sql = format!(
                    "INSERT INTO auth_states (id, instance_id, type, state, redirect_uri, data, step, done) \
                     VALUES ($1, $2, 'login_flow', $3, $4, {}, 'identifier', 0) \
                     ON CONFLICT(id) DO UPDATE SET state = EXCLUDED.state, redirect_uri = EXCLUDED.redirect_uri, data = EXCLUDED.data, step = 'identifier', done = 0",
                    scoped.json_bind(5),
                );
                sqlx::query(&sql)
                    .bind(&flow.flow_id)
                    .bind(scoped.instance_id())
                    .bind(&flow.state)
                    .bind(&flow.redirect_uri)
                    .bind(serde_json::to_string(&flow.data).unwrap_or_else(|_| "{}".into()))
                    .execute(&mut *tx)
                    .await?;
            }
            TransientRecord::LoginFlowStepSet {
                instance_id,
                flow_id,
                step,
            } => {
                sqlx::query(
                    "UPDATE auth_states SET step = $1 WHERE instance_id = $2 AND id = $3 AND type = 'login_flow'",
                )
                .bind(step)
                .bind(instance_id)
                .bind(flow_id)
                .execute(&mut *tx)
                .await?;
            }
            TransientRecord::LoginFlowPromotedToPassword {
                instance_id,
                flow_id,
                user_id,
                data,
            } => {
                let scoped = db.scoped(instance_id.to_string());
                let sql = format!(
                    "UPDATE auth_states SET step = 'password', user_id = $1, data = {} WHERE instance_id = $2 AND id = $3 AND type = 'login_flow'",
                    scoped.json_bind(4),
                );
                sqlx::query(&sql)
                    .bind(user_id)
                    .bind(scoped.instance_id())
                    .bind(flow_id)
                    .bind(serde_json::to_string(data).unwrap_or_else(|_| "{}".into()))
                    .execute(&mut *tx)
                    .await?;
            }
            TransientRecord::LoginFlowDataUpdated {
                instance_id,
                flow_id,
                data,
            } => {
                let scoped = db.scoped(instance_id.to_string());
                let sql = format!(
                    "UPDATE auth_states SET data = {} WHERE instance_id = $1 AND id = $2 AND type = 'login_flow'",
                    scoped.json_bind(3),
                );
                sqlx::query(&sql)
                    .bind(scoped.instance_id())
                    .bind(flow_id)
                    .bind(serde_json::to_string(data).unwrap_or_else(|_| "{}".into()))
                    .execute(&mut *tx)
                    .await?;
            }
            TransientRecord::LoginFlowCompleted {
                instance_id,
                flow_id,
            } => {
                sqlx::query(
                    "UPDATE auth_states SET step = 'complete', done = 1 WHERE instance_id = $1 AND id = $2 AND type = 'login_flow'",
                )
                .bind(instance_id)
                .bind(flow_id)
                .execute(&mut *tx)
                .await?;
            }
            TransientRecord::AuthRequestCompleted {
                instance_id,
                auth_request_id,
                user_id,
                code,
            } => {
                sqlx::query(
                    "UPDATE oidc_auth_requests SET user_id = $1, done = 1, auth_time = CURRENT_TIMESTAMP, code = $2 WHERE instance_id = $3 AND id = $4",
                )
                .bind(user_id)
                .bind(code)
                .bind(instance_id)
                .bind(auth_request_id)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct RecordingSink {
        records: Arc<Mutex<Vec<TransientRecord>>>,
    }

    impl RecordingSink {
        fn records(&self) -> Vec<TransientRecord> {
            self.records.lock().unwrap().clone()
        }
    }

    impl Sink for RecordingSink {
        async fn emit(&self, record: TransientRecord) -> anyhow::Result<()> {
            self.records.lock().unwrap().push(record);
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct FailingSink;

    impl Sink for FailingSink {
        async fn emit(&self, _record: TransientRecord) -> anyhow::Result<()> {
            anyhow::bail!("sink unavailable")
        }
    }

    #[tokio::test]
    async fn transient_storage_emits_session_records() {
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

        let sink = RecordingSink::default();
        let storage = TransientStorage::new(SqlKvStore::new(db.clone()), sink.clone());

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
        let revoked = storage
            .revoke_session(zitadel_db::DEFAULT_INSTANCE_ID, &created.session_id)
            .await
            .unwrap();
        assert!(revoked);

        let records = sink.records();
        assert!(records.iter().any(
            |record| matches!(record, TransientRecord::SessionCreated { session, .. } if session.id == created.session_id)
        ));
        assert!(records.iter().any(
            |record| matches!(record, TransientRecord::SessionRevoked { session_id, .. } if session_id == &created.session_id)
        ));
    }

    #[tokio::test]
    async fn sink_failure_does_not_fail_memory_session_creation() {
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

        let storage = TransientStorage::new(MemoryKvStore::new(db.clone()), FailingSink);
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
        let storage = TransientStorage::new(MemoryKvStore::new(db), NoopSink);

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

    #[tokio::test]
    async fn channel_sink_batches_memory_sessions_into_sqlite() {
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

        let sink = ChannelSink::new(db.clone(), 16, 4, parse_duration("10ms"));
        let storage = TransientStorage::new(MemoryKvStore::new(db.clone()), sink);
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

        tokio::time::sleep(Duration::from_millis(40)).await;

        let found = SqlKvStore::new(db)
            .find_session_by_token(zitadel_db::DEFAULT_INSTANCE_ID, &created.token)
            .await
            .unwrap();
        assert!(found.is_some());
    }
}
