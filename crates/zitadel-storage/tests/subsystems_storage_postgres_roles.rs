use sqlx::{Executor, postgres::PgPoolOptions};
use uuid::Uuid;
use zitadel_config::{
    AnalyticsStorageConfig, PrimaryStorageConfig, ReplicaReadConfig, StorageConfig,
    TransientStorageConfig,
};
use zitadel_crypto::token_hash;
use zitadel_db::{Db, migrate};
use zitadel_storage::{ReadConsistency, StorageRuntime, prepare_auxiliary_databases};

#[tokio::test]
async fn auxiliary_preparation_only_migrates_transient_and_analytics_databases()
-> anyhow::Result<()> {
    let Some(env) = PostgresTestEnv::new().await? else {
        return Ok(());
    };

    let primary_db = Db::open(&env.primary_url).await?;
    migrate::migrate(&primary_db).await?;

    prepare_auxiliary_databases(&env.storage_config(), &primary_db).await?;

    let transient_db = Db::open(&env.transient_url).await?;
    let analytics_db = Db::open(&env.analytics_url).await?;
    let replica_db = Db::open(&env.replica_url).await?;

    assert!(table_exists(&transient_db, "sessions").await?);
    assert!(table_exists(&transient_db, "oidc_auth_requests").await?);
    assert!(table_exists(&analytics_db, "events").await?);
    assert!(table_exists(&analytics_db, "users").await?);
    assert!(!table_exists(&replica_db, "sessions").await?);
    assert!(!table_exists(&replica_db, "users").await?);

    Ok(())
}

#[tokio::test]
async fn storage_runtime_uses_explicit_replica_reads_and_authoritative_transient_db()
-> anyhow::Result<()> {
    let Some(env) = PostgresTestEnv::new().await? else {
        return Ok(());
    };

    let primary_db = Db::open(&env.primary_url).await?;
    let replica_db = Db::open(&env.replica_url).await?;
    migrate::migrate(&primary_db).await?;
    migrate::migrate(&replica_db).await?;

    let config = env.storage_config();
    prepare_auxiliary_databases(&config, &primary_db).await?;

    seed_user(
        &primary_db,
        "instance-a",
        "org-a",
        "user-primary",
        "authoritative@example.com",
    )
    .await?;
    seed_user(
        &replica_db,
        "instance-a",
        "org-a",
        "user-replica",
        "reader@example.com",
    )
    .await?;

    seed_user(
        &primary_db,
        "instance-a",
        "org-a",
        "user-session",
        "session@example.com",
    )
    .await?;
    seed_session(
        &primary_db,
        "instance-a",
        "session-primary-only",
        "user-session",
        "org-a",
        "primary-only-token",
    )
    .await?;

    let runtime = StorageRuntime::from_config(&config, primary_db.clone(), 86_400).await?;

    assert!(
        runtime
            .primary
            .find_active_user_by_identifier("instance-a", "reader@example.com")
            .await?
            .is_none(),
        "strong reads should stay on the primary DB",
    );

    let replica_user = runtime
        .primary
        .find_active_user_by_identifier_with_consistency(
            "instance-a",
            "reader@example.com",
            ReadConsistency::StaleOk,
        )
        .await?;
    assert_eq!(replica_user.unwrap().user_id, "user-replica");

    let created = runtime
        .transient
        .create_session(
            "instance-a",
            "user-session",
            "org-a",
            "ua",
            "127.0.0.1",
            "fp",
        )
        .await?;

    let transient_db = Db::open(&env.transient_url).await?;
    assert!(session_exists(&transient_db, "instance-a", &created.session_id).await?);
    assert!(!session_exists(&primary_db, "instance-a", &created.session_id).await?);

    let found_local = runtime
        .transient
        .find_session_by_token("instance-a", &created.token)
        .await?;
    assert_eq!(found_local.unwrap().id, created.session_id);

    let primary_only = runtime
        .transient
        .find_session_by_token("instance-a", "primary-only-token")
        .await?;
    assert!(
        primary_only.is_none(),
        "transient session reads should not fall back to the primary DB",
    );

    Ok(())
}

#[tokio::test]
async fn stale_ok_replica_reads_fall_back_to_primary_on_failure() -> anyhow::Result<()> {
    let Some(env) = PostgresTestEnv::new().await? else {
        return Ok(());
    };

    let primary_db = Db::open(&env.primary_url).await?;
    migrate::migrate(&primary_db).await?;

    seed_user(
        &primary_db,
        "instance-a",
        "org-a",
        "user-primary",
        "authoritative@example.com",
    )
    .await?;

    let config = env.storage_config();
    prepare_auxiliary_databases(&config, &primary_db).await?;

    let runtime = StorageRuntime::from_config(&config, primary_db.clone(), 86_400).await?;
    let user = runtime
        .primary
        .find_active_user_by_identifier_with_consistency(
            "instance-a",
            "authoritative@example.com",
            ReadConsistency::StaleOk,
        )
        .await?;

    assert_eq!(user.unwrap().user_id, "user-primary");
    Ok(())
}

#[derive(Clone)]
struct PostgresTestEnv {
    primary_url: String,
    replica_url: String,
    transient_url: String,
    analytics_url: String,
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
        let primary = format!("storage_primary_{suffix}");
        let replica = format!("storage_replica_{suffix}");
        let transient = format!("storage_transient_{suffix}");
        let analytics = format!("storage_analytics_{suffix}");

        for db_name in [&primary, &replica, &transient, &analytics] {
            admin
                .execute(sqlx::query(&format!("CREATE DATABASE \"{db_name}\"")))
                .await?;
        }

        Ok(Some(Self {
            primary_url: replace_database_name(&base_url, &primary),
            replica_url: replace_database_name(&base_url, &replica),
            transient_url: replace_database_name(&base_url, &transient),
            analytics_url: replace_database_name(&base_url, &analytics),
        }))
    }

    fn storage_config(&self) -> StorageConfig {
        StorageConfig {
            primary: PrimaryStorageConfig {
                url: self.primary_url.clone(),
                backend: "postgres".into(),
                replica: ReplicaReadConfig {
                    enabled: true,
                    url: self.replica_url.clone(),
                    mode: "explicit".into(),
                },
                ..Default::default()
            },
            transient: TransientStorageConfig {
                backend: "postgres".into(),
                url: self.transient_url.clone(),
                ..Default::default()
            },
            analytics: AnalyticsStorageConfig {
                backend: "postgres".into(),
                url: self.analytics_url.clone(),
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
        "INSERT INTO instances (instance_id, kind) VALUES ($1, 'root') ON CONFLICT DO NOTHING",
    )
    .bind(instance_id)
    .execute(db.pool())
    .await?;
    sqlx::query(
        "INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(org_id)
    .bind(instance_id)
    .bind(org_id)
    .execute(db.pool())
    .await?;
    sqlx::query(
        "INSERT INTO users (id, instance_id, org_id, identifier, user_type, state) \
         VALUES ($1, $2, $3, $4, 'human', 'active') ON CONFLICT DO NOTHING",
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
