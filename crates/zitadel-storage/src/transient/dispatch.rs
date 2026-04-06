use serde_json::Value;

use super::{
    AuthRequestRedirect, AuthRequestRequirements, CreatedSession, KvStore, LoginFlowRuntimeState,
    NewLoginFlowState, ProviderAuthState, SessionRecord, Sink, TransientRecord, TransientStorage,
    kv_memory::MemoryKvStore, kv_spanner::SpannerKvStore, kv_sql::SqlKvStore,
    sinks::{ChannelSink, NoopSink, SqlSink},
};

#[derive(Clone)]
pub enum DefaultKvStore {
    Memory(MemoryKvStore),
    Sql(SqlKvStore),
    Spanner(SpannerKvStore),
}

impl KvStore for DefaultKvStore {
    fn should_emit_provider_state_record(&self) -> bool {
        match self {
            Self::Memory(store) => store.should_emit_provider_state_record(),
            Self::Sql(store) => store.should_emit_provider_state_record(),
            Self::Spanner(store) => store.should_emit_provider_state_record(),
        }
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
            Self::Spanner(store) => {
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
            Self::Spanner(store) => store.find_session_by_token(instance_id, raw_token).await,
        }
    }

    async fn list_sessions(&self, instance_id: &str) -> anyhow::Result<Vec<SessionRecord>> {
        match self {
            Self::Memory(store) => store.list_sessions(instance_id).await,
            Self::Sql(store) => store.list_sessions(instance_id).await,
            Self::Spanner(store) => store.list_sessions(instance_id).await,
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
            Self::Spanner(store) => store.get_session(instance_id, session_id).await,
        }
    }

    async fn revoke_session(&self, instance_id: &str, session_id: &str) -> anyhow::Result<bool> {
        match self {
            Self::Memory(store) => store.revoke_session(instance_id, session_id).await,
            Self::Sql(store) => store.revoke_session(instance_id, session_id).await,
            Self::Spanner(store) => store.revoke_session(instance_id, session_id).await,
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
            Self::Spanner(store) => store.create_login_flow(instance_id, input).await,
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
            Self::Spanner(store) => store.load_login_flow(instance_id, flow_id).await,
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
            Self::Spanner(store) => store.set_login_flow_step(instance_id, flow_id, step).await,
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
            Self::Spanner(store) => {
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
            Self::Spanner(store) => {
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
            Self::Spanner(store) => store.complete_login_flow(instance_id, flow_id).await,
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
            Self::Spanner(store) => {
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
        session_id: Option<&str>,
        code: &str,
        auth_time: Option<&str>,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        match self {
            Self::Memory(store) => {
                store
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
            Self::Sql(store) => {
                store
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
            Self::Spanner(store) => {
                store
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
        }
    }

    async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<AuthRequestRequirements> {
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
            Self::Spanner(store) => {
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
            Self::Spanner(store) => store.create_provider_auth_state(instance_id, state).await,
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
            Self::Spanner(store) => store.consume_provider_auth_state(instance_id, state).await,
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
