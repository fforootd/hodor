use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
};

use anyhow::Context;
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, header::HOST},
};
use serde_json::json;
use uuid::Uuid;
use zitadel_config::Config;
use zitadel_db::DEFAULT_INSTANCE_ID;
use zitadel_server::{AppState, build_router, routing::InstanceResolver};
use zitadel_testkit::{AuthActor, SessionFixture, TestApp, TestContext, UserFixture};

const ROOT_HOST: &str = "root.example.com";

async fn build_test_app() -> anyhow::Result<TestApp> {
    let mut config = Config::default();
    config.cloud.enabled = true;
    config.storage.stateful.url = "sqlite://:memory:".into();
    let ctx = TestContext::with_config(config).await?;
    let scoped = ctx.db.scoped_default();
    sqlx::query(
        "INSERT INTO domains (domain, instance_id, org_id, is_primary, state, verified) \
         VALUES ($1, $2, NULL, 1, 'active', 1)",
    )
    .bind(ROOT_HOST)
    .bind(DEFAULT_INSTANCE_ID)
    .execute(scoped.pool())
    .await
    .context("insert root domain")?;
    let app_state = Arc::new(AppState {
        config: ctx.config.clone(),
        db: ctx.db.db.clone(),
        secret_box: Arc::new(zitadel_crypto::SecretBox::new("", &HashMap::new())?),
        ready: AtomicBool::new(true),
        instance_resolver: Arc::new(InstanceResolver::new(&ctx.config, ctx.db.db.clone())),
    });
    let router: Router = build_router(
        app_state,
        ctx.api_state.clone(),
        ctx.oidc_state.clone(),
        ctx.login_state.clone(),
    );

    Ok(TestApp::new(ctx, router))
}

async fn create_root_user_in_org(
    app: &TestApp,
    org_id: &str,
    org_name: &str,
    identifier: &str,
) -> anyhow::Result<UserFixture> {
    let scoped = app.ctx.db.scoped_default();
    sqlx::query("INSERT INTO orgs (instance_id, id, name, state) VALUES ($1, $2, $3, 'active')")
        .bind(scoped.instance_id())
        .bind(org_id)
        .bind(org_name)
        .execute(scoped.pool())
        .await
        .context("insert root org")?;

    let user_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, metadata) \
         VALUES ($1, $2, $3, $4, $5, 'human', 'active', '{}')",
    )
    .bind(&user_id)
    .bind(scoped.instance_id())
    .bind(org_id)
    .bind(identifier)
    .bind(identifier)
    .execute(scoped.pool())
    .await
    .context("insert root user")?;

    Ok(UserFixture {
        user_id,
        org_id: org_id.to_string(),
        identifier: identifier.to_string(),
    })
}

async fn create_session_for_instance(
    app: &TestApp,
    instance_id: &str,
    user: &UserFixture,
) -> anyhow::Result<SessionFixture> {
    let session = app
        .ctx
        .login_state
        .transient
        .create_session(
            instance_id,
            &user.user_id,
            &user.org_id,
            "root-management-contract",
            "127.0.0.1",
            "",
        )
        .await
        .context("create session")?;
    Ok(SessionFixture {
        session_id: session.session_id,
        token: session.token,
    })
}

async fn insert_child_instance(
    app: &TestApp,
    instance_id: &str,
    owner_org_id: &str,
    domain: &str,
) -> anyhow::Result<()> {
    let scoped = app.ctx.db.scoped_default();
    sqlx::query(
        "INSERT INTO instances (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, feature_overrides) \
         VALUES ($1, $2, $3, 'managed', 'active', 'global', '{}')",
    )
    .bind(instance_id)
    .bind(DEFAULT_INSTANCE_ID)
    .bind(owner_org_id)
    .execute(scoped.pool())
    .await
    .context("insert child instance")?;
    sqlx::query(
        "INSERT INTO domains (domain, instance_id, org_id, is_primary, state, verified) \
         VALUES ($1, $2, NULL, 1, 'active', 1)",
    )
    .bind(domain)
    .bind(instance_id)
    .execute(scoped.pool())
    .await
    .context("insert child domain")?;
    Ok(())
}

async fn create_child_user_session(
    app: &TestApp,
    instance_id: &str,
    org_id: &str,
    identifier: &str,
) -> anyhow::Result<SessionFixture> {
    let scoped = app.ctx.db.db.scoped(instance_id.to_string());
    sqlx::query("INSERT INTO orgs (instance_id, id, name, state) VALUES ($1, $2, $3, 'active')")
        .bind(instance_id)
        .bind(org_id)
        .bind(org_id)
        .execute(scoped.pool())
        .await
        .context("insert child org")?;
    let user_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, metadata) \
         VALUES ($1, $2, $3, $4, $5, 'human', 'active', '{}')",
    )
    .bind(&user_id)
    .bind(instance_id)
    .bind(org_id)
    .bind(identifier)
    .bind(identifier)
    .execute(scoped.pool())
    .await
    .context("insert child user")?;

    create_session_for_instance(
        app,
        instance_id,
        &UserFixture {
            user_id,
            org_id: org_id.to_string(),
            identifier: identifier.to_string(),
        },
    )
    .await
}

fn host_headers(host: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(HOST, host.parse().unwrap());
    headers
}

async fn get_on_host(
    app: &TestApp,
    path: &str,
    actor: AuthActor,
    host: &str,
) -> anyhow::Result<zitadel_testkit::TestResponse> {
    app.request(Method::GET, path, actor, host_headers(host), Body::empty())
        .await
}

async fn post_json_on_host(
    app: &TestApp,
    path: &str,
    actor: AuthActor,
    host: &str,
    body: &serde_json::Value,
) -> anyhow::Result<zitadel_testkit::TestResponse> {
    let mut headers = host_headers(host);
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    app.request(
        Method::POST,
        path,
        actor,
        headers,
        Body::from(body.to_string()),
    )
    .await
}

#[tokio::test]
async fn root_users_are_owner_scoped_and_operators_are_unscoped() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let owner_one = app
        .ctx
        .create_user("owner-one@example.com", "password123")
        .await?;
    let owner_one_session = app.ctx.create_session(&owner_one).await?;

    let owner_two =
        create_root_user_in_org(&app, "org-2", "Org Two", "owner-two@example.com").await?;
    let owner_two_session =
        create_session_for_instance(&app, DEFAULT_INSTANCE_ID, &owner_two).await?;

    insert_child_instance(&app, "inst-one", &owner_one.org_id, "one.example.com").await?;
    insert_child_instance(&app, "inst-two", &owner_two.org_id, "two.example.com").await?;

    let owner_one_list = get_on_host(
        &app,
        "/v1/instances",
        owner_one_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(owner_one_list.status, axum::http::StatusCode::OK);
    let owner_one_json = owner_one_list.json_value();
    assert_eq!(owner_one_json["items"].as_array().unwrap().len(), 1);
    assert_eq!(owner_one_json["items"][0]["instance_id"], "inst-one");

    let owner_two_list = get_on_host(
        &app,
        "/v1/instances",
        owner_two_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(owner_two_list.status, axum::http::StatusCode::OK);
    let owner_two_json = owner_two_list.json_value();
    assert_eq!(owner_two_json["items"].as_array().unwrap().len(), 1);
    assert_eq!(owner_two_json["items"][0]["instance_id"], "inst-two");

    let owner_admin_list = app
        .request(
            Method::GET,
            "/v1/admin/instances",
            owner_one_session.bearer_actor(),
            host_headers(ROOT_HOST),
            Body::empty(),
        )
        .await?;
    assert_eq!(owner_admin_list.status, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(
        owner_admin_list.json_value(),
        json!({"error": "operator admin required", "code": 403})
    );

    let operator = app.ctx.admin_user().await?;
    let operator_session = app.ctx.create_session(&operator).await?;

    let operator_list = get_on_host(
        &app,
        "/v1/instances",
        operator_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(operator_list.status, axum::http::StatusCode::OK);
    assert_eq!(
        operator_list.json_value()["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    let operator_admin_list = app
        .request(
            Method::GET,
            "/v1/admin/instances",
            operator_session.bearer_actor(),
            host_headers(ROOT_HOST),
            Body::empty(),
        )
        .await?;
    assert_eq!(operator_admin_list.status, axum::http::StatusCode::OK);
    assert_eq!(
        operator_admin_list.json_value()["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );

    Ok(())
}

#[tokio::test]
async fn root_bootstrap_exposes_capabilities_and_child_context_rejects_instance_management()
-> anyhow::Result<()> {
    let app = build_test_app().await?;
    let owner = app
        .ctx
        .create_user("owner@example.com", "password123")
        .await?;
    let owner_session = app.ctx.create_session(&owner).await?;

    let create = post_json_on_host(
        &app,
        "/v1/instances",
        owner_session.bearer_actor(),
        ROOT_HOST,
        &json!({
            "instance_id": "child-a",
            "domain": "child-a.example.com",
        }),
    )
    .await?;
    assert_eq!(create.status, axum::http::StatusCode::CREATED);
    assert_eq!(create.json_value()["kind"], "managed");

    let root_bootstrap = get_on_host(
        &app,
        "/v1/console/bootstrap",
        owner_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(root_bootstrap.status, axum::http::StatusCode::OK);
    let root_bootstrap_json = root_bootstrap.json_value();
    assert_eq!(root_bootstrap_json["instance"]["is_root"], true);
    assert_eq!(root_bootstrap_json["instance"]["kind"], "root");
    assert_eq!(
        root_bootstrap_json["capabilities"]["instance_management"],
        true
    );
    assert_eq!(root_bootstrap_json["capabilities"]["billing"], true);
    assert_eq!(root_bootstrap_json["capabilities"]["operator_admin"], false);

    let operator = app.ctx.admin_user().await?;
    let operator_session = app.ctx.create_session(&operator).await?;
    let operator_bootstrap = get_on_host(
        &app,
        "/v1/console/bootstrap",
        operator_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(operator_bootstrap.status, axum::http::StatusCode::OK);
    assert_eq!(
        operator_bootstrap.json_value()["capabilities"]["operator_admin"],
        true
    );

    let child_session =
        create_child_user_session(&app, "child-a", "child-org", "child-user@example.com").await?;

    let child_bootstrap = app
        .request(
            Method::GET,
            "/v1/console/bootstrap",
            child_session.bearer_actor(),
            host_headers("child-a.example.com"),
            Body::empty(),
        )
        .await?;
    assert_eq!(child_bootstrap.status, axum::http::StatusCode::OK);
    let child_bootstrap_json = child_bootstrap.json_value();
    assert_eq!(child_bootstrap_json["instance"]["is_root"], false);
    assert_eq!(
        child_bootstrap_json["capabilities"]["instance_management"],
        false
    );
    assert_eq!(child_bootstrap_json["capabilities"]["billing"], false);
    assert_eq!(
        child_bootstrap_json["capabilities"]["operator_admin"],
        false
    );

    let child_instances = app
        .request(
            Method::GET,
            "/v1/instances",
            child_session.bearer_actor(),
            host_headers("child-a.example.com"),
            Body::empty(),
        )
        .await?;
    assert_eq!(child_instances.status, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(
        child_instances.json_value(),
        json!({"error": "instance management is only available from the root instance", "code": 403})
    );

    Ok(())
}

#[tokio::test]
async fn root_instance_access_uses_fga_not_only_session_org_scope() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let root_user =
        create_root_user_in_org(&app, "org-a", "Org A", "fga-scope@example.com").await?;
    let root_session = create_session_for_instance(&app, DEFAULT_INSTANCE_ID, &root_user).await?;

    insert_child_instance(&app, "inst-owned", "org-a", "owned.example.com").await?;

    let initial = get_on_host(
        &app,
        "/v1/instances",
        root_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(initial.status, axum::http::StatusCode::OK);
    assert_eq!(initial.json_value()["items"].as_array().unwrap().len(), 1);

    let scoped = app.ctx.db.scoped_default();
    sqlx::query("INSERT INTO orgs (instance_id, id, name, state) VALUES ($1, $2, $3, 'active')")
        .bind(scoped.instance_id())
        .bind("org-b")
        .bind("Org B")
        .execute(scoped.pool())
        .await
        .context("insert org-b")?;
    sqlx::query("UPDATE users SET org_id = $1 WHERE instance_id = $2 AND id = $3")
        .bind("org-b")
        .bind(scoped.instance_id())
        .bind(&root_user.user_id)
        .execute(scoped.pool())
        .await
        .context("move user to org-b")?;

    let after_move = get_on_host(
        &app,
        "/v1/instances",
        root_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(after_move.status, axum::http::StatusCode::OK);
    assert_eq!(after_move.json_value()["items"], json!([]));

    Ok(())
}
