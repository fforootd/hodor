use uuid::Uuid;
use zitadel_config::StatefulStorageConfig;
use zitadel_db::{
    CreateManagedInstanceInput, DEFAULT_INSTANCE_ID, DEFAULT_ORG_ID, Db, add_membership, bootstrap,
    create_managed_instance, create_user, migrate,
};
use zitadel_fga::{CheckRequest, Evaluator, FgaService, PLATFORM_STORE_ID, StoreResolver, TupleKey};

async fn assert_root_org_owner_inherits_child_instance_admin(db: Db) -> anyhow::Result<()> {
    migrate::migrate(&db).await?;
    bootstrap::bootstrap(&db, None).await?;

    let service = FgaService::new(db.clone());
    service.initialize_instance(DEFAULT_INSTANCE_ID).await?;

    let user_id = format!("root-user-{}", Uuid::new_v4());
    let child_id = format!("child-{}", Uuid::new_v4());
    create_user(
        &db,
        DEFAULT_INSTANCE_ID,
        &user_id,
        DEFAULT_ORG_ID,
        &format!("{user_id}@example.com"),
        "Root User",
        "",
        "{}",
    )
    .await?;
    add_membership(
        &db,
        DEFAULT_INSTANCE_ID,
        "org",
        DEFAULT_ORG_ID,
        &user_id,
        "owner",
    )
    .await?;
    create_managed_instance(
        &db,
        &CreateManagedInstanceInput {
            instance_id: child_id.clone(),
            root_instance_id: DEFAULT_INSTANCE_ID.into(),
            owner_org_id: DEFAULT_ORG_ID.into(),
            primary_domain: format!("{child_id}.example.com"),
            kind: "managed".into(),
            placement_mode: "global".into(),
            region_key: None,
        },
    )
    .await?;

    service
        .reconcile_root_hierarchy(DEFAULT_INSTANCE_ID)
        .await?;
    let root_store = service.discover_platform_store().await?;
    let allowed = service
        .check(
            PLATFORM_STORE_ID,
            &root_store.id,
            CheckRequest {
                tuple_key: TupleKey {
                    user: format!("user:{user_id}"),
                    relation: "admin".into(),
                    object: format!("instance:{child_id}"),
                    condition: None,
                },
                authorization_model_id: None,
                contextual_tuples: None,
                context: None,
            },
        )
        .await?;
    assert!(allowed.allowed);
    Ok(())
}

#[tokio::test]
async fn root_org_owner_can_administer_child_instance_on_sqlite() -> anyhow::Result<()> {
    let db = Db::open("").await?;
    assert_root_org_owner_inherits_child_instance_admin(db).await
}

#[tokio::test]
async fn root_org_owner_can_administer_child_instance_on_postgres_when_configured() -> anyhow::Result<()> {
    let Some(url) = std::env::var("ZITADEL_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping Postgres hierarchy test: ZITADEL_TEST_POSTGRES_URL is not set");
        return Ok(());
    };
    let db = Db::open(&url).await?;
    assert_root_org_owner_inherits_child_instance_admin(db).await
}

#[tokio::test]
async fn root_org_owner_can_administer_child_instance_on_spanner_when_configured() -> anyhow::Result<()> {
    let Some(database) = std::env::var("ZITADEL_TEST_SPANNER_DATABASE").ok() else {
        eprintln!("skipping Spanner hierarchy test: ZITADEL_TEST_SPANNER_DATABASE is not set");
        return Ok(());
    };
    let Some(emulator_host) = std::env::var("ZITADEL_TEST_SPANNER_EMULATOR_HOST").ok() else {
        eprintln!("skipping Spanner hierarchy test: ZITADEL_TEST_SPANNER_EMULATOR_HOST is not set");
        return Ok(());
    };

    let config = StatefulStorageConfig {
        backend: "spanner".into(),
        database,
        emulator_host,
        ..Default::default()
    };
    let db = Db::open_with_config("", &config).await?;
    assert_root_org_owner_inherits_child_instance_admin(db).await
}
