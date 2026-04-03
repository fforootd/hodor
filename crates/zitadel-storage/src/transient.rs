use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use zitadel_crypto::token_hash;
use zitadel_db::{Db, Dialect};

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

    fn scoped(&self, instance_id: &str) -> zitadel_db::scoped::ScopedDb {
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
    ) -> anyhow::Result<CreatedSession> {
        let scoped = self.scoped(instance_id);
        let session_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let hashed_token = token_hash(&token);
        let org = if org_id.is_empty() { "_global" } else { org_id };
        let expires_expr = match scoped.dialect() {
            Dialect::Postgres => "CURRENT_TIMESTAMP + INTERVAL '24 hours'",
            Dialect::Sqlite => "datetime(CURRENT_TIMESTAMP, '+24 hours')",
        };
        let sql = format!(
            "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, created_at, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, {})",
            scoped.timestamp_now(),
            expires_expr,
        );

        sqlx::query(&sql)
            .bind(&session_id)
            .bind(scoped.instance_id())
            .bind(user_id)
            .bind(org)
            .bind(&hashed_token)
            .bind(user_agent)
            .bind(ip_address)
            .execute(scoped.pool())
            .await?;

        Ok(CreatedSession { session_id, token })
    }

    async fn find_session_by_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        let scoped = self.scoped(instance_id);
        let created_at = scoped.as_text("created_at");
        let expires_at = scoped.as_text("expires_at");
        let revoked_at = scoped.as_text("revoked_at");
        let sql = format!(
            "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, {created_at}, {expires_at}, {revoked_at} \
             FROM sessions \
             WHERE instance_id = $1 AND token_hash = $2 AND revoked_at IS NULL \
             AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)"
        );
        let row: Option<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(&sql)
            .bind(scoped.instance_id())
            .bind(token_hash(raw_token))
            .fetch_optional(scoped.pool())
            .await?;

        Ok(row.map(map_session_row))
    }

    async fn list_sessions(&self, instance_id: &str) -> anyhow::Result<Vec<SessionRecord>> {
        let scoped = self.scoped(instance_id);
        let created_at = scoped.as_text("created_at");
        let expires_at = scoped.as_text("expires_at");
        let revoked_at = scoped.as_text("revoked_at");
        let sql = format!(
            "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, {created_at}, {expires_at}, {revoked_at} \
             FROM sessions WHERE instance_id = $1 ORDER BY created_at DESC LIMIT 50"
        );
        let rows: Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(&sql)
            .bind(scoped.instance_id())
            .fetch_all(scoped.pool())
            .await?;

        Ok(rows.into_iter().map(map_session_row).collect())
    }

    async fn get_session(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        let scoped = self.scoped(instance_id);
        let created_at = scoped.as_text("created_at");
        let expires_at = scoped.as_text("expires_at");
        let revoked_at = scoped.as_text("revoked_at");
        let sql = format!(
            "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, {created_at}, {expires_at}, {revoked_at} \
             FROM sessions WHERE instance_id = $1 AND id = $2"
        );
        let row: Option<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(&sql)
            .bind(scoped.instance_id())
            .bind(session_id)
            .fetch_optional(scoped.pool())
            .await?;

        Ok(row.map(map_session_row))
    }

    async fn revoke_session(&self, instance_id: &str, session_id: &str) -> anyhow::Result<bool> {
        let scoped = self.scoped(instance_id);
        let result = sqlx::query(
            "UPDATE sessions SET revoked_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND id = $2",
        )
        .bind(scoped.instance_id())
        .bind(session_id)
        .execute(scoped.pool())
        .await?;

        Ok(result.rows_affected() > 0)
    }

    async fn create_login_flow(
        &self,
        instance_id: &str,
        input: &NewLoginFlowState,
    ) -> anyhow::Result<()> {
        let scoped = self.scoped(instance_id);
        let sql = format!(
            "INSERT INTO auth_states (id, instance_id, type, state, redirect_uri, data, step) \
             VALUES ($1, $2, 'login_flow', $3, $4, {}, 'identifier')",
            scoped.json_bind(5),
        );
        sqlx::query(&sql)
            .bind(&input.flow_id)
            .bind(scoped.instance_id())
            .bind(&input.state)
            .bind(&input.redirect_uri)
            .bind(serde_json::to_string(&input.data).unwrap_or_default())
            .execute(scoped.pool())
            .await?;
        Ok(())
    }

    async fn load_login_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> anyhow::Result<Option<LoginFlowRuntimeState>> {
        let scoped = self.scoped(instance_id);
        let sql = format!(
            "SELECT COALESCE(step, 'identifier'), COALESCE({}, '{{}}'), COALESCE(redirect_uri, '') \
             FROM auth_states WHERE instance_id = $1 AND id = $2 AND type = 'login_flow'",
            scoped.as_text("data"),
        );
        let row: Option<(String, String, String)> = sqlx::query_as(&sql)
            .bind(scoped.instance_id())
            .bind(flow_id)
            .fetch_optional(scoped.pool())
            .await?;

        Ok(row.map(|(step, data, redirect_uri)| LoginFlowRuntimeState {
            flow_id: flow_id.to_string(),
            step,
            redirect_uri,
            data: serde_json::from_str(&data).unwrap_or_default(),
        }))
    }

    async fn set_login_flow_step(
        &self,
        instance_id: &str,
        flow_id: &str,
        step: &str,
    ) -> anyhow::Result<bool> {
        let scoped = self.scoped(instance_id);
        let result = sqlx::query(
            "UPDATE auth_states SET step = $1 WHERE instance_id = $2 AND id = $3 AND type = 'login_flow'",
        )
        .bind(step)
        .bind(scoped.instance_id())
        .bind(flow_id)
        .execute(scoped.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn advance_login_flow_to_password(
        &self,
        instance_id: &str,
        flow_id: &str,
        user_id: &str,
        data: &Value,
    ) -> anyhow::Result<bool> {
        let scoped = self.scoped(instance_id);
        let sql = format!(
            "UPDATE auth_states SET step = 'password', user_id = $1, data = {} WHERE instance_id = $2 AND id = $3",
            scoped.json_bind(4),
        );
        let result = sqlx::query(&sql)
            .bind(user_id)
            .bind(scoped.instance_id())
            .bind(flow_id)
            .bind(serde_json::to_string(data).unwrap_or_else(|_| "{}".into()))
            .execute(scoped.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn complete_login_flow(&self, instance_id: &str, flow_id: &str) -> anyhow::Result<bool> {
        let scoped = self.scoped(instance_id);
        let result = sqlx::query(
            "UPDATE auth_states SET step = 'complete', done = 1 WHERE instance_id = $1 AND id = $2",
        )
        .bind(scoped.instance_id())
        .bind(flow_id)
        .execute(scoped.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn load_auth_request_redirect(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        if auth_request_id.is_empty() {
            return Ok(None);
        }

        let scoped = self.scoped(instance_id);
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT redirect_uri, COALESCE(state, '') FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2",
        )
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .fetch_optional(scoped.pool())
        .await?;

        match row {
            Some((redirect_uri, state)) => Ok(Some(AuthRequestRedirect {
                redirect_uri,
                state,
            })),
            None => anyhow::bail!("auth request not found for instance {instance_id}"),
        }
    }

    async fn complete_auth_request(
        &self,
        instance_id: &str,
        auth_request_id: &str,
        user_id: &str,
        code: &str,
    ) -> anyhow::Result<()> {
        if auth_request_id.is_empty() {
            return Ok(());
        }

        let scoped = self.scoped(instance_id);
        let result = sqlx::query(
            "UPDATE oidc_auth_requests SET user_id = $1, done = 1, auth_time = CURRENT_TIMESTAMP, code = $2 WHERE instance_id = $3 AND id = $4",
        )
        .bind(user_id)
        .bind(code)
        .bind(scoped.instance_id())
        .bind(auth_request_id)
        .execute(scoped.pool())
        .await?;

        if result.rows_affected() == 0 {
            anyhow::bail!("auth request not found for instance {instance_id}");
        }

        Ok(())
    }

    async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<Vec<String>> {
        let scoped = self.scoped(instance_id);
        let prompt = scoped.as_text("prompt");
        let sql = format!(
            "SELECT COALESCE({prompt}, '[]') FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2"
        );
        let row: Option<(String,)> = sqlx::query_as(&sql)
            .bind(scoped.instance_id())
            .bind(auth_request_id)
            .fetch_optional(scoped.pool())
            .await?;

        Ok(row
            .and_then(|(prompt_json,)| serde_json::from_str::<Vec<String>>(&prompt_json).ok())
            .unwrap_or_default())
    }

    async fn create_provider_auth_state(
        &self,
        instance_id: &str,
        state: &ProviderAuthState,
    ) -> anyhow::Result<()> {
        let scoped = self.scoped(instance_id);
        sqlx::query(
            "INSERT INTO oidc_rp_auth_states (id, instance_id, provider_id, state, nonce, pkce_verifier, flow_id, redirect_uri, expected_issuer, callback_uri) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(scoped.instance_id())
        .bind(&state.provider_id)
        .bind(&state.state)
        .bind(&state.nonce)
        .bind(&state.pkce_verifier)
        .bind(&state.flow_id)
        .bind(&state.redirect_uri)
        .bind(&state.expected_issuer)
        .bind(&state.callback_uri)
        .execute(scoped.pool())
        .await?;
        Ok(())
    }

    async fn consume_provider_auth_state(
        &self,
        instance_id: &str,
        state: &str,
    ) -> anyhow::Result<Option<ProviderAuthState>> {
        let scoped = self.scoped(instance_id);
        let mut tx = scoped.pool().begin().await?;
        let row: Option<(String, String, String, String, String, String, String, String)> =
            sqlx::query_as(
                "SELECT provider_id, state, nonce, pkce_verifier, flow_id, redirect_uri, expected_issuer, callback_uri \
                 FROM oidc_rp_auth_states WHERE instance_id = $1 AND state = $2",
            )
            .bind(scoped.instance_id())
            .bind(state)
            .fetch_optional(&mut *tx)
            .await?;

        let Some(row) = row else {
            tx.rollback().await?;
            return Ok(None);
        };

        sqlx::query("DELETE FROM oidc_rp_auth_states WHERE instance_id = $1 AND state = $2")
            .bind(scoped.instance_id())
            .bind(state)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(Some(ProviderAuthState {
            provider_id: row.0,
            state: row.1,
            nonce: row.2,
            pkce_verifier: row.3,
            flow_id: row.4,
            redirect_uri: row.5,
            expected_issuer: row.6,
            callback_uri: row.7,
        }))
    }
}

fn map_session_row(
    row: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<String>,
        Option<String>,
    ),
) -> SessionRecord {
    SessionRecord {
        id: row.0,
        user_id: row.1,
        org_id: row.2,
        token_hash: row.3,
        user_agent: row.4,
        ip_address: row.5,
        metadata: Value::Object(Default::default()),
        created_at: row.6,
        expires_at: row.7,
        revoked_at: row.8,
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
    ) -> anyhow::Result<CreatedSession> {
        let session = self
            .kv
            .create_session(instance_id, user_id, org_id, user_agent, ip_address)
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
            tracing::warn!(%error, session_id = %session.session_id, "transient sink emit failed");
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
                tracing::warn!(%error, %session_id, "transient sink emit failed");
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
            tracing::warn!(%error, flow_id = %input.flow_id, "transient sink emit failed");
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
                tracing::warn!(%error, %flow_id, "transient sink emit failed");
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
                tracing::warn!(%error, %flow_id, "transient sink emit failed");
            }
        }
        Ok(changed)
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
                tracing::warn!(%error, %flow_id, "transient sink emit failed");
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
            tracing::warn!(%error, %auth_request_id, "transient sink emit failed");
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
        self.kv.consume_provider_auth_state(instance_id, state).await
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
