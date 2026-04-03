#![allow(clippy::too_many_arguments, unused_imports)]
mod report;

use std::{
    future::Future,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, bail};
use axum::{Router, http::StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::task::JoinSet;
use uuid::Uuid;
use zitadel_authn::{password::encode_credential_json, session::hash_token};
use zitadel_config::{Config, password::PasswordHasherConfig};
use zitadel_db::{DEFAULT_INSTANCE_ID, Dialect};
use zitadel_fga::{
    AuthorizationModelWriteRequest, BatchCheckItem, BatchCheckRequest, BatchCheckResponse,
    CheckRequest, CheckResponse, Evaluator, FgaService, ModelRepository, StoreResolver, TupleKey,
    TupleKeySet, TupleRepository, TypeDefinition, WriteRequest,
};
use zitadel_testkit::{AuthActor, PatFixture, SessionFixture, TestApp, TestContext, UserFixture};

pub use report::{
    DatasetProfile, DbPerfReport, ScenarioReport, StorageRolesSnapshot, load_report, load_reports,
    render_markdown_summary, write_report,
};

const DEFAULT_POSTGRES_URL: &str = "postgres://postgres:postgres@127.0.0.1:5432/zitadel_perf";
const HOT_PASSWORD: &str = "perf-password-123";
const HOT_USER_AGENT: &str = "zitadel-perf";
const HOT_IP: &str = "127.0.0.1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PerfBackend {
    Sqlite,
    Postgres,
}

impl PerfBackend {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sqlite => "sqlite",
            Self::Postgres => "postgres",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Sqlite => "SQLite",
            Self::Postgres => "PostgreSQL 18",
        }
    }

    fn default_roles(self) -> StorageRolesSnapshot {
        match self {
            Self::Sqlite => StorageRolesSnapshot {
                stateful: "sqlite".into(),
                read: "same_connection".into(),
                kv: "memory".into(),
                sink: "channel".into(),
                process_cache: "memory".into(),
                analytics: "same_stateful".into(),
            },
            Self::Postgres => StorageRolesSnapshot {
                stateful: "postgres".into(),
                read: "same_primary".into(),
                kv: "postgres_unlogged".into(),
                sink: "postgres".into(),
                process_cache: "memory".into(),
                analytics: "same_stateful".into(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkProfile {
    Ci,
    Smoke,
}

impl BenchmarkProfile {
    pub fn name(self) -> &'static str {
        match self {
            Self::Ci => "ci",
            Self::Smoke => "smoke",
        }
    }

    fn dataset(self) -> DatasetProfile {
        match self {
            Self::Ci => DatasetProfile {
                name: self.name().into(),
                users: 10_000,
                active_sessions: 10_000,
                revoked_sessions: 10_000,
                expired_sessions: 10_000,
                fga_tuples: 50_000,
            },
            Self::Smoke => DatasetProfile {
                name: self.name().into(),
                users: 50,
                active_sessions: 50,
                revoked_sessions: 35,
                expired_sessions: 35,
                fga_tuples: 250,
            },
        }
    }

    fn tuning(self) -> ScenarioTuning {
        match self {
            Self::Ci => ScenarioTuning {
                serial_warmup_rounds: 5,
                serial_rounds: 30,
                concurrent_warmup_rounds: 2,
                concurrent_rounds: 6,
                concurrent_workers: 16,
            },
            Self::Smoke => ScenarioTuning {
                serial_warmup_rounds: 1,
                serial_rounds: 3,
                concurrent_warmup_rounds: 1,
                concurrent_rounds: 2,
                concurrent_workers: 4,
            },
        }
    }
}

pub struct RunOptions {
    pub backend: PerfBackend,
    pub profile: BenchmarkProfile,
    pub database_url: Option<String>,
}

pub fn summarize_report_files(
    current_paths: &[PathBuf],
    previous_paths: &[PathBuf],
) -> anyhow::Result<String> {
    let current = load_reports(current_paths)?;
    let previous = load_reports(previous_paths)?;
    Ok(render_markdown_summary(&current, &previous))
}

pub async fn run_db_benchmark(options: RunOptions) -> anyhow::Result<DbPerfReport> {
    let dataset = options.profile.dataset();
    let tuning = options.profile.tuning();
    let mut env = build_environment(options.backend, options.database_url, &dataset).await?;
    let scenarios = collect_scenarios(&env, &dataset.name, tuning).await?;

    env.app.ctx.db.db.close().await;
    if let Some(path) = env.sqlite_file.take() {
        cleanup_sqlite_files(&path);
    }

    Ok(DbPerfReport {
        generated_at_epoch_secs: unix_epoch_now(),
        backend: options.backend,
        profile: options.profile.name().into(),
        storage_roles: options.backend.default_roles(),
        dataset,
        scenarios,
    })
}

struct ScenarioTuning {
    serial_warmup_rounds: u32,
    serial_rounds: u32,
    concurrent_warmup_rounds: u32,
    concurrent_rounds: u32,
    concurrent_workers: usize,
}

struct PerfEnvironment {
    app: Arc<TestApp>,
    sqlite_file: Option<PathBuf>,
    hot_user: UserFixture,
    hot_password: String,
    hot_session: SessionFixture,
    hot_pat: PatFixture,
    store_id: String,
    direct_check_request: CheckRequest,
    nested_check_request: CheckRequest,
    batch_check_request: BatchCheckRequest,
}

impl PerfEnvironment {
    fn ctx(&self) -> &TestContext {
        &self.app.ctx
    }
}

async fn build_environment(
    backend: PerfBackend,
    database_url: Option<String>,
    dataset: &DatasetProfile,
) -> anyhow::Result<PerfEnvironment> {
    let (config, sqlite_file) = benchmark_config(backend, database_url);
    let ctx = TestContext::with_config(config).await?;
    let seeded = seed_dataset(&ctx, dataset).await?;
    let router = perf_router(&ctx);
    let app = Arc::new(TestApp::new(ctx, router));

    Ok(PerfEnvironment {
        app,
        sqlite_file,
        hot_user: seeded.hot_user,
        hot_password: seeded.hot_password,
        hot_session: seeded.hot_session,
        hot_pat: seeded.hot_pat,
        store_id: seeded.store_id,
        direct_check_request: seeded.direct_check_request,
        nested_check_request: seeded.nested_check_request,
        batch_check_request: seeded.batch_check_request,
    })
}

fn benchmark_config(
    backend: PerfBackend,
    database_url: Option<String>,
) -> (Config, Option<PathBuf>) {
    let mut config = Config::default();
    config.server.external_domain = "localhost".into();
    config.server.public_origin = "http://localhost:18080".into();
    config.server.force_insecure_cookies = true;
    config.password_hasher = PasswordHasherConfig::dev_defaults();
    config.storage.stateful.migrate = "auto".into();
    config.storage.stateful.bootstrap = "auto".into();

    match backend {
        PerfBackend::Sqlite => {
            let sqlite_file =
                std::env::temp_dir().join(format!("zitadel-perf-{}.db", Uuid::new_v4().simple()));
            config.storage.stateful.url =
                database_url.unwrap_or_else(|| format!("sqlite://{}", sqlite_file.display()));
            (config, Some(sqlite_file))
        }
        PerfBackend::Postgres => {
            config.storage.stateful.url =
                database_url.unwrap_or_else(|| DEFAULT_POSTGRES_URL.to_string());
            (config, None)
        }
    }
}

fn perf_router(ctx: &TestContext) -> Router {
    Router::new()
        .merge(zitadel_login::routes(ctx.login_state.clone()))
        .merge(zitadel_api::routes(ctx.api_state.clone()))
}

struct SeededDataset {
    hot_user: UserFixture,
    hot_password: String,
    hot_session: SessionFixture,
    hot_pat: PatFixture,
    store_id: String,
    direct_check_request: CheckRequest,
    nested_check_request: CheckRequest,
    batch_check_request: BatchCheckRequest,
}

async fn seed_dataset(
    ctx: &TestContext,
    dataset: &DatasetProfile,
) -> anyhow::Result<SeededDataset> {
    let scoped = ctx.db.scoped_default();
    let org_id = ctx.db.default_org_id().await?;
    let shared_hash = ctx
        .login_state
        .passwords
        .hash(HOT_PASSWORD)
        .context("hash shared perf credential")?;
    let shared_credential = encode_credential_json(&shared_hash);

    let hot_identifier = "perf-user-00000".to_string();
    let mut user_ids = Vec::with_capacity(dataset.users as usize);
    let user_sql = "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state) VALUES ($1, $2, $3, $4, $5, 'human', 'active')";
    let credential_sql = format!(
        "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, $2, $3, 'password', {})",
        scoped.json_bind(4),
    );

    let mut tx = scoped
        .pool()
        .begin()
        .await
        .context("begin perf dataset transaction")?;

    for index in 0..dataset.users {
        let user_id = Uuid::new_v4().to_string();
        let identifier = format!("perf-user-{index:05}");
        sqlx::query(user_sql)
            .bind(&user_id)
            .bind(scoped.instance_id())
            .bind(&org_id)
            .bind(&identifier)
            .bind(&identifier)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("insert perf user {identifier}"))?;

        sqlx::query(&credential_sql)
            .bind(format!("cred-{user_id}"))
            .bind(scoped.instance_id())
            .bind(&user_id)
            .bind(&shared_credential)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("insert credential for perf user {identifier}"))?;

        user_ids.push(user_id);
    }

    seed_persisted_sessions(&scoped, &mut tx, &org_id, &user_ids, dataset).await?;

    tx.commit()
        .await
        .context("commit perf dataset transaction")?;

    let hot_user = UserFixture {
        user_id: user_ids
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("perf dataset requires at least one user"))?,
        org_id: org_id.clone(),
        identifier: hot_identifier,
    };
    let hot_session = ctx.create_session(&hot_user).await?;
    let hot_pat = ctx.create_pat(&hot_user, "perf-admin").await?;
    let (store_id, direct_check_request, nested_check_request, batch_check_request) =
        seed_fga_dataset(ctx, &user_ids, dataset).await?;

    Ok(SeededDataset {
        hot_user,
        hot_password: HOT_PASSWORD.into(),
        hot_session,
        hot_pat,
        store_id,
        direct_check_request,
        nested_check_request,
        batch_check_request,
    })
}

async fn seed_persisted_sessions(
    scoped: &zitadel_db::scoped::ScopedDb,
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    org_id: &str,
    user_ids: &[String],
    dataset: &DatasetProfile,
) -> anyhow::Result<()> {
    let active_sql = match scoped.dialect() {
        Dialect::Sqlite => format!(
            "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, last_active_at, expires_at, revoked_at, fingerprint) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, datetime('now', '-1 hour'), datetime('now', '-5 minutes'), datetime('now', '+1 day'), NULL, $9)",
            scoped.json_bind(8),
        ),
        Dialect::Postgres => format!(
            "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, last_active_at, expires_at, revoked_at, fingerprint) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, NOW() - INTERVAL '1 hour', NOW() - INTERVAL '5 minutes', NOW() + INTERVAL '1 day', NULL, $9)",
            scoped.json_bind(8),
        ),
    };
    let revoked_sql = match scoped.dialect() {
        Dialect::Sqlite => format!(
            "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, last_active_at, expires_at, revoked_at, fingerprint) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, datetime('now', '-2 days'), datetime('now', '-1 day'), datetime('now', '+1 day'), datetime('now', '-12 hours'), $9)",
            scoped.json_bind(8),
        ),
        Dialect::Postgres => format!(
            "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, last_active_at, expires_at, revoked_at, fingerprint) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, NOW() - INTERVAL '2 days', NOW() - INTERVAL '1 day', NOW() + INTERVAL '1 day', NOW() - INTERVAL '12 hours', $9)",
            scoped.json_bind(8),
        ),
    };
    let expired_sql = match scoped.dialect() {
        Dialect::Sqlite => format!(
            "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, last_active_at, expires_at, revoked_at, fingerprint) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, datetime('now', '-2 days'), datetime('now', '-1 day'), datetime('now', '-1 hour'), NULL, $9)",
            scoped.json_bind(8),
        ),
        Dialect::Postgres => format!(
            "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, last_active_at, expires_at, revoked_at, fingerprint) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, NOW() - INTERVAL '2 days', NOW() - INTERVAL '1 day', NOW() - INTERVAL '1 hour', NULL, $9)",
            scoped.json_bind(8),
        ),
    };

    let active_persisted = dataset.active_sessions.saturating_sub(1);
    insert_session_rows(
        scoped,
        tx,
        &active_sql,
        org_id,
        user_ids,
        active_persisted,
        "active",
    )
    .await?;
    insert_session_rows(
        scoped,
        tx,
        &revoked_sql,
        org_id,
        user_ids,
        dataset.revoked_sessions,
        "revoked",
    )
    .await?;
    insert_session_rows(
        scoped,
        tx,
        &expired_sql,
        org_id,
        user_ids,
        dataset.expired_sessions,
        "expired",
    )
    .await?;
    Ok(())
}

async fn insert_session_rows(
    scoped: &zitadel_db::scoped::ScopedDb,
    tx: &mut sqlx::Transaction<'_, sqlx::Any>,
    sql: &str,
    org_id: &str,
    user_ids: &[String],
    total: u32,
    kind: &str,
) -> anyhow::Result<()> {
    if user_ids.is_empty() {
        bail!("perf dataset requires at least one user before seeding sessions");
    }

    for index in 0..total {
        let user_id = &user_ids[index as usize % user_ids.len()];
        let raw_token = format!("perf-{kind}-token-{index}");
        sqlx::query(sql)
            .bind(Uuid::new_v4().to_string())
            .bind(scoped.instance_id())
            .bind(user_id)
            .bind(org_id)
            .bind(hash_token(&raw_token))
            .bind(HOT_USER_AGENT)
            .bind(HOT_IP)
            .bind("{}")
            .bind("")
            .execute(&mut **tx)
            .await
            .with_context(|| format!("insert {kind} perf session {index}"))?;
    }

    Ok(())
}

async fn seed_fga_dataset(
    ctx: &TestContext,
    user_ids: &[String],
    dataset: &DatasetProfile,
) -> anyhow::Result<(String, CheckRequest, CheckRequest, BatchCheckRequest)> {
    let store = ctx
        .api_state
        .fga
        .initialize_instance(DEFAULT_INSTANCE_ID)
        .await
        .context("initialize singleton fga store")?;

    ctx.api_state
        .fga
        .write_model(DEFAULT_INSTANCE_ID, &store.id, perf_authorization_model())
        .await
        .context("write perf authorization model")?;

    let total_folders = user_ids.len();
    let folder_viewer_tuples = dataset.fga_tuples.min(total_folders as u32);
    let remaining = dataset.fga_tuples.saturating_sub(folder_viewer_tuples);
    let nested_tuples = remaining / 2;
    let direct_tuples = remaining - nested_tuples;

    let mut tuples = Vec::with_capacity(dataset.fga_tuples as usize);
    for index in 0..folder_viewer_tuples {
        tuples.push(TupleKey {
            user: format!("user:{}", user_ids[index as usize % user_ids.len()]),
            relation: "viewer".into(),
            object: format!("document:folder-{index:05}"),
            condition: None,
        });
    }
    for index in 0..nested_tuples {
        tuples.push(TupleKey {
            user: format!(
                "document:folder-{:05}",
                index as usize % total_folders.max(1)
            ),
            relation: "parent".into(),
            object: format!("document:doc-nested-{index:05}"),
            condition: None,
        });
    }
    for index in 0..direct_tuples {
        tuples.push(TupleKey {
            user: format!("user:{}", user_ids[index as usize % user_ids.len()]),
            relation: "viewer".into(),
            object: format!("document:doc-direct-{index:05}"),
            condition: None,
        });
    }

    for chunk in tuples.chunks(500) {
        ctx.api_state
            .fga
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: chunk.to_vec(),
                    },
                    deletes: TupleKeySet::default(),
                    authorization_model_id: None,
                },
            )
            .await
            .context("seed perf fga tuples")?;
    }

    let hot_user = format!("user:{}", user_ids[0]);
    let direct = CheckRequest {
        tuple_key: TupleKey {
            user: hot_user.clone(),
            relation: "viewer".into(),
            object: "document:doc-direct-00000".into(),
            condition: None,
        },
        authorization_model_id: None,
        contextual_tuples: None,
        context: None,
    };
    let nested = CheckRequest {
        tuple_key: TupleKey {
            user: hot_user,
            relation: "viewer".into(),
            object: "document:doc-nested-00000".into(),
            condition: None,
        },
        authorization_model_id: None,
        contextual_tuples: None,
        context: None,
    };
    let batch = BatchCheckRequest {
        checks: (0..10)
            .map(|index| BatchCheckItem {
                correlation_id: Some(format!("check-{index}")),
                tuple_key: TupleKey {
                    user: format!("user:{}", user_ids[index as usize % user_ids.len()]),
                    relation: "viewer".into(),
                    object: format!("document:doc-direct-{index:05}"),
                    condition: None,
                },
            })
            .collect(),
        authorization_model_id: None,
        contextual_tuples: None,
        context: None,
    };

    let direct_allowed = ctx
        .api_state
        .fga
        .check(DEFAULT_INSTANCE_ID, &store.id, direct.clone())
        .await
        .context("validate direct perf fga check")?;
    if !direct_allowed.allowed {
        bail!("perf direct FGA dataset check did not resolve to allowed");
    }
    let nested_allowed = ctx
        .api_state
        .fga
        .check(DEFAULT_INSTANCE_ID, &store.id, nested.clone())
        .await
        .context("validate nested perf fga check")?;
    if !nested_allowed.allowed {
        bail!("perf nested FGA dataset check did not resolve to allowed");
    }

    Ok((store.id, direct, nested, batch))
}

fn perf_authorization_model() -> AuthorizationModelWriteRequest {
    let mut types = vec![
        direct_type("user", &[]),
        direct_type("instance", &["owner", "admin", "viewer", "parent"]),
        direct_type("org", &["owner", "admin", "member", "viewer"]),
        direct_type("group", &["member", "admin"]),
        direct_type("project", &["owner", "admin", "member"]),
        direct_type("app", &["admin", "viewer"]),
        direct_type("settings", &["admin", "viewer"]),
        direct_type("session", &["owner"]),
    ];
    types.push(TypeDefinition {
        type_name: "document".into(),
        relations: Map::from_iter([
            ("parent".into(), json!({ "this": {} })),
            (
                "viewer".into(),
                json!({
                    "union": {
                        "child": [
                            { "this": {} },
                            { "tupleToUserset": {
                                "tupleset": { "relation": "parent" },
                                "computedUserset": { "relation": "viewer" }
                            }}
                        ]
                    }
                }),
            ),
        ]),
        metadata: Some(json!({
            "relations": {
                "parent": { "directly_related_user_types": [{ "type": "document" }] },
                "viewer": { "directly_related_user_types": [{ "type": "user" }] }
            }
        })),
    });
    AuthorizationModelWriteRequest {
        schema_version: "1.1".into(),
        type_definitions: types,
        conditions: Map::new(),
    }
}

fn direct_type(type_name: &str, relations: &[&str]) -> TypeDefinition {
    let relation_map = relations
        .iter()
        .map(|relation| (relation.to_string(), json!({ "this": {} })))
        .collect::<Map<String, Value>>();
    let metadata_relations = relations
        .iter()
        .map(|relation| {
            (
                relation.to_string(),
                json!({
                    "directly_related_user_types": [
                        { "type": "user" }
                    ]
                }),
            )
        })
        .collect::<Map<String, Value>>();
    TypeDefinition {
        type_name: type_name.into(),
        relations: relation_map,
        metadata: Some(json!({ "relations": metadata_relations })),
    }
}

async fn collect_scenarios(
    env: &PerfEnvironment,
    dataset_profile: &str,
    tuning: ScenarioTuning,
) -> anyhow::Result<Vec<ScenarioReport>> {
    let mut reports = Vec::new();

    let stateful = env.ctx().login_state.stateful.clone();
    let transient = env.ctx().login_state.transient.clone();
    let passwords = env.ctx().login_state.passwords.clone();
    let identifier = env.hot_user.identifier.clone();
    let password = env.hot_password.clone();
    let user_id = env.hot_user.user_id.clone();
    let org_id = env.hot_user.org_id.clone();
    reports.push(
        measure_serial(
            "auth_password_lookup_and_session_create",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let stateful = stateful.clone();
                let transient = transient.clone();
                let passwords = passwords.clone();
                let identifier = identifier.clone();
                let password = password.clone();
                let user_id = user_id.clone();
                let org_id = org_id.clone();
                async move {
                    let user = stateful
                        .find_active_user_by_identifier(DEFAULT_INSTANCE_ID, &identifier)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("hot perf user missing"))?;
                    if user.user_id != user_id {
                        bail!("perf user lookup returned unexpected user id");
                    }
                    let hash = stateful
                        .load_password_hash(DEFAULT_INSTANCE_ID, &user.user_id)
                        .await?
                        .ok_or_else(|| anyhow::anyhow!("hot perf password hash missing"))?;
                    if passwords.verify(&hash, &password).is_err() {
                        bail!("perf password verification failed");
                    }
                    let session = transient
                        .create_session(
                            DEFAULT_INSTANCE_ID,
                            &user.user_id,
                            &org_id,
                            HOT_USER_AGENT,
                            HOT_IP,
                            "",
                        )
                        .await?;
                    if session.session_id.is_empty() || session.token.is_empty() {
                        bail!("perf session create returned empty identifiers");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let transient = env.ctx().api_state.transient.clone();
    let hot_token = env.hot_session.token.clone();
    reports.push(
        measure_serial(
            "session_lookup_hit",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let transient = transient.clone();
                let hot_token = hot_token.clone();
                async move {
                    let session = transient
                        .find_session_by_token(DEFAULT_INSTANCE_ID, &hot_token)
                        .await?;
                    if session.is_none() {
                        bail!("hot perf session lookup unexpectedly missed");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let transient = env.ctx().api_state.transient.clone();
    reports.push(
        measure_serial(
            "session_lookup_miss",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let transient = transient.clone();
                async move {
                    let token = format!("missing-session-token-{}", Uuid::new_v4().simple());
                    let session = transient
                        .find_session_by_token(DEFAULT_INSTANCE_ID, &token)
                        .await?;
                    if session.is_some() {
                        bail!("missing perf session lookup unexpectedly hit");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let transient = env.ctx().api_state.transient.clone();
    let user_id = env.hot_user.user_id.clone();
    let org_id = env.hot_user.org_id.clone();
    reports.push(
        measure_serial(
            "session_revoke",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let transient = transient.clone();
                let user_id = user_id.clone();
                let org_id = org_id.clone();
                async move {
                    let created = transient
                        .create_session(
                            DEFAULT_INSTANCE_ID,
                            &user_id,
                            &org_id,
                            HOT_USER_AGENT,
                            HOT_IP,
                            "",
                        )
                        .await?;
                    let changed = transient
                        .revoke_session(DEFAULT_INSTANCE_ID, &created.session_id)
                        .await?;
                    if !changed {
                        bail!("perf session revoke reported no change");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let fga = env.ctx().api_state.fga.clone();
    let store_id = env.store_id.clone();
    let direct_request = env.direct_check_request.clone();
    reports.push(
        measure_serial(
            "fga_check_direct",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let fga = fga.clone();
                let store_id = store_id.clone();
                let direct_request = direct_request.clone();
                async move {
                    let response = fga
                        .check(DEFAULT_INSTANCE_ID, &store_id, direct_request)
                        .await?;
                    if !response.allowed {
                        bail!("perf direct FGA check unexpectedly denied");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let fga = env.ctx().api_state.fga.clone();
    let store_id = env.store_id.clone();
    let nested_request = env.nested_check_request.clone();
    reports.push(
        measure_serial(
            "fga_check_nested",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let fga = fga.clone();
                let store_id = store_id.clone();
                let nested_request = nested_request.clone();
                async move {
                    let response = fga
                        .check(DEFAULT_INSTANCE_ID, &store_id, nested_request)
                        .await?;
                    if !response.allowed {
                        bail!("perf nested FGA check unexpectedly denied");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let fga = env.ctx().api_state.fga.clone();
    let store_id = env.store_id.clone();
    let batch_request = env.batch_check_request.clone();
    reports.push(
        measure_serial(
            "fga_batch_check_10",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let fga = fga.clone();
                let store_id = store_id.clone();
                let batch_request = batch_request.clone();
                async move {
                    let response = fga
                        .batch_check(DEFAULT_INSTANCE_ID, &store_id, batch_request)
                        .await?;
                    if response.results.iter().any(|result| !result.allowed) {
                        bail!("perf batch FGA check unexpectedly denied");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let app = env.app.clone();
    let login_body = json!({
        "identifier": env.hot_user.identifier,
        "password": env.hot_password,
    });
    reports.push(
        measure_serial(
            "http_login_password_happy_path",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let app = app.clone();
                let body = login_body.clone();
                async move {
                    let response = app
                        .post_json("/v1/auth/login", AuthActor::Anonymous, &body)
                        .await?;
                    if response.status != StatusCode::OK {
                        bail!("perf http login returned {}", response.status);
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let app = env.app.clone();
    let actor = env.hot_session.bearer_actor();
    reports.push(
        measure_serial(
            "http_whoami_session",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let app = app.clone();
                let actor = actor.clone();
                async move {
                    let response = app.get("/v1/auth/whoami", actor).await?;
                    if response.status != StatusCode::OK {
                        bail!("perf whoami returned {}", response.status);
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let app = env.app.clone();
    let actor = env.hot_pat.actor();
    let path = format!("/v1/fga/stores/{}/check", env.store_id);
    let body =
        serde_json::to_value(&env.direct_check_request).context("serialize fga check body")?;
    reports.push(
        measure_serial(
            "http_fga_check",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let app = app.clone();
                let actor = actor.clone();
                let path = path.clone();
                let body = body.clone();
                async move {
                    let response = app.post_json(&path, actor, &body).await?;
                    if response.status != StatusCode::OK {
                        bail!("perf http fga check returned {}", response.status);
                    }
                    let body: CheckResponse = response.json();
                    if !body.allowed {
                        bail!("perf http fga check unexpectedly denied");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let app = env.app.clone();
    let actor = env.hot_pat.actor();
    let path = format!("/v1/fga/stores/{}/batch-check", env.store_id);
    let body =
        serde_json::to_value(&env.batch_check_request).context("serialize fga batch body")?;
    reports.push(
        measure_serial(
            "http_fga_batch_check",
            dataset_profile,
            tuning.serial_warmup_rounds,
            tuning.serial_rounds,
            move || {
                let app = app.clone();
                let actor = actor.clone();
                let path = path.clone();
                let body = body.clone();
                async move {
                    let response = app.post_json(&path, actor, &body).await?;
                    if response.status != StatusCode::OK {
                        bail!("perf http fga batch check returned {}", response.status);
                    }
                    let payload: BatchCheckResponse = response.json();
                    if payload.results.iter().any(|result| !result.allowed) {
                        bail!("perf http fga batch check unexpectedly denied");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let app = env.app.clone();
    let login_body = json!({
        "identifier": env.hot_user.identifier,
        "password": env.hot_password,
    });
    reports.push(
        measure_concurrent(
            "http_login_password_happy_path_concurrent_16",
            dataset_profile,
            tuning.concurrent_warmup_rounds,
            tuning.concurrent_rounds,
            tuning.concurrent_workers,
            move || {
                let app = app.clone();
                let body = login_body.clone();
                async move {
                    let response = app
                        .post_json("/v1/auth/login", AuthActor::Anonymous, &body)
                        .await?;
                    if response.status != StatusCode::OK {
                        bail!("perf concurrent http login returned {}", response.status);
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let transient = env.ctx().api_state.transient.clone();
    let hot_token = env.hot_session.token.clone();
    reports.push(
        measure_concurrent(
            "session_lookup_hit_concurrent_16",
            dataset_profile,
            tuning.concurrent_warmup_rounds,
            tuning.concurrent_rounds,
            tuning.concurrent_workers,
            move || {
                let transient = transient.clone();
                let hot_token = hot_token.clone();
                async move {
                    let session = transient
                        .find_session_by_token(DEFAULT_INSTANCE_ID, &hot_token)
                        .await?;
                    if session.is_none() {
                        bail!("perf concurrent session lookup unexpectedly missed");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    let app = env.app.clone();
    let actor = env.hot_pat.actor();
    let path = format!("/v1/fga/stores/{}/batch-check", env.store_id);
    let body = serde_json::to_value(&env.batch_check_request)
        .context("serialize concurrent batch body")?;
    reports.push(
        measure_concurrent(
            "http_fga_batch_check_concurrent_16",
            dataset_profile,
            tuning.concurrent_warmup_rounds,
            tuning.concurrent_rounds,
            tuning.concurrent_workers,
            move || {
                let app = app.clone();
                let actor = actor.clone();
                let path = path.clone();
                let body = body.clone();
                async move {
                    let response = app.post_json(&path, actor, &body).await?;
                    if response.status != StatusCode::OK {
                        bail!(
                            "perf concurrent http fga batch check returned {}",
                            response.status
                        );
                    }
                    let payload: BatchCheckResponse = response.json();
                    if payload.results.iter().any(|result| !result.allowed) {
                        bail!("perf concurrent http fga batch check unexpectedly denied");
                    }
                    Ok(())
                }
            },
        )
        .await?,
    );

    Ok(reports)
}

async fn measure_serial<F, Fut>(
    scenario: &str,
    dataset_profile: &str,
    warmup_rounds: u32,
    measured_rounds: u32,
    mut operation: F,
) -> anyhow::Result<ScenarioReport>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<()>>,
{
    for _ in 0..warmup_rounds {
        operation()
            .await
            .with_context(|| format!("warmup failed for {scenario}"))?;
    }

    let total_start = Instant::now();
    let mut durations = Vec::with_capacity(measured_rounds as usize);
    let mut errors = 0u64;
    for _ in 0..measured_rounds {
        let started = Instant::now();
        if operation().await.is_err() {
            errors += 1;
        }
        durations.push(started.elapsed());
    }
    Ok(build_scenario_report(
        scenario,
        dataset_profile,
        warmup_rounds,
        measured_rounds,
        measured_rounds as u64,
        durations,
        errors,
        total_start.elapsed(),
    ))
}

async fn measure_concurrent<F, Fut>(
    scenario: &str,
    dataset_profile: &str,
    warmup_rounds: u32,
    measured_rounds: u32,
    workers: usize,
    operation: F,
) -> anyhow::Result<ScenarioReport>
where
    F: Fn() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    for _ in 0..warmup_rounds {
        let (_, errors) = run_concurrent_round(workers, operation.clone()).await?;
        if errors > 0 {
            bail!("warmup failed for {scenario}");
        }
    }

    let total_start = Instant::now();
    let mut durations = Vec::with_capacity(measured_rounds as usize * workers);
    let mut errors = 0u64;
    for _ in 0..measured_rounds {
        let (round_durations, round_errors) =
            run_concurrent_round(workers, operation.clone()).await?;
        durations.extend(round_durations);
        errors += round_errors;
    }

    Ok(build_scenario_report(
        scenario,
        dataset_profile,
        warmup_rounds,
        measured_rounds,
        measured_rounds as u64 * workers as u64,
        durations,
        errors,
        total_start.elapsed(),
    ))
}

async fn run_concurrent_round<F, Fut>(
    workers: usize,
    operation: F,
) -> anyhow::Result<(Vec<Duration>, u64)>
where
    F: Fn() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    let mut join_set = JoinSet::new();
    for _ in 0..workers {
        let operation = operation.clone();
        join_set.spawn(async move {
            let started = Instant::now();
            let result = operation().await;
            (started.elapsed(), result)
        });
    }

    let mut durations = Vec::with_capacity(workers);
    let mut errors = 0u64;
    while let Some(joined) = join_set.join_next().await {
        let (duration, result) = joined.context("benchmark worker panicked")?;
        durations.push(duration);
        if result.is_err() {
            errors += 1;
        }
    }

    Ok((durations, errors))
}

fn build_scenario_report(
    scenario: &str,
    dataset_profile: &str,
    warmup_rounds: u32,
    measured_rounds: u32,
    total_operations: u64,
    durations: Vec<Duration>,
    error_count: u64,
    elapsed: Duration,
) -> ScenarioReport {
    let mut millis = durations
        .into_iter()
        .map(|duration| duration.as_secs_f64() * 1_000.0)
        .collect::<Vec<_>>();
    millis.sort_by(|left, right| left.partial_cmp(right).unwrap_or(std::cmp::Ordering::Equal));

    let p50_ms = percentile(&millis, 0.50);
    let p95_ms = percentile(&millis, 0.95);
    let max_ms = millis.last().copied().unwrap_or_default();
    let ops_per_sec = if elapsed.is_zero() {
        0.0
    } else {
        total_operations as f64 / elapsed.as_secs_f64()
    };

    ScenarioReport {
        scenario: scenario.into(),
        dataset_profile: dataset_profile.into(),
        warmup_rounds,
        measured_rounds,
        total_operations,
        p50_ms,
        p95_ms,
        max_ms,
        ops_per_sec,
        error_count,
    }
}

fn percentile(values: &[f64], pct: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let index = ((values.len() as f64 * pct).ceil() as usize)
        .saturating_sub(1)
        .min(values.len() - 1);
    values[index]
}

fn unix_epoch_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn cleanup_sqlite_files(path: &PathBuf) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}-shm", path.display()));
    let _ = std::fs::remove_file(format!("{}-wal", path.display()));
    let _ = std::fs::remove_file(format!("{}-journal", path.display()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn smoke_sqlite_run_generates_metrics() {
        let report = run_db_benchmark(RunOptions {
            backend: PerfBackend::Sqlite,
            profile: BenchmarkProfile::Smoke,
            database_url: None,
        })
        .await
        .unwrap();

        assert_eq!(report.backend, PerfBackend::Sqlite);
        assert_eq!(report.profile, "smoke");
        assert!(!report.scenarios.is_empty());
        assert!(
            report
                .scenarios
                .iter()
                .all(|scenario| scenario.total_operations > 0)
        );
    }

    #[tokio::test]
    async fn smoke_postgres_run_works_when_url_is_provided() {
        let Some(database_url) = std::env::var("ZITADEL_TEST_POSTGRES_URL").ok() else {
            return;
        };

        let report = run_db_benchmark(RunOptions {
            backend: PerfBackend::Postgres,
            profile: BenchmarkProfile::Smoke,
            database_url: Some(database_url),
        })
        .await
        .unwrap();

        assert_eq!(report.backend, PerfBackend::Postgres);
        assert!(!report.scenarios.is_empty());
    }
}
