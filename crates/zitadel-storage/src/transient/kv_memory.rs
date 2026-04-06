use std::{collections::HashMap, sync::Arc};

use serde_json::Value;
use tokio::sync::RwLock;
use uuid::Uuid;
use zitadel_crypto::token_hash;
use zitadel_db::Db;

use super::{
    AuthRequestRedirect, AuthRequestRequirements, CreatedSession, KvStore, LoginFlowRuntimeState,
    NewLoginFlowState, ProviderAuthState, SessionRecord,
    current_timestamp, kv_sql::SqlKvStore, semantics, session_timestamps, sql_user_is_active,
    semantics::{
        SessionLookupOutcome, TransientStateMeta, TransientStateOutcome,
        default_transient_state_meta, session_lookup_outcome, transient_state_outcome,
    },
};

#[derive(Clone)]
pub(super) struct MemorySessionEntry {
    pub(super) record: SessionRecord,
    pub(super) meta: TransientStateMeta,
}

#[derive(Clone)]
pub(super) struct MemoryLoginFlowEntry {
    pub(super) _state: String,
    pub(super) redirect_uri: String,
    pub(super) step: String,
    pub(super) data: Value,
    pub(super) user_id: String,
    pub(super) meta: TransientStateMeta,
}

#[derive(Clone)]
pub(super) struct MemoryProviderStateEntry {
    pub(super) state: ProviderAuthState,
    pub(super) meta: TransientStateMeta,
}

#[derive(Clone, Default)]
pub(super) struct MemoryState {
    pub(super) sessions: HashMap<String, HashMap<String, MemorySessionEntry>>,
    pub(super) login_flows: HashMap<String, HashMap<String, MemoryLoginFlowEntry>>,
    pub(super) provider_states: HashMap<String, HashMap<String, MemoryProviderStateEntry>>,
}

#[derive(Clone)]
pub struct MemoryKvStore {
    db: Db,
    session_max_age_secs: u64,
    pub(super) state: Arc<RwLock<MemoryState>>,
}

impl MemoryKvStore {
    pub fn new(db: Db, session_max_age_secs: u64) -> Self {
        Self {
            db,
            session_max_age_secs,
            state: Arc::new(RwLock::new(MemoryState::default())),
        }
    }

    fn sql(&self) -> SqlKvStore {
        SqlKvStore::local_only(self.db.clone(), self.session_max_age_secs)
    }
}

impl KvStore for MemoryKvStore {
    fn should_emit_provider_state_record(&self) -> bool {
        true
    }

    async fn create_session(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        user_agent: &str,
        ip_address: &str,
        fingerprint: &str,
    ) -> anyhow::Result<CreatedSession> {
        let session_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let (created_at, created_at_epoch, expires_at) =
            session_timestamps(&self.db, instance_id, self.session_max_age_secs).await?;
        let record = SessionRecord {
            id: session_id.clone(),
            user_id: user_id.to_string(),
            org_id: if org_id.is_empty() {
                String::new()
            } else {
                org_id.to_string()
            },
            token_hash: token_hash(&token),
            user_agent: user_agent.to_string(),
            ip_address: ip_address.to_string(),
            fingerprint: fingerprint.to_string(),
            metadata: Value::Object(Default::default()),
            created_at,
            created_at_epoch,
            expires_at,
            revoked_at: None,
        };
        let created_at = record.created_at.clone();
        let created_at_epoch = record.created_at_epoch;
        let meta = TransientStateMeta {
            expires_at_epoch: Some(created_at_epoch + self.session_max_age_secs.max(1)),
            ..TransientStateMeta::default()
        };

        self.state
            .write()
            .await
            .sessions
            .entry(instance_id.to_string())
            .or_default()
            .insert(session_id.clone(), MemorySessionEntry { record, meta });

        Ok(CreatedSession {
            session_id,
            token,
            created_at,
            created_at_epoch,
        })
    }

    async fn find_session_by_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        let hashed = token_hash(raw_token);
        if let Some(entry) =
            self.state
                .read()
                .await
                .sessions
                .get(instance_id)
                .and_then(|sessions| {
                    sessions
                        .values()
                        .find(|entry| entry.record.token_hash == hashed)
                        .cloned()
                })
        {
            if !sql_user_is_active(&self.db, instance_id, &entry.record.user_id).await? {
                return Ok(None);
            }

            let mut record = entry.record;
            if entry.meta.revoked && record.revoked_at.is_none() {
                record.revoked_at = Some(String::new());
            }
            return Ok(
                match session_lookup_outcome(record, entry.meta.expires_at_epoch) {
                    SessionLookupOutcome::Active(found) => Some(found),
                    SessionLookupOutcome::Inactive | SessionLookupOutcome::Missing => None,
                },
            );
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
            entry.meta.revoked = true;
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
                    meta: default_transient_state_meta(),
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
            let outcome = transient_state_outcome(
                LoginFlowRuntimeState {
                    flow_id: flow_id.to_string(),
                    step: flow.step,
                    redirect_uri: flow.redirect_uri,
                    data: flow.data,
                },
                flow.meta,
            );
            return Ok(match outcome {
                TransientStateOutcome::Active(state) => Some(state),
                TransientStateOutcome::Inactive | TransientStateOutcome::Missing => None,
            });
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
            if flow.meta.is_inactive_at(semantics::now_epoch_secs()) {
                return Ok(false);
            }
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
            if flow.meta.is_inactive_at(semantics::now_epoch_secs()) {
                return Ok(false);
            }
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
            if flow.meta.is_inactive_at(semantics::now_epoch_secs()) {
                return Ok(false);
            }
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
            if flow.meta.is_inactive_at(semantics::now_epoch_secs()) {
                return Ok(false);
            }
            flow.step = "complete".to_string();
            flow.meta.consumed_or_done = true;
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
        session_id: Option<&str>,
        code: &str,
        auth_time: Option<&str>,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        self.sql()
            .complete_auth_request(
                instance_id,
                auth_request_id,
                user_id,
                session_id,
                code,
                auth_time,
            )
            .await
    }

    async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<AuthRequestRequirements> {
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
            .insert(
                state.state.clone(),
                MemoryProviderStateEntry {
                    state: state.clone(),
                    meta: default_transient_state_meta(),
                },
            );
        Ok(())
    }

    async fn consume_provider_auth_state(
        &self,
        instance_id: &str,
        state: &str,
    ) -> anyhow::Result<Option<ProviderAuthState>> {
        if let Some(stored) = self
            .sql()
            .consume_provider_auth_state(instance_id, state)
            .await?
        {
            if let Some(states) = self
                .state
                .write()
                .await
                .provider_states
                .get_mut(instance_id)
            {
                states.remove(state);
            }
            return Ok(Some(stored));
        }

        Ok(self
            .state
            .write()
            .await
            .provider_states
            .get_mut(instance_id)
            .and_then(|states| states.remove(state))
            .and_then(
                |entry| match transient_state_outcome(entry.state, entry.meta) {
                    TransientStateOutcome::Active(state) => Some(state),
                    TransientStateOutcome::Inactive | TransientStateOutcome::Missing => None,
                },
            ))
    }
}
