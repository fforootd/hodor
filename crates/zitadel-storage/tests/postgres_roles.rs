use std::time::Duration;

use sqlx::{Executor, postgres::PgPoolOptions};
use uuid::Uuid;
use zitadel_config::{
    KvStoreConfig, ReadStoreConfig, SinkConfig, StatefulStorageConfig, StorageConfig,
};
use zitadel_crypto::token_hash;
use zitadel_db::{Db, migrate};
use zitadel_storage::{
    SessionRecord, Sink, SqlKvStore, SqlSink, StorageRuntime, TransientRecord, TransientStorage,
    prepare_postgres_role_databases,
};

#[tokio::test]
async fn postgres_role_preparation_only_touches_kv_and_sink_databases() -> anyhow::Result<()> {
    let Some(env) = PostgresTestEnv::new().await? else {
        return Ok(());
    };

    let primary_db = Db::open(&env.primary_url).await?;
    migrate::migrate(&primary_db).await?;

    let config = env.storage_config();
    prepare_postgres_role_databases(&config, &primary_db).await?;

    let kv_db = Db::open(&env.kv_url).await?;
    let sink_db = Db::open(&env.sink_url).await?;
    let read_db = Db::open(&env.read_url).await?;

    assert!(table_exists(&kv_db, "sessions").await?);
    assert!(table_exists(&kv_db, "auth_states").await?);
    assert!(table_exists(&kv_db, "oidc_auth_requests").await?);
    assert!(table_exists(&kv_db, "oidc_rp_auth_states").await?);
    assert!(!table_exists(&kv_db, "users").await?);

    assert_eq!(
        table_persistence(&kv_db, "sessions").await?.as_deref(),
        Some("u")
    );

    assert!(table_exists(&sink_db, "storage_sink_inbox").await?);
    assert!(!table_exists(&sink_db, "users").await?);

    assert!(!table_exists(&read_db, "sessions").await?);
    assert!(!table_exists(&read_db, "users").await?);

    Ok(())
}

#[tokio::test]
async fn storage_runtime_uses_distinct_postgres_roles_and_session_fallback() -> anyhow::Result<()> {
    let Some(env) = PostgresTestEnv::new().await? else {
        return Ok(());
    };

    let primary_db = Db::open(&env.primary_url).await?;
    let read_db = Db::open(&env.read_url).await?;
    migrate::migrate(&primary_db).await?;
    migrate::migrate(&read_db).await?;

    let config = env.storage_config();
    prepare_postgres_role_databases(&config, &primary_db).await?;

    seed_user(
        &primary_db,
        "instance-a",
        "org-a",
        "user-a",
        "authoritative@example.com",
    )
    .await?;
    seed_user(
        &read_db,
        "instance-a",
        "org-a",
        "reader-a",
        "reader@example.com",
    )
    .await?;

    let fallback_token = "primary-only-token";
    seed_session(
        &primary_db,
        "instance-a",
        "session-primary",
        "user-a",
        "org-a",
        fallback_token,
    )
    .await?;

    let runtime = StorageRuntime::from_config(&config, primary_db.clone(), 86_400).await?;

    let user = runtime
        .stateful
        .find_active_user_by_identifier("instance-a", "reader@example.com")
        .await?;
    assert_eq!(user.unwrap().user_id, "reader-a");

    let created = runtime
        .transient
        .create_session("instance-a", "user-a", "org-a", "ua", "127.0.0.1", "fp")
        .await?;

    let kv_db = Db::open(&env.kv_url).await?;
    assert!(session_exists(&kv_db, "instance-a", &created.session_id).await?);
    assert!(!session_exists(&primary_db, "instance-a", &created.session_id).await?);

    let found_local = runtime
        .transient
        .find_session_by_token("instance-a", &created.token)
        .await?;
    assert_eq!(found_local.unwrap().id, created.session_id);

    let found_fallback = runtime
        .transient
        .find_session_by_token("instance-a", fallback_token)
        .await?;
    assert_eq!(found_fallback.unwrap().id, "session-primary");

    let fallback_by_id = runtime
        .transient
        .get_session("instance-a", "session-primary")
        .await?;
    assert_eq!(fallback_by_id.unwrap().id, "session-primary");

    let foreign = runtime
        .transient
        .find_session_by_token("instance-b", fallback_token)
        .await?;
    assert!(foreign.is_none());

    Ok(())
}

#[tokio::test]
async fn degraded_mode_buffers_in_sink_and_replays_to_authoritative_postgres() -> anyhow::Result<()>
{
    let Some(env) = PostgresTestEnv::new().await? else {
        return Ok(());
    };

    let primary_db = Db::open(&env.primary_url).await?;
    let bad_target_db = Db::open(&env.read_url).await?;
    let kv_db = Db::open(&env.kv_url).await?;
    let sink_db = Db::open(&env.sink_url).await?;
    migrate::migrate(&primary_db).await?;

    seed_user(
        &primary_db,
        "instance-a",
        "org-a",
        "user-a",
        "alice@example.com",
    )
    .await?;
    zitadel_storage::prepare_postgres_kv_schema(&kv_db, true).await?;
    zitadel_storage::prepare_postgres_sink_schema(&sink_db).await?;

    let bad_sink = SqlSink::new(
        sink_db.clone(),
        bad_target_db.clone(),
        32,
        Duration::from_secs(3600),
    )
    .await?;
    let storage = TransientStorage::new(
        SqlKvStore::new(kv_db.clone(), Some(primary_db.clone()), 86_400),
        bad_sink.clone(),
    );

    let created = storage
        .create_session("instance-a", "user-a", "org-a", "ua", "127.0.0.1", "fp")
        .await?;
    let local = storage
        .find_session_by_token("instance-a", &created.token)
        .await?;
    assert_eq!(local.unwrap().id, created.session_id);

    assert!(bad_sink.drain_once().await.is_err());
    assert_eq!(sink_inbox_count(&sink_db).await?, 1);
    assert!(!session_exists(&primary_db, "instance-a", &created.session_id).await?);

    let good_sink = SqlSink::new(
        sink_db.clone(),
        primary_db.clone(),
        32,
        Duration::from_secs(3600),
    )
    .await?;
    good_sink.drain_once().await?;

    assert_eq!(sink_inbox_count(&sink_db).await?, 0);
    assert!(session_exists(&primary_db, "instance-a", &created.session_id).await?);

    let replay_record = TransientRecord::SessionCreated {
        instance_id: "instance-a".into(),
        session: PersistedSessionRecordForTest::from_session(
            &storage
                .get_session("instance-a", &created.session_id)
                .await?
                .expect("session present in kv"),
        )
        .into(),
    };
    good_sink.emit(replay_record).await?;
    good_sink.drain_once().await?;

    assert_eq!(
        session_count_for_id(&primary_db, "instance-a", &created.session_id).await?,
        1
    );

    Ok(())
}

#[derive(Clone)]
struct PostgresTestEnv {
    primary_url: String,
    read_url: String,
    kv_url: String,
    sink_url: String,
}

impl PostgresTestEnv {
    async fn new() -> anyhow::Result<Option<Self>> {
        let Some(base_url) = std::env::var("ZITADEL_TEST_POSTGRES_URL").ok() else {
            eprintln!("skipping Postgres integration test: ZITADEL_TEST_POSTGRES_URL is not set");
            return Ok(None);
        };

        let admin_url = replace_database_name(&base_url, "postgres");
        let admin = PgPoolOptions::new()
            .max_connections(1)
            .connect(&admin_url)
            .await?;

        let suffix = Uuid::new_v4().simple().to_string();
        let primary = format!("auth_primary_{suffix}");
        let read = format!("auth_read_{suffix}");
        let kv = format!("auth_kv_{suffix}");
        let sink = format!("auth_sink_{suffix}");

        for db_name in [&primary, &read, &kv, &sink] {
            admin
                .execute(sqlx::query(&format!("CREATE DATABASE \"{db_name}\"")))
                .await?;
        }

        Ok(Some(Self {
            primary_url: replace_database_name(&base_url, &primary),
            read_url: replace_database_name(&base_url, &read),
            kv_url: replace_database_name(&base_url, &kv),
            sink_url: replace_database_name(&base_url, &sink),
        }))
    }

    fn storage_config(&self) -> StorageConfig {
        StorageConfig {
            stateful: StatefulStorageConfig {
                url: self.primary_url.clone(),
                ..Default::default()
            },
            read: ReadStoreConfig {
                backend: "postgres_replica".into(),
                url: self.read_url.clone(),
            },
            kv: KvStoreConfig {
                backend: "postgres_unlogged".into(),
                url: self.kv_url.clone(),
            },
            sink: SinkConfig {
                backend: "postgres".into(),
                url: self.sink_url.clone(),
                batch_size: 32,
                flush_interval: "1h".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

async fn seed_user(
    db: &Db,
    instance_id: &str,
    org_id: &str,
    user_id: &str,
    identifier: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(org_id)
    .bind(instance_id)
    .bind(org_id)
    .execute(db.pool())
    .await?;
    sqlx::query(
        "INSERT INTO users (id, instance_id, org_id, identifier, user_type) VALUES ($1, $2, $3, $4, 'human') ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(instance_id)
    .bind(org_id)
    .bind(identifier)
    .execute(db.pool())
    .await?;
    Ok(())
}

async fn seed_session(
    db: &Db,
    instance_id: &str,
    session_id: &str,
    user_id: &str,
    org_id: &str,
    raw_token: &str,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, metadata, fingerprint) \
         VALUES ($1, $2, $3, $4, $5, '', '', '{}'::jsonb, '')",
    )
    .bind(session_id)
    .bind(instance_id)
    .bind(user_id)
    .bind(org_id)
    .bind(token_hash(raw_token))
    .execute(db.pool())
    .await?;
    Ok(())
}

async fn table_exists(db: &Db, table: &str) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (
            SELECT 1
            FROM information_schema.tables
            WHERE table_schema = 'public' AND table_name = $1
        )",
    )
    .bind(table)
    .fetch_one(db.pool())
    .await?;
    Ok(exists)
}

async fn table_persistence(db: &Db, table: &str) -> anyhow::Result<Option<String>> {
    let persistence = sqlx::query_scalar::<_, String>(
        "SELECT relpersistence::text FROM pg_class WHERE relname = $1 LIMIT 1",
    )
    .bind(table)
    .fetch_optional(db.pool())
    .await?;
    Ok(persistence)
}

async fn session_exists(db: &Db, instance_id: &str, session_id: &str) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM sessions WHERE instance_id = $1 AND id = $2)",
    )
    .bind(instance_id)
    .bind(session_id)
    .fetch_one(db.pool())
    .await?;
    Ok(exists)
}

async fn session_count_for_id(db: &Db, instance_id: &str, session_id: &str) -> anyhow::Result<i64> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND id = $2",
    )
    .bind(instance_id)
    .bind(session_id)
    .fetch_one(db.pool())
    .await?;
    Ok(count)
}

async fn sink_inbox_count(db: &Db) -> anyhow::Result<i64> {
    let count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM storage_sink_inbox")
        .fetch_one(db.pool())
        .await?;
    Ok(count)
}

fn replace_database_name(url: &str, db_name: &str) -> String {
    let (without_query, query) = match url.split_once('?') {
        Some((without_query, query)) => (without_query, Some(query)),
        None => (url, None),
    };
    let slash = without_query
        .rfind('/')
        .expect("postgres URL should include a database name");
    let mut rewritten = format!("{}/{}", &without_query[..slash], db_name);
    if let Some(query) = query {
        rewritten.push('?');
        rewritten.push_str(query);
    }
    rewritten
}

#[derive(Clone)]
struct PersistedSessionRecordForTest {
    id: String,
    user_id: String,
    org_id: String,
    token_hash: String,
    user_agent: String,
    ip_address: String,
    metadata: serde_json::Value,
    created_at: String,
    expires_at: Option<String>,
}

impl PersistedSessionRecordForTest {
    fn from_session(session: &SessionRecord) -> Self {
        Self {
            id: session.id.clone(),
            user_id: session.user_id.clone(),
            org_id: session.org_id.clone(),
            token_hash: session.token_hash.clone(),
            user_agent: session.user_agent.clone(),
            ip_address: session.ip_address.clone(),
            metadata: session.metadata.clone(),
            created_at: session.created_at.clone(),
            expires_at: session.expires_at.clone(),
        }
    }
}

impl From<PersistedSessionRecordForTest> for zitadel_storage::PersistedSessionRecord {
    fn from(value: PersistedSessionRecordForTest) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            org_id: value.org_id,
            token_hash: value.token_hash,
            user_agent: value.user_agent,
            ip_address: value.ip_address,
            metadata: value.metadata,
            created_at: value.created_at,
            expires_at: value.expires_at,
        }
    }
}
