use std::sync::Arc;

use tokio::{sync::Barrier, task::JoinSet};
use uuid::Uuid;
use zitadel_config::{StatefulStorageConfig, StorageConfig};
use zitadel_crypto::token_hash;
use zitadel_db::{
    DEFAULT_INSTANCE_ID, Db, create_oidc_auth_request_record, create_org, create_pat, create_user,
    migrate,
};
use zitadel_storage::{AnalyticsQuery, ProviderAuthState, StorageRuntime};

async fn spanner_runtime() -> anyhow::Result<Option<StorageRuntime>> {
    let Some(database) = std::env::var("ZITADEL_TEST_SPANNER_DATABASE").ok() else {
        eprintln!("skipping Spanner transient test: ZITADEL_TEST_SPANNER_DATABASE is not set");
        return Ok(None);
    };
    let Some(emulator_host) = std::env::var("ZITADEL_TEST_SPANNER_EMULATOR_HOST").ok() else {
        eprintln!("skipping Spanner transient test: ZITADEL_TEST_SPANNER_EMULATOR_HOST is not set");
        return Ok(None);
    };

    let config = StorageConfig {
        stateful: StatefulStorageConfig {
            backend: "spanner".into(),
            database,
            emulator_host,
            ..Default::default()
        },
        ..Default::default()
    };
    let db = Db::open_with_config("", &config.stateful).await?;
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
    let db = runtime.stateful.db();
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

async fn disable_user(runtime: &StorageRuntime, user_id: &str) -> anyhow::Result<()> {
    let spanner = runtime
        .stateful
        .db()
        .spanner()
        .expect("runtime uses native spanner backend");
    let mut stmt = google_cloud_spanner::statement::Statement::new(
        "UPDATE users SET state = 'disabled' WHERE instance_id = @instance_id AND id = @id",
    );
    stmt.add_param("instance_id", &DEFAULT_INSTANCE_ID);
    stmt.add_param("id", &user_id);
    let _ = spanner
        .client()
        .read_write_transaction(|tx| {
            let stmt = stmt.clone();
            Box::pin(async move {
                tx.update(stmt).await?;
                Ok::<(), google_cloud_spanner::client::Error>(())
            })
        })
        .await?;
    Ok(())
}

#[tokio::test]
async fn provider_auth_state_is_consumed_once_on_spanner_emulator_when_configured()
-> anyhow::Result<()> {
    let Some(runtime) = spanner_runtime().await? else {
        return Ok(());
    };
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

    runtime
        .transient
        .create_provider_auth_state(DEFAULT_INSTANCE_ID, &state)
        .await?;

    let attempts = 8;
    let barrier = Arc::new(Barrier::new(attempts));
    let mut tasks = JoinSet::new();
    for _ in 0..attempts {
        let transient = runtime.transient.clone();
        let barrier = barrier.clone();
        tasks.spawn(async move {
            barrier.wait().await;
            transient
                .consume_provider_auth_state(DEFAULT_INSTANCE_ID, "state-1")
                .await
        });
    }

    let mut successful_consumes = 0;
    while let Some(result) = tasks.join_next().await {
        let state = result??;
        if state.is_some() {
            successful_consumes += 1;
        }
    }

    assert_eq!(successful_consumes, 1);
    Ok(())
}

#[tokio::test]
async fn completed_auth_request_cannot_be_reused_on_spanner_emulator_when_configured()
-> anyhow::Result<()> {
    let Some(runtime) = spanner_runtime().await? else {
        return Ok(());
    };

    let org_id = unique("org");
    let user_id = unique("user");
    seed_active_user(&runtime, &org_id, &user_id, &unique("alice")).await?;

    let auth_request_id = unique("authreq");
    create_oidc_auth_request_record(
        runtime.stateful.db(),
        DEFAULT_INSTANCE_ID,
        &auth_request_id,
        "client-1",
        "https://app.example/callback",
        "openid profile",
        "state-1",
        "nonce-1",
        "code",
        "",
        "",
        "[]",
        "",
        Some(300),
    )
    .await?;

    assert!(
        runtime
            .transient
            .load_auth_request_redirect(DEFAULT_INSTANCE_ID, &auth_request_id)
            .await?
            .is_some()
    );

    runtime
        .transient
        .complete_auth_request(
            DEFAULT_INSTANCE_ID,
            &auth_request_id,
            &user_id,
            None,
            "code-1",
            None,
        )
        .await?;

    assert!(
        runtime
            .transient
            .load_auth_request_redirect(DEFAULT_INSTANCE_ID, &auth_request_id)
            .await
            .is_err()
    );
    assert_eq!(
        runtime
            .transient
            .load_auth_request_prompts(DEFAULT_INSTANCE_ID, &auth_request_id)
            .await?,
        Default::default()
    );
    assert!(
        runtime
            .transient
            .complete_auth_request(
                DEFAULT_INSTANCE_ID,
                &auth_request_id,
                &user_id,
                None,
                "code-2",
                None,
            )
            .await
            .is_err()
    );

    Ok(())
}

#[tokio::test]
async fn disabled_users_cannot_authenticate_with_session_or_pat_on_spanner_emulator()
-> anyhow::Result<()> {
    let Some(runtime) = spanner_runtime().await? else {
        return Ok(());
    };

    let org_id = unique("org");
    let user_id = unique("user");
    let identifier = unique("alice");
    seed_active_user(&runtime, &org_id, &user_id, &identifier).await?;

    let session = runtime
        .transient
        .create_session(
            DEFAULT_INSTANCE_ID,
            &user_id,
            &org_id,
            "ua",
            "127.0.0.1",
            "fp-spanner",
        )
        .await?;

    let pat_token = unique("pat-token");
    create_pat(
        runtime.stateful.db(),
        DEFAULT_INSTANCE_ID,
        &unique("pat"),
        &user_id,
        "test-pat",
        &token_hash(&pat_token),
        "[]",
    )
    .await?;

    disable_user(&runtime, &user_id).await?;

    assert!(
        runtime
            .transient
            .find_session_by_token(DEFAULT_INSTANCE_ID, &session.token)
            .await?
            .is_none()
    );
    assert!(
        runtime
            .stateful
            .resolve_pat_token(DEFAULT_INSTANCE_ID, &pat_token)
            .await?
            .is_none()
    );

    Ok(())
}

#[tokio::test]
async fn spanner_analytics_accepts_dollar_placeholders_when_configured() -> anyhow::Result<()> {
    let Some(runtime) = spanner_runtime().await? else {
        return Ok(());
    };

    let org_id = unique("org");
    let user_id = unique("user");
    let identifier = unique("analytics-user");
    seed_active_user(&runtime, &org_id, &user_id, &identifier).await?;

    let result = runtime
        .analytics
        .query(&AnalyticsQuery {
            sql: "SELECT identifier FROM users WHERE identifier = $1".into(),
            params: vec![identifier.clone()],
            limit: Some(1),
        })
        .await?;

    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.row_count, 1);
    assert_eq!(result.columns, vec!["identifier".to_string()]);
    assert_eq!(
        result.rows,
        vec![vec![serde_json::Value::String(identifier)]]
    );

    Ok(())
}
