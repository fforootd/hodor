use serde_json::Value;
use zitadel_db::Db;

use super::{
    AuthRequestRedirect, AuthRequestRequirements, CreatedSession, KvStore, LoginFlowRuntimeState,
    NewLoginFlowState, ProviderAuthState, SessionRecord, auth_request, login_flow, provider,
    sessions,
};

#[derive(Clone)]
pub struct SqlKvStore {
    primary_db: Db,
    authoritative_db: Option<Db>,
    validate_local_users: bool,
    session_max_age_secs: u64,
}

impl SqlKvStore {
    pub fn new(primary_db: Db, authoritative_db: Option<Db>, session_max_age_secs: u64) -> Self {
        let validate_local_users = authoritative_db.is_none();
        Self {
            primary_db,
            authoritative_db,
            validate_local_users,
            session_max_age_secs,
        }
    }

    pub fn local_only(primary_db: Db, session_max_age_secs: u64) -> Self {
        Self::new(primary_db, None, session_max_age_secs)
    }

    pub(crate) fn scoped(&self, instance_id: &str) -> zitadel_db::scoped::ScopedDb {
        self.primary_db.scoped(instance_id.to_string())
    }

    pub(crate) fn authoritative_scoped(
        &self,
        instance_id: &str,
    ) -> Option<zitadel_db::scoped::ScopedDb> {
        self.authoritative_db
            .as_ref()
            .map(|db| db.scoped(instance_id.to_string()))
    }

    pub(crate) fn validate_local_users(&self) -> bool {
        self.validate_local_users
    }

    pub(crate) fn session_max_age_secs(&self) -> u64 {
        self.session_max_age_secs
    }
}

impl KvStore for SqlKvStore {
    fn should_emit_provider_state_record(&self) -> bool {
        false
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
        session_id: Option<&str>,
        code: &str,
        auth_time: Option<&str>,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        auth_request::complete_auth_request_impl(
            self,
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
        auth_request::load_auth_request_requirements_impl(self, instance_id, auth_request_id).await
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
