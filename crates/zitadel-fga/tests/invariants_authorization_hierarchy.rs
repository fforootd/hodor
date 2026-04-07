use uuid::Uuid;
use zitadel_db::{
    CreateManagedInstanceInput, DEFAULT_INSTANCE_ID, DEFAULT_ORG_ID, Db, add_membership, bootstrap,
    create_managed_instance, create_user, migrate,
};
use zitadel_fga::{
    ChangeRepository, CheckRequest, Evaluator, FgaService, ModelRepository, PLATFORM_STORE_ID,
    ReadRequest, StoreResolver, TupleFilter, TupleKey, TupleKeySet, TupleRepository, WriteRequest,
    core_authorization_model,
};

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

async fn assert_tuple_lifecycle_and_scope_isolation(db: Db) -> anyhow::Result<()> {
    migrate::migrate(&db).await?;
    bootstrap::bootstrap(&db, None).await?;

    let service = FgaService::new(db.clone());
    service.initialize_platform_store().await?;
    let root_store = service.initialize_instance(DEFAULT_INSTANCE_ID).await?;

    let child_id = format!("child-{}", Uuid::new_v4());
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

    let child_store = service.initialize_instance(&child_id).await?;
    let new_model = service
        .write_model(&child_id, &child_store.id, core_authorization_model())
        .await?;
    let models = service.read_models(&child_id, &child_store.id).await?;
    assert!(
        models
            .authorization_models
            .iter()
            .any(|model| { model.authorization_model_id == new_model.authorization_model_id })
    );

    let root_tuple = TupleKey {
        user: "user:anne".into(),
        relation: "member".into(),
        object: "group:engineering".into(),
        condition: None,
    };
    service
        .write_tuples(
            DEFAULT_INSTANCE_ID,
            &root_store.id,
            WriteRequest {
                writes: TupleKeySet {
                    tuple_keys: vec![root_tuple.clone()],
                },
                deletes: TupleKeySet { tuple_keys: vec![] },
                authorization_model_id: None,
            },
        )
        .await?;

    let child_tuple = TupleKey {
        user: "user:bob".into(),
        relation: "member".into(),
        object: "group:engineering".into(),
        condition: None,
    };
    service
        .write_tuples(
            &child_id,
            &child_store.id,
            WriteRequest {
                writes: TupleKeySet {
                    tuple_keys: vec![child_tuple.clone()],
                },
                deletes: TupleKeySet { tuple_keys: vec![] },
                authorization_model_id: Some(new_model.authorization_model_id.clone()),
            },
        )
        .await?;

    let root_read = service
        .read_tuples(
            DEFAULT_INSTANCE_ID,
            &root_store.id,
            ReadRequest {
                tuple_key: Some(TupleFilter {
                    user: Some(root_tuple.user.clone()),
                    relation: Some(root_tuple.relation.clone()),
                    object: Some(root_tuple.object.clone()),
                }),
                page_size: Some(10),
                continuation_token: None,
            },
        )
        .await?;
    assert_eq!(root_read.tuples.len(), 1);

    let child_read = service
        .read_tuples(
            &child_id,
            &child_store.id,
            ReadRequest {
                tuple_key: Some(TupleFilter {
                    user: Some(child_tuple.user.clone()),
                    relation: Some(child_tuple.relation.clone()),
                    object: Some(child_tuple.object.clone()),
                }),
                page_size: Some(10),
                continuation_token: None,
            },
        )
        .await?;
    assert_eq!(child_read.tuples.len(), 1);

    let child_cannot_see_root = service
        .read_tuples(
            &child_id,
            &child_store.id,
            ReadRequest {
                tuple_key: Some(TupleFilter {
                    user: Some(root_tuple.user.clone()),
                    relation: Some(root_tuple.relation.clone()),
                    object: Some(root_tuple.object.clone()),
                }),
                page_size: Some(10),
                continuation_token: None,
            },
        )
        .await?;
    assert!(
        child_cannot_see_root.tuples.is_empty(),
        "child store should not leak tuples from the root store",
    );

    let changes = service
        .read_changes(&child_id, &child_store.id, Some("group"), 10, None)
        .await?;
    assert!(
        changes
            .changes
            .iter()
            .any(|change| change.operation == "WRITE" && change.tuple_key.user == child_tuple.user),
        "child store should record tuple writes in its own change log",
    );

    service
        .write_tuples(
            &child_id,
            &child_store.id,
            WriteRequest {
                writes: TupleKeySet { tuple_keys: vec![] },
                deletes: TupleKeySet {
                    tuple_keys: vec![child_tuple.clone()],
                },
                authorization_model_id: Some(new_model.authorization_model_id),
            },
        )
        .await?;

    let child_after_delete = service
        .read_tuples(
            &child_id,
            &child_store.id,
            ReadRequest {
                tuple_key: Some(TupleFilter {
                    user: Some(child_tuple.user),
                    relation: Some(child_tuple.relation),
                    object: Some(child_tuple.object),
                }),
                page_size: Some(10),
                continuation_token: None,
            },
        )
        .await?;
    assert!(child_after_delete.tuples.is_empty());

    Ok(())
}

#[tokio::test]
async fn root_org_owner_can_administer_child_instance_on_sqlite() -> anyhow::Result<()> {
    let db = Db::open("").await?;
    assert_root_org_owner_inherits_child_instance_admin(db).await
}

#[tokio::test]
async fn root_org_owner_can_administer_child_instance_on_postgres_when_configured()
-> anyhow::Result<()> {
    let Some(url) = std::env::var("ZITADEL_TEST_POSTGRES_URL").ok() else {
        eprintln!("skipping Postgres hierarchy test: ZITADEL_TEST_POSTGRES_URL is not set");
        return Ok(());
    };
    let db = Db::open(&url).await?;
    assert_root_org_owner_inherits_child_instance_admin(db).await
}

#[tokio::test]
async fn root_org_owner_can_administer_child_instance_on_spanner_when_configured()
-> anyhow::Result<()> {
    let Some(db) = zitadel_db::test_support::spanner_db_from_env("fga-root-hierarchy").await?
    else {
        return Ok(());
    };
    assert_root_org_owner_inherits_child_instance_admin(db).await
}

#[tokio::test]
async fn tuple_lifecycle_and_scope_isolation_hold_on_sqlite() -> anyhow::Result<()> {
    let db = Db::open("").await?;
    assert_tuple_lifecycle_and_scope_isolation(db).await
}

#[tokio::test]
async fn tuple_lifecycle_and_scope_isolation_hold_on_spanner_when_configured() -> anyhow::Result<()>
{
    let Some(db) = zitadel_db::test_support::spanner_db_from_env("fga-tuple-lifecycle").await?
    else {
        return Ok(());
    };
    assert_tuple_lifecycle_and_scope_isolation(db).await
}
