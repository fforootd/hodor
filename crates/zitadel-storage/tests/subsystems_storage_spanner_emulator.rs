use uuid::Uuid;
use zitadel_config::StorageConfig;
use zitadel_crypto::token_hash;
use zitadel_db::{DEFAULT_INSTANCE_ID, Db, create_org, create_pat, create_user, migrate};
use zitadel_storage::StorageRuntime;

async fn spanner_runtime() -> anyhow::Result<Option<StorageRuntime>> {
    let Some(stateful) =
        zitadel_db::test_support::spanner_stateful_config_from_env("storage-subsystems").await?
    else {
        return Ok(None);
    };

    let config = StorageConfig {
        primary: stateful,
        ..Default::default()
    };
    let db = Db::open_with_config("", &config.primary).await?;
    migrate::migrate(&db).await?;
    StorageRuntime::from_config(&config, db, 86_400)
        .await
        .map(Some)
}

fn unique(prefix: &str) -> String {
    format!("{prefix}-{}", Uuid::new_v4().simple())
}

async fn seed_active_user(
    runtime: &StorageRuntime,
    org_id: &str,
    user_id: &str,
    identifier: &str,
) -> anyhow::Result<()> {
    let db = runtime.primary.db();
    create_org(db, DEFAULT_INSTANCE_ID, org_id, org_id, "{}").await?;
    create_user(
        db,
        DEFAULT_INSTANCE_ID,
        user_id,
        org_id,
        identifier,
        "Display Name",
        "human_user",
        "{}",
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn spanner_runtime_supports_sessions_pats_and_schema_queries_when_configured()
-> anyhow::Result<()> {
    let Some(runtime) = spanner_runtime().await? else {
        return Ok(());
    };

    let org_id = unique("org");
    let user_id = unique("user");
    let identifier = unique("subsystem-user");
    seed_active_user(&runtime, &org_id, &user_id, &identifier).await?;

    let session = runtime
        .transient
        .create_session(
            DEFAULT_INSTANCE_ID,
            &user_id,
            &org_id,
            "storage-subsystems",
            "127.0.0.1",
            "fp-subsystems",
        )
        .await?;

    let pat_token = unique("pat-token");
    create_pat(
        runtime.primary.db(),
        DEFAULT_INSTANCE_ID,
        &unique("pat"),
        &user_id,
        "storage-subsystems",
        &token_hash(&pat_token),
        "[]",
    )
    .await?;

    assert!(
        runtime
            .transient
            .find_session_by_token(DEFAULT_INSTANCE_ID, &session.token)
            .await?
            .is_some(),
        "native Spanner transient storage should resolve newly-created sessions",
    );
    assert!(
        runtime
            .primary
            .resolve_pat_token(DEFAULT_INSTANCE_ID, &pat_token)
            .await?
            .is_some(),
        "native Spanner stateful storage should resolve PAT hashes",
    );

    let schema = runtime.analytics.schema().await?;
    assert!(
        schema.get("events").is_some(),
        "analytics schema should expose the events table on native Spanner",
    );
    assert!(
        schema.get("sessions").is_some(),
        "analytics schema should expose the sessions table on native Spanner",
    );
    assert!(
        schema.get("tokens").is_some(),
        "analytics schema should expose the tokens table on native Spanner",
    );

    Ok(())
}
