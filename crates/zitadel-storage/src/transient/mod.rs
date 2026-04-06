use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;
use zitadel_db::Db;

mod auth_request;
mod dispatch;
mod kv_memory;
mod kv_spanner;
mod kv_sql;
mod login_flow;
mod provider;
mod semantics;
mod sessions;
mod sinks;

use self::semantics::{
    SessionLookupOutcome, TransientStateMeta, TransientStateOutcome, default_transient_state_meta,
    session_lookup_outcome, transient_state_outcome,
};

pub use self::dispatch::{DefaultKvStore, DefaultSink, DefaultTransientStorage};
pub use self::kv_memory::MemoryKvStore;
pub use self::kv_spanner::SpannerKvStore;
pub use self::kv_sql::SqlKvStore;
pub use self::sinks::{ChannelSink, NoopSink, SqlSink};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub user_id: String,
    pub org_id: String,
    pub token_hash: String,
    pub user_agent: String,
    pub ip_address: String,
    pub fingerprint: String,
    pub metadata: Value,
    pub created_at: String,
    pub created_at_epoch: u64,
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
    pub fingerprint: String,
    pub metadata: Value,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatedSession {
    pub session_id: String,
    pub token: String,
    pub created_at: String,
    pub created_at_epoch: u64,
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AuthRequestRequirements {
    pub prompt: Vec<String>,
    pub max_age: Option<u64>,
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
        session_id: Option<String>,
        code: String,
    },
    ProviderAuthStateCreated {
        instance_id: String,
        state: ProviderAuthState,
    },
}

pub trait KvStore: Clone + Send + Sync + 'static {
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
        session_id: Option<&str>,
        code: &str,
        auth_time: Option<&str>,
    ) -> anyhow::Result<Option<AuthRequestRedirect>>;

    async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<AuthRequestRequirements>;

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
                    fingerprint: persisted.fingerprint,
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
        session_id: Option<&str>,
        code: &str,
        auth_time: Option<&str>,
    ) -> anyhow::Result<Option<AuthRequestRedirect>> {
        let redirect = self
            .kv
            .complete_auth_request(
                instance_id,
                auth_request_id,
                user_id,
                session_id,
                code,
                auth_time,
            )
            .await?;
        if let Err(error) = self
            .sink
            .emit(TransientRecord::AuthRequestCompleted {
                instance_id: instance_id.to_string(),
                auth_request_id: auth_request_id.to_string(),
                user_id: user_id.to_string(),
                session_id: session_id.map(ToString::to_string),
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
        Ok(redirect)
    }

    pub async fn load_auth_request_prompts(
        &self,
        instance_id: &str,
        auth_request_id: &str,
    ) -> anyhow::Result<AuthRequestRequirements> {
        self.kv
            .load_auth_request_prompts(instance_id, auth_request_id)
            .await
    }

    pub async fn create_provider_auth_state(
        &self,
        instance_id: &str,
        state: &ProviderAuthState,
    ) -> anyhow::Result<()> {
        self.kv
            .create_provider_auth_state(instance_id, state)
            .await?;
        if self.kv.should_emit_provider_state_record()
            && let Err(error) = self
                .sink
                .emit(TransientRecord::ProviderAuthStateCreated {
                    instance_id: instance_id.to_string(),
                    state: state.clone(),
                })
                .await
        {
            tracing::warn!(
                stream = "event_pusher",
                %error,
                provider_id = %state.provider_id,
                state = %state.state,
                "transient sink emit failed"
            );
        }
        Ok(())
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

// --- Helper functions used by backends and sinks ---

#[cfg(test)]
fn parse_duration(raw: &str) -> tokio::time::Duration {
    use tokio::time::Duration;
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

pub(self) async fn apply_channel_batch(
    db: &Db,
    pending: &mut Vec<TransientRecord>,
) -> anyhow::Result<()> {
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
    session_max_age_secs: u64,
) -> anyhow::Result<(String, u64, Option<String>)> {
    let scoped = db.scoped(instance_id.to_string());
    let max_age = session_max_age_secs.max(1);
    let sql = match db.dialect() {
        zitadel_db::Dialect::Postgres => {
            format!(
                "SELECT CURRENT_TIMESTAMP::text, EXTRACT(EPOCH FROM CURRENT_TIMESTAMP)::bigint, (CURRENT_TIMESTAMP + INTERVAL '{max_age} seconds')::text"
            )
        }
        zitadel_db::Dialect::Spanner => {
            format!(
                "SELECT CAST(CURRENT_TIMESTAMP() AS STRING), UNIX_SECONDS(CURRENT_TIMESTAMP()), CAST(TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL {max_age} SECOND) AS STRING)"
            )
        }
        zitadel_db::Dialect::Sqlite => {
            format!(
                "SELECT datetime('now'), CAST(strftime('%s', 'now') AS INTEGER), datetime('now', '+{max_age} seconds')"
            )
        }
    };
    let row: (String, i64, String) = sqlx::query_as(&sql).fetch_one(scoped.pool()).await?;
    Ok((row.0, row.1 as u64, Some(row.2)))
}

async fn current_timestamp(db: &Db, instance_id: &str) -> anyhow::Result<String> {
    let scoped = db.scoped(instance_id.to_string());
    let sql = match db.dialect() {
        zitadel_db::Dialect::Postgres => "SELECT CURRENT_TIMESTAMP::text",
        zitadel_db::Dialect::Spanner => "SELECT CAST(CURRENT_TIMESTAMP() AS STRING)",
        zitadel_db::Dialect::Sqlite => "SELECT datetime('now')",
    };
    let row: (String,) = sqlx::query_as(sql).fetch_one(scoped.pool()).await?;
    Ok(row.0)
}

async fn sql_user_is_active(db: &Db, instance_id: &str, user_id: &str) -> anyhow::Result<bool> {
    let scoped = db.scoped(instance_id.to_string());
    let active = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM users WHERE instance_id = $1 AND id = $2 AND state = 'active' LIMIT 1",
    )
    .bind(scoped.instance_id())
    .bind(user_id)
    .fetch_optional(scoped.pool())
    .await?;
    Ok(active.is_some())
}

pub(self) async fn ensure_sink_inbox_table(db: &Db) -> anyhow::Result<()> {
    let sql = match db.dialect() {
        zitadel_db::Dialect::Postgres => {
            "CREATE TABLE IF NOT EXISTS storage_sink_inbox (
                id TEXT PRIMARY KEY,
                record_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
            )"
        }
        zitadel_db::Dialect::Spanner => {
            "CREATE TABLE IF NOT EXISTS storage_sink_inbox (
                id STRING(MAX) NOT NULL,
                record_type STRING(MAX) NOT NULL,
                payload STRING(MAX) NOT NULL,
                created_at TIMESTAMP NOT NULL OPTIONS (allow_commit_timestamp = true)
            ) PRIMARY KEY(id)"
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
    sqlx::query("CREATE INDEX IF NOT EXISTS idx_storage_sink_inbox_created_at ON storage_sink_inbox(created_at)")
        .execute(db.pool())
        .await?;
    Ok(())
}

pub async fn prepare_postgres_kv_schema(db: &Db, unlogged: bool) -> anyhow::Result<()> {
    anyhow::ensure!(
        db.dialect() == zitadel_db::Dialect::Postgres,
        "postgres KV schema preparation requires a Postgres database"
    );

    let table_prefix = if unlogged { "UNLOGGED " } else { "" };
    let ddl = [
        format!(
            "CREATE {table_prefix}TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL DEFAULT 'default',
                user_id TEXT NOT NULL,
                org_id TEXT NOT NULL DEFAULT '1',
                token_hash TEXT NOT NULL DEFAULT '',
                user_agent TEXT DEFAULT '',
                ip_address TEXT DEFAULT '',
                metadata JSONB NOT NULL DEFAULT '{{}}'::jsonb,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                last_active_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ,
                revoked_at TIMESTAMPTZ,
                fingerprint TEXT DEFAULT ''
            )"
        ),
        format!(
            "CREATE {table_prefix}TABLE IF NOT EXISTS auth_states (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL DEFAULT 'default',
                type TEXT NOT NULL DEFAULT '',
                state TEXT NOT NULL DEFAULT '',
                redirect_uri TEXT NOT NULL DEFAULT '',
                data JSONB NOT NULL DEFAULT '{{}}'::jsonb,
                step TEXT NOT NULL DEFAULT '',
                user_id TEXT NOT NULL DEFAULT '',
                done BOOLEAN NOT NULL DEFAULT FALSE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )"
        ),
        format!(
            "CREATE {table_prefix}TABLE IF NOT EXISTS oidc_auth_requests (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL DEFAULT 'default',
                client_id TEXT NOT NULL,
                redirect_uri TEXT NOT NULL DEFAULT '',
                scope TEXT NOT NULL DEFAULT '',
                state TEXT NOT NULL DEFAULT '',
                nonce TEXT NOT NULL DEFAULT '',
                response_type TEXT NOT NULL DEFAULT 'code',
                code_challenge TEXT NOT NULL DEFAULT '',
                code_challenge_method TEXT NOT NULL DEFAULT '',
                prompt JSONB NOT NULL DEFAULT '[]'::jsonb,
                login_hint TEXT NOT NULL DEFAULT '',
                user_id TEXT NOT NULL DEFAULT '',
                session_id TEXT NOT NULL DEFAULT '',
                code TEXT NOT NULL DEFAULT '',
                done BOOLEAN NOT NULL DEFAULT FALSE,
                auth_time TIMESTAMPTZ,
                max_age BIGINT,
                expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes'),
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )"
        ),
        format!(
            "CREATE {table_prefix}TABLE IF NOT EXISTS oidc_rp_auth_states (
                id TEXT PRIMARY KEY,
                instance_id TEXT NOT NULL DEFAULT 'default',
                provider_id TEXT NOT NULL DEFAULT '',
                state TEXT NOT NULL,
                nonce TEXT NOT NULL DEFAULT '',
                pkce_verifier TEXT NOT NULL DEFAULT '',
                flow_id TEXT NOT NULL DEFAULT '',
                redirect_uri TEXT NOT NULL DEFAULT '',
                expected_issuer TEXT NOT NULL DEFAULT '',
                callback_uri TEXT NOT NULL DEFAULT '',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                expires_at TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes')
            )"
        ),
        "CREATE INDEX IF NOT EXISTS idx_sessions_instance ON sessions(instance_id)".to_string(),
        "CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash)".to_string(),
        "CREATE INDEX IF NOT EXISTS idx_auth_states_instance ON auth_states(instance_id)".to_string(),
        "CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance ON oidc_auth_requests(instance_id, created_at)".to_string(),
        "CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_code ON oidc_auth_requests(instance_id, code) WHERE code != ''".to_string(),
        "CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_client ON oidc_auth_requests(instance_id, client_id)".to_string(),
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_state ON oidc_rp_auth_states(instance_id, state)".to_string(),
        "CREATE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_provider ON oidc_rp_auth_states(instance_id, provider_id)".to_string(),
    ];

    for statement in ddl {
        sqlx::query(&statement).execute(db.pool()).await?;
    }

    Ok(())
}

pub async fn prepare_postgres_sink_schema(db: &Db) -> anyhow::Result<()> {
    anyhow::ensure!(
        db.dialect() == zitadel_db::Dialect::Postgres,
        "postgres sink schema preparation requires a Postgres database"
    );
    ensure_sink_inbox_table(db).await
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
        TransientRecord::ProviderAuthStateCreated { .. } => "provider_auth_state.created",
    }
}

pub(self) async fn insert_sink_record(db: &Db, record: &TransientRecord) -> anyhow::Result<()> {
    let payload = serde_json::to_string(record)?;
    sqlx::query("INSERT INTO storage_sink_inbox (id, record_type, payload) VALUES ($1, $2, $3)")
        .bind(Uuid::new_v4().to_string())
        .bind(record_type(record))
        .bind(payload)
        .execute(db.pool())
        .await?;
    Ok(())
}

pub(self) async fn drain_sink_inbox(
    buffer_db: &Db,
    target_db: &Db,
    batch_size: usize,
) -> anyhow::Result<()> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, payload FROM storage_sink_inbox ORDER BY created_at ASC LIMIT $1",
    )
    .bind(batch_size as i64)
    .fetch_all(buffer_db.pool())
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
    apply_transient_records(target_db, &records).await?;

    let mut tx = buffer_db.pool().begin().await?;
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
                let created_at_bind = match db.dialect() {
                    zitadel_db::Dialect::Postgres => "$10::timestamptz",
                    zitadel_db::Dialect::Spanner => "$10",
                    zitadel_db::Dialect::Sqlite => "$10",
                };
                let expires_at_bind = match db.dialect() {
                    zitadel_db::Dialect::Postgres => "$11::timestamptz",
                    zitadel_db::Dialect::Spanner => "$11",
                    zitadel_db::Dialect::Sqlite => "$11",
                };
                let sql = format!(
                    "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, fingerprint, metadata, created_at, expires_at) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, {}, {created_at_bind}, {expires_at_bind}) \
                     ON CONFLICT(instance_id, id) DO UPDATE SET user_id = EXCLUDED.user_id, org_id = EXCLUDED.org_id, token_hash = EXCLUDED.token_hash, user_agent = EXCLUDED.user_agent, ip_address = EXCLUDED.ip_address, fingerprint = EXCLUDED.fingerprint, metadata = EXCLUDED.metadata, expires_at = EXCLUDED.expires_at",
                    scoped.json_bind(9),
                );
                sqlx::query(&sql)
                    .bind(&session.id)
                    .bind(scoped.instance_id())
                    .bind(&session.user_id)
                    .bind(&session.org_id)
                    .bind(&session.token_hash)
                    .bind(&session.user_agent)
                    .bind(&session.ip_address)
                    .bind(&session.fingerprint)
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
                     ON CONFLICT(instance_id, id) DO NOTHING",
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
                    "UPDATE auth_states SET step = $1 \
                     WHERE instance_id = $2 AND id = $3 AND type = 'login_flow' \
                       AND done = 0 AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
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
                    "UPDATE auth_states SET step = 'password', user_id = $1, data = {} \
                     WHERE instance_id = $2 AND id = $3 AND type = 'login_flow' \
                       AND done = 0 AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
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
                    "UPDATE auth_states SET data = {} \
                     WHERE instance_id = $1 AND id = $2 AND type = 'login_flow' \
                       AND done = 0 AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
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
                    "UPDATE auth_states SET step = 'complete', done = 1 \
                     WHERE instance_id = $1 AND id = $2 AND type = 'login_flow' \
                       AND done = 0 AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
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
                session_id,
                code,
            } => {
                sqlx::query(
                    "UPDATE oidc_auth_requests SET user_id = $1, session_id = $2, done = 1, auth_time = CURRENT_TIMESTAMP, code = $3 \
                     WHERE instance_id = $4 AND id = $5 AND done = 0 \
                       AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)",
                )
                .bind(user_id)
                .bind(session_id.as_deref().unwrap_or_default())
                .bind(code)
                .bind(instance_id)
                .bind(auth_request_id)
                .execute(&mut *tx)
                .await?;
            }
            TransientRecord::ProviderAuthStateCreated { instance_id, state } => {
                sqlx::query(
                    "INSERT INTO oidc_rp_auth_states \
                     (instance_id, id, provider_id, state, nonce, pkce_verifier, flow_id, redirect_uri, expected_issuer, callback_uri) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) \
                     ON CONFLICT(instance_id, state) DO NOTHING",
                )
                .bind(instance_id)
                .bind(Uuid::new_v4().to_string())
                .bind(&state.provider_id)
                .bind(&state.state)
                .bind(&state.nonce)
                .bind(&state.pkce_verifier)
                .bind(&state.flow_id)
                .bind(&state.redirect_uri)
                .bind(&state.expected_issuer)
                .bind(&state.callback_uri)
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
    use tokio::time::Duration;

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
        let storage =
            TransientStorage::new(SqlKvStore::local_only(db.clone(), 86_400), sink.clone());

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

        let storage = TransientStorage::new(MemoryKvStore::new(db.clone(), 86_400), FailingSink);
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
    async fn sql_session_lookup_rejects_disabled_users() {
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
        sqlx::query(
            "INSERT INTO users (id, instance_id, org_id, identifier, user_type, state) VALUES ($1, $2, $3, $4, $5, 'active')",
        )
        .bind("user-1")
        .bind(scoped.instance_id())
        .bind("org-1")
        .bind("alice")
        .bind("human")
        .execute(scoped.pool())
        .await
        .unwrap();

        let store = SqlKvStore::local_only(db.clone(), 86_400);
        let created = store
            .create_session(
                zitadel_db::DEFAULT_INSTANCE_ID,
                "user-1",
                "org-1",
                "ua",
                "127.0.0.1",
                "fp-disabled",
            )
            .await
            .unwrap();

        sqlx::query("UPDATE users SET state = 'disabled' WHERE instance_id = $1 AND id = $2")
            .bind(scoped.instance_id())
            .bind("user-1")
            .execute(scoped.pool())
            .await
            .unwrap();

        let found = store
            .find_session_by_token(zitadel_db::DEFAULT_INSTANCE_ID, &created.token)
            .await
            .unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn memory_session_lookup_rejects_disabled_users() {
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
        sqlx::query(
            "INSERT INTO users (id, instance_id, org_id, identifier, user_type, state) VALUES ($1, $2, $3, $4, $5, 'active')",
        )
        .bind("user-1")
        .bind(scoped.instance_id())
        .bind("org-1")
        .bind("alice")
        .bind("human")
        .execute(scoped.pool())
        .await
        .unwrap();

        let store = MemoryKvStore::new(db.clone(), 86_400);
        let created = store
            .create_session(
                zitadel_db::DEFAULT_INSTANCE_ID,
                "user-1",
                "org-1",
                "ua",
                "127.0.0.1",
                "fp-disabled",
            )
            .await
            .unwrap();

        sqlx::query("UPDATE users SET state = 'disabled' WHERE instance_id = $1 AND id = $2")
            .bind(scoped.instance_id())
            .bind("user-1")
            .execute(scoped.pool())
            .await
            .unwrap();

        let found = store
            .find_session_by_token(zitadel_db::DEFAULT_INSTANCE_ID, &created.token)
            .await
            .unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn provider_auth_state_is_consumed_once() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let storage = TransientStorage::new(MemoryKvStore::new(db, 86_400), NoopSink);

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
    async fn provider_auth_state_survives_sqlite_replay_and_stays_consume_once() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();

        let sink = ChannelSink::new(db.clone(), 16, 4, parse_duration("10ms"));
        let storage = TransientStorage::new(MemoryKvStore::new(db.clone(), 86_400), sink);
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

        tokio::time::sleep(Duration::from_millis(40)).await;

        let restarted = MemoryKvStore::new(db.clone(), 86_400);
        let first = restarted
            .consume_provider_auth_state(zitadel_db::DEFAULT_INSTANCE_ID, "state-1")
            .await
            .unwrap();
        let second = restarted
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
        let storage = TransientStorage::new(MemoryKvStore::new(db.clone(), 86_400), sink);
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

        let found = SqlKvStore::local_only(db, 86_400)
            .find_session_by_token(zitadel_db::DEFAULT_INSTANCE_ID, &created.token)
            .await
            .unwrap();
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn channel_sink_replay_preserves_session_fingerprint() {
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
        let storage = TransientStorage::new(MemoryKvStore::new(db.clone(), 86_400), sink);
        let created = storage
            .create_session(
                zitadel_db::DEFAULT_INSTANCE_ID,
                "user-1",
                "org-1",
                "ua",
                "127.0.0.1",
                "fp-preserved",
            )
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(40)).await;

        let replayed = SqlKvStore::local_only(db, 86_400)
            .get_session(zitadel_db::DEFAULT_INSTANCE_ID, &created.session_id)
            .await
            .unwrap()
            .expect("replayed session");
        assert_eq!(replayed.fingerprint, "fp-preserved");
    }

    #[tokio::test]
    async fn memory_revoked_session_does_not_fall_back_to_stale_sqlite_copy() {
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

        let store = MemoryKvStore::new(db.clone(), 86_400);
        let sink = ChannelSink::new(db.clone(), 16, 4, parse_duration("10ms"));
        let storage = TransientStorage::new(store.clone(), sink);
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

        {
            let mut state = store.state.write().await;
            let entry = state
                .sessions
                .get_mut(zitadel_db::DEFAULT_INSTANCE_ID)
                .and_then(|sessions| sessions.get_mut(&created.session_id))
                .expect("memory session present");
            entry.record.revoked_at = Some("revoked".into());
            entry.meta.revoked = true;
        }

        let found = store
            .find_session_by_token(zitadel_db::DEFAULT_INSTANCE_ID, &created.token)
            .await
            .unwrap();
        assert!(found.is_none());

        let stale_sql = SqlKvStore::local_only(db.clone(), 86_400)
            .find_session_by_token(zitadel_db::DEFAULT_INSTANCE_ID, &created.token)
            .await
            .unwrap();
        assert!(stale_sql.is_some());
    }

    #[tokio::test]
    async fn memory_expired_session_does_not_fall_back_to_stale_sqlite_copy() {
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

        let store = MemoryKvStore::new(db.clone(), 86_400);
        let sink = ChannelSink::new(db.clone(), 16, 4, parse_duration("10ms"));
        let storage = TransientStorage::new(store.clone(), sink);
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

        {
            let mut state = store.state.write().await;
            let entry = state
                .sessions
                .get_mut(zitadel_db::DEFAULT_INSTANCE_ID)
                .and_then(|sessions| sessions.get_mut(&created.session_id))
                .expect("memory session present");
            entry.meta.expires_at_epoch = Some(semantics::now_epoch_secs().saturating_sub(1));
        }

        let found = store
            .find_session_by_token(zitadel_db::DEFAULT_INSTANCE_ID, &created.token)
            .await
            .unwrap();
        assert!(found.is_none());

        let stale_sql = SqlKvStore::local_only(db.clone(), 86_400)
            .find_session_by_token(zitadel_db::DEFAULT_INSTANCE_ID, &created.token)
            .await
            .unwrap();
        assert!(stale_sql.is_some());
    }

    #[tokio::test]
    async fn expired_login_flow_is_inactive_for_reads_and_mutations() {
        let db = Db::open("").await.unwrap();
        let store = MemoryKvStore::new(db, 86_400);
        let flow = NewLoginFlowState {
            flow_id: "flow-1".into(),
            state: "state-1".into(),
            redirect_uri: "/console".into(),
            data: serde_json::json!({"step": "identifier"}),
        };

        store
            .create_login_flow(zitadel_db::DEFAULT_INSTANCE_ID, &flow)
            .await
            .unwrap();

        {
            let mut state = store.state.write().await;
            let flow = state
                .login_flows
                .get_mut(zitadel_db::DEFAULT_INSTANCE_ID)
                .and_then(|flows| flows.get_mut("flow-1"))
                .expect("flow present");
            flow.meta.expires_at_epoch = Some(semantics::now_epoch_secs().saturating_sub(1));
        }

        assert!(
            store
                .load_login_flow(zitadel_db::DEFAULT_INSTANCE_ID, "flow-1")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            !store
                .set_login_flow_step(zitadel_db::DEFAULT_INSTANCE_ID, "flow-1", "password")
                .await
                .unwrap()
        );
        assert!(
            !store
                .complete_login_flow(zitadel_db::DEFAULT_INSTANCE_ID, "flow-1")
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn completed_login_flow_is_not_reusable() {
        let db = Db::open("").await.unwrap();
        let store = MemoryKvStore::new(db, 86_400);
        let flow = NewLoginFlowState {
            flow_id: "flow-1".into(),
            state: "state-1".into(),
            redirect_uri: "/console".into(),
            data: serde_json::json!({}),
        };

        store
            .create_login_flow(zitadel_db::DEFAULT_INSTANCE_ID, &flow)
            .await
            .unwrap();
        assert!(
            store
                .complete_login_flow(zitadel_db::DEFAULT_INSTANCE_ID, "flow-1")
                .await
                .unwrap()
        );
        assert!(
            store
                .load_login_flow(zitadel_db::DEFAULT_INSTANCE_ID, "flow-1")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            !store
                .update_login_flow_data(
                    zitadel_db::DEFAULT_INSTANCE_ID,
                    "flow-1",
                    &serde_json::json!({"reused": true}),
                )
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn expired_auth_request_is_unusable_in_sql_store() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();

        sqlx::query(
            "INSERT INTO oidc_auth_requests (id, instance_id, client_id, redirect_uri, state, prompt, max_age, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, datetime('now', '-1 minute'))",
        )
        .bind("authreq-1")
        .bind(scoped.instance_id())
        .bind("client-1")
        .bind("http://localhost/callback")
        .bind("state-1")
        .bind(r#"["login"]"#)
        .bind(300_i64)
        .execute(scoped.pool())
        .await
        .unwrap();

        let store = SqlKvStore::local_only(db.clone(), 86_400);
        assert!(
            store
                .load_auth_request_redirect(zitadel_db::DEFAULT_INSTANCE_ID, "authreq-1")
                .await
                .is_err()
        );

        let prompts = store
            .load_auth_request_prompts(zitadel_db::DEFAULT_INSTANCE_ID, "authreq-1")
            .await
            .unwrap();
        assert_eq!(prompts, AuthRequestRequirements::default());

        assert!(
            store
                .complete_auth_request(
                    zitadel_db::DEFAULT_INSTANCE_ID,
                    "authreq-1",
                    "user-1",
                    None,
                    "code-1",
                    None,
                )
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn expired_provider_auth_state_cannot_be_consumed_in_sql_store() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();

        sqlx::query(
            "INSERT INTO oidc_rp_auth_states (id, instance_id, provider_id, state, nonce, pkce_verifier, flow_id, redirect_uri, expected_issuer, callback_uri, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, datetime('now', '-1 minute'))",
        )
        .bind("provider-state-1")
        .bind(scoped.instance_id())
        .bind("provider-1")
        .bind("state-1")
        .bind("nonce-1")
        .bind("verifier-1")
        .bind("flow-1")
        .bind("/console")
        .bind("https://issuer.example")
        .bind("http://localhost:8080/v1/auth/sso/callback")
        .execute(scoped.pool())
        .await
        .unwrap();

        let store = SqlKvStore::local_only(db.clone(), 86_400);
        let consumed = store
            .consume_provider_auth_state(zitadel_db::DEFAULT_INSTANCE_ID, "state-1")
            .await
            .unwrap();
        assert!(consumed.is_none());
    }

    #[tokio::test]
    async fn replay_does_not_reopen_completed_login_flow() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();

        sqlx::query(
            "INSERT INTO auth_states (id, instance_id, type, state, redirect_uri, data, step, done) \
             VALUES ($1, $2, 'login_flow', $3, $4, $5, 'complete', 1)",
        )
        .bind("flow-1")
        .bind(scoped.instance_id())
        .bind("state-1")
        .bind("/console")
        .bind("{}")
        .execute(scoped.pool())
        .await
        .unwrap();

        apply_transient_records(
            &db,
            &[TransientRecord::LoginFlowCreated {
                instance_id: zitadel_db::DEFAULT_INSTANCE_ID.into(),
                flow: NewLoginFlowState {
                    flow_id: "flow-1".into(),
                    state: "state-2".into(),
                    redirect_uri: "/other".into(),
                    data: serde_json::json!({"replayed": true}),
                },
            }],
        )
        .await
        .unwrap();

        let row: (String, i64) =
            sqlx::query_as("SELECT step, done FROM auth_states WHERE instance_id = $1 AND id = $2")
                .bind(scoped.instance_id())
                .bind("flow-1")
                .fetch_one(scoped.pool())
                .await
                .unwrap();
        assert_eq!(row.0, "complete");
        assert_eq!(row.1, 1);
    }
}
