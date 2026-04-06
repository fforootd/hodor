//! Integration tests verifying CRUD operations are properly isolated
//! between root and child instances.
//!
//! Tests exercise the full HTTP stack (router -> middleware -> handler -> DB)
//! with cloud mode enabled, host-based routing, and path-scoped instance access.

use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
};

use anyhow::Context;
use axum::{
    Router,
    body::Body,
    http::{HeaderMap, Method, StatusCode, header::{CONTENT_TYPE, HOST}},
};
use serde_json::{Value, json};
use uuid::Uuid;
use zitadel_config::Config;
use zitadel_db::DEFAULT_INSTANCE_ID;
use zitadel_fga::StoreResolver;
use zitadel_server::{AppState, build_router, routing::InstanceResolver};
use zitadel_testkit::{AuthActor, PatFixture, SessionFixture, TestApp, TestContext, UserFixture};

// ─── Constants ──────────────────────────────────────────────

const ROOT_HOST: &str = "root.example.com";
const CHILD_HOST: &str = "child.example.com";
const CHILD_INSTANCE_ID: &str = "child-inst";
const CHILD_ORG_ID: &str = "child-org";

// ─── Shared helpers ─────────────────────────────────────────

async fn build_cloud_test_app() -> anyhow::Result<TestApp> {
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

async fn setup_child(
    app: &TestApp,
    instance_id: &str,
    domain: &str,
    org_id: &str,
) -> anyhow::Result<(SessionFixture, PatFixture)> {
    let scoped = app.ctx.db.scoped_default();

    sqlx::query(
        "INSERT INTO instances (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, feature_overrides) \
         VALUES ($1, $2, $3, 'managed', 'active', 'global', '{}')",
    )
    .bind(instance_id)
    .bind(DEFAULT_INSTANCE_ID)
    .bind(&app.ctx.db.default_org_id().await?)
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

    let fga: &zitadel_fga::FgaService = &app.ctx.api_state.fga;
    fga.initialize_instance(instance_id).await.context("init child fga")?;

    let child_scoped = app.ctx.db.db.scoped(instance_id.to_string());
    sqlx::query("INSERT INTO orgs (instance_id, id, name, state) VALUES ($1, $2, $3, 'active')")
        .bind(instance_id)
        .bind(org_id)
        .bind("Child Org")
        .execute(child_scoped.pool())
        .await
        .context("insert child org")?;

    let user_id = Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, metadata) \
         VALUES ($1, $2, $3, 'child-admin', 'Child Admin', 'human', 'active', '{}')",
    )
    .bind(&user_id)
    .bind(instance_id)
    .bind(org_id)
    .execute(child_scoped.pool())
    .await
    .context("insert child admin user")?;

    let user = UserFixture {
        user_id: user_id.clone(),
        org_id: org_id.to_string(),
        identifier: "child-admin".into(),
    };

    let session = app
        .ctx
        .login_state
        .transient
        .create_session(instance_id, &user.user_id, &user.org_id, "test", "127.0.0.1", "")
        .await
        .context("create child session")?;

    let pat_id = Uuid::new_v4().to_string();
    let token = format!("zit_pat_{}", zitadel_crypto::random_hex(24));
    let token_hash = zitadel_authn::session::hash_token(&token);
    let sql = format!(
        "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) VALUES ($1, $2, 'pat', $3, $4, $5, {})",
        child_scoped.json_bind(6),
    );
    sqlx::query(&sql)
        .bind(&pat_id)
        .bind(instance_id)
        .bind(&token_hash)
        .bind(&user_id)
        .bind("child-admin-pat")
        .bind("[\"admin\"]")
        .execute(child_scoped.pool())
        .await
        .context("insert child pat")?;

    Ok((
        SessionFixture {
            session_id: session.session_id,
            token: session.token,
        },
        PatFixture { pat_id, token },
    ))
}

async fn create_root_user_in_org(
    app: &TestApp,
    org_id: &str,
    identifier: &str,
) -> anyhow::Result<UserFixture> {
    let scoped = app.ctx.db.scoped_default();
    sqlx::query("INSERT INTO orgs (instance_id, id, name, state) VALUES ($1, $2, $3, 'active')")
        .bind(scoped.instance_id())
        .bind(org_id)
        .bind(org_id)
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

fn host_headers(host: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(HOST, host.parse().unwrap());
    headers
}

fn json_host_headers(host: &str) -> HeaderMap {
    let mut headers = host_headers(host);
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    headers
}

fn extract_ids(json: &Value) -> Vec<String> {
    json["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|item| item["id"].as_str().map(String::from))
        .collect()
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
    app.request(
        Method::POST,
        path,
        actor,
        json_host_headers(host),
        Body::from(body.to_string()),
    )
    .await
}

async fn patch_json_on_host(
    app: &TestApp,
    path: &str,
    actor: AuthActor,
    host: &str,
    body: &serde_json::Value,
) -> anyhow::Result<zitadel_testkit::TestResponse> {
    app.request(
        Method::PATCH,
        path,
        actor,
        json_host_headers(host),
        Body::from(body.to_string()),
    )
    .await
}

// ─── Users: isolation between root and child ────────────────

#[tokio::test]
async fn users_crud_isolated_between_root_and_child() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "root-admin").await?;
    let (_child_session, child_pat) =
        setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;

    // Create user in root instance.
    let root_created = post_json_on_host(
        &app,
        "/v1/users",
        admin_pat.actor(),
        ROOT_HOST,
        &json!({ "identifier": "root-user@example.com", "display_name": "Root User" }),
    )
    .await?;
    assert_eq!(root_created.status, StatusCode::CREATED);
    let root_user_id = root_created.json_value()["id"].as_str().unwrap().to_string();

    // Create user in child instance.
    let child_created = post_json_on_host(
        &app,
        "/v1/users",
        child_pat.actor(),
        CHILD_HOST,
        &json!({ "identifier": "child-user@example.com", "display_name": "Child User" }),
    )
    .await?;
    assert_eq!(child_created.status, StatusCode::CREATED);
    let child_user_id = child_created.json_value()["id"].as_str().unwrap().to_string();

    // Root list — child user must NOT appear.
    let root_list = get_on_host(&app, "/v1/users", admin_pat.actor(), ROOT_HOST).await?;
    assert_eq!(root_list.status, StatusCode::OK);
    let root_json = root_list.json_value();
    let root_ids = extract_ids(&root_json);
    assert!(root_ids.contains(&root_user_id), "root list should contain root user");
    assert!(!root_ids.contains(&child_user_id), "root list must NOT contain child user");

    // Child list — root user must NOT appear.
    let child_list = get_on_host(&app, "/v1/users", child_pat.actor(), CHILD_HOST).await?;
    assert_eq!(child_list.status, StatusCode::OK);
    let child_json = child_list.json_value();
    let child_ids = extract_ids(&child_json);
    assert!(child_ids.contains(&child_user_id), "child list should contain child user");
    assert!(!child_ids.contains(&root_user_id), "child list must NOT contain root user");

    // Cross-instance get — should 404.
    let cross = get_on_host(&app, &format!("/v1/users/{root_user_id}"), child_pat.actor(), CHILD_HOST).await?;
    assert_eq!(cross.status, StatusCode::NOT_FOUND);
    let cross_rev = get_on_host(&app, &format!("/v1/users/{child_user_id}"), admin_pat.actor(), ROOT_HOST).await?;
    assert_eq!(cross_rev.status, StatusCode::NOT_FOUND);

    Ok(())
}

// ─── Users: path-scoped CRUD from root ──────────────────────

#[tokio::test]
async fn users_crud_via_path_scoped_instance_access() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "root-admin").await?;
    setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;

    // Create user in child via path-scoped route.
    let created = post_json_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/users"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({ "identifier": "path-user@example.com", "display_name": "Path User" }),
    )
    .await?;
    assert_eq!(created.status, StatusCode::CREATED);
    let user_id = created.json_value()["id"].as_str().unwrap().to_string();

    // Path-scoped list — user visible.
    let child_list = get_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/users"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(child_list.status, StatusCode::OK);
    let child_json = child_list.json_value();
    assert!(extract_ids(&child_json).contains(&user_id));

    // Root list — user NOT visible.
    let root_list = get_on_host(&app, "/v1/users", admin_pat.actor(), ROOT_HOST).await?;
    let root_json = root_list.json_value();
    assert!(!extract_ids(&root_json).contains(&user_id));

    // Path-scoped get single user.
    let fetched = get_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/users/{user_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(fetched.status, StatusCode::OK);
    assert_eq!(fetched.json_value()["identifier"], "path-user@example.com");

    // Path-scoped update.
    let updated = patch_json_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/users/{user_id}"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({ "display_name": "Updated Path User" }),
    )
    .await?;
    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.json_value()["display_name"], "Updated Path User");

    Ok(())
}

#[tokio::test]
async fn path_scoped_child_routes_require_child_instance_access() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let outsider = create_root_user_in_org(&app, "org-outsider", "outsider@example.com").await?;
    let outsider_session = app.ctx.create_session(&outsider).await?;
    let (_child_session, _child_pat) =
        setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;

    let denied = get_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/users"),
        outsider_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(denied.status, StatusCode::NOT_FOUND);
    assert_eq!(
        denied.json_value(),
        json!({"error": "instance not found", "code": 404})
    );

    Ok(())
}

// ─── Orgs: isolation between instances ──────────────────────

#[tokio::test]
async fn orgs_crud_isolated_between_instances() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "root-admin").await?;
    let (_child_session, child_pat) =
        setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;

    let root_org = post_json_on_host(&app, "/v1/orgs", admin_pat.actor(), ROOT_HOST, &json!({ "name": "Root Org" })).await?;
    assert_eq!(root_org.status, StatusCode::CREATED);
    let root_org_id = root_org.json_value()["id"].as_str().unwrap().to_string();

    let child_org = post_json_on_host(&app, "/v1/orgs", child_pat.actor(), CHILD_HOST, &json!({ "name": "Child Org Two" })).await?;
    assert_eq!(child_org.status, StatusCode::CREATED);
    let child_org_id = child_org.json_value()["id"].as_str().unwrap().to_string();

    // Root list isolation.
    let rl = get_on_host(&app, "/v1/orgs", admin_pat.actor(), ROOT_HOST).await?;
    let rj = rl.json_value();
    assert!(extract_ids(&rj).contains(&root_org_id));
    assert!(!extract_ids(&rj).contains(&child_org_id));

    // Child list isolation.
    let cl = get_on_host(&app, "/v1/orgs", child_pat.actor(), CHILD_HOST).await?;
    let cj = cl.json_value();
    assert!(extract_ids(&cj).contains(&child_org_id));
    assert!(!extract_ids(&cj).contains(&root_org_id));

    // Cross-instance get — 404.
    let cross = get_on_host(&app, &format!("/v1/orgs/{root_org_id}"), child_pat.actor(), CHILD_HOST).await?;
    assert_eq!(cross.status, StatusCode::NOT_FOUND);

    // Path-scoped get from root.
    let ps = get_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/orgs/{child_org_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(ps.status, StatusCode::OK);
    assert_eq!(ps.json_value()["name"], "Child Org Two");

    Ok(())
}

// ─── Groups: isolation between instances ────────────────────

#[tokio::test]
async fn groups_crud_isolated_between_instances() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "root-admin").await?;
    let (_child_session, child_pat) =
        setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;

    let rg = post_json_on_host(&app, "/v1/groups", admin_pat.actor(), ROOT_HOST, &json!({ "name": "Root Engineers" })).await?;
    assert_eq!(rg.status, StatusCode::CREATED);
    let rg_id = rg.json_value()["id"].as_str().unwrap().to_string();

    let cg = post_json_on_host(&app, "/v1/groups", child_pat.actor(), CHILD_HOST, &json!({ "name": "Child Engineers" })).await?;
    assert_eq!(cg.status, StatusCode::CREATED);
    let cg_id = cg.json_value()["id"].as_str().unwrap().to_string();

    let rl = get_on_host(&app, "/v1/groups", admin_pat.actor(), ROOT_HOST).await?;
    let rj = rl.json_value();
    assert!(extract_ids(&rj).contains(&rg_id));
    assert!(!extract_ids(&rj).contains(&cg_id));

    let cl = get_on_host(&app, "/v1/groups", child_pat.actor(), CHILD_HOST).await?;
    let cj = cl.json_value();
    assert!(extract_ids(&cj).contains(&cg_id));
    assert!(!extract_ids(&cj).contains(&rg_id));

    // Path-scoped update from root into child group.
    let updated = patch_json_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/groups/{cg_id}"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({ "name": "Child Engineers Renamed" }),
    )
    .await?;
    assert_eq!(updated.status, StatusCode::OK);
    assert_eq!(updated.json_value()["name"], "Child Engineers Renamed");

    Ok(())
}

// ─── Projects and Apps: isolation between instances ─────────

#[tokio::test]
async fn projects_and_apps_crud_isolated_between_instances() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "root-admin").await?;
    let (_child_session, child_pat) =
        setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;

    // Projects.
    let rp = post_json_on_host(&app, "/v1/projects", admin_pat.actor(), ROOT_HOST, &json!({ "name": "Root Project" })).await?;
    assert_eq!(rp.status, StatusCode::CREATED);
    let rp_id = rp.json_value()["id"].as_str().unwrap().to_string();

    let cp = post_json_on_host(&app, "/v1/projects", child_pat.actor(), CHILD_HOST, &json!({ "name": "Child Project" })).await?;
    assert_eq!(cp.status, StatusCode::CREATED);
    let cp_id = cp.json_value()["id"].as_str().unwrap().to_string();

    let rl = get_on_host(&app, "/v1/projects", admin_pat.actor(), ROOT_HOST).await?;
    let rj = rl.json_value();
    assert!(extract_ids(&rj).contains(&rp_id));
    assert!(!extract_ids(&rj).contains(&cp_id));

    let cl = get_on_host(&app, "/v1/projects", child_pat.actor(), CHILD_HOST).await?;
    let cj = cl.json_value();
    assert!(extract_ids(&cj).contains(&cp_id));
    assert!(!extract_ids(&cj).contains(&rp_id));

    // Path-scoped get child project from root.
    let ps = get_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/projects/{cp_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(ps.status, StatusCode::OK);
    assert_eq!(ps.json_value()["name"], "Child Project");

    Ok(())
}

// ─── Cross-child isolation ──────────────────────────────────

#[tokio::test]
async fn child_cannot_access_other_child_data() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "root-admin").await?;

    let (_, pat_a) = setup_child(&app, "child-a", "child-a.example.com", "org-a").await?;
    let (_, pat_b) = setup_child(&app, "child-b", "child-b.example.com", "org-b").await?;

    let ca = post_json_on_host(&app, "/v1/users", pat_a.actor(), "child-a.example.com",
        &json!({ "identifier": "alice@a.com", "display_name": "Alice" })).await?;
    assert_eq!(ca.status, StatusCode::CREATED);
    let ua = ca.json_value()["id"].as_str().unwrap().to_string();

    let cb = post_json_on_host(&app, "/v1/users", pat_b.actor(), "child-b.example.com",
        &json!({ "identifier": "bob@b.com", "display_name": "Bob" })).await?;
    assert_eq!(cb.status, StatusCode::CREATED);
    let ub = cb.json_value()["id"].as_str().unwrap().to_string();

    // Cross-child get — 404.
    let x1 = get_on_host(&app, &format!("/v1/users/{ua}"), pat_b.actor(), "child-b.example.com").await?;
    assert_eq!(x1.status, StatusCode::NOT_FOUND);
    let x2 = get_on_host(&app, &format!("/v1/users/{ub}"), pat_a.actor(), "child-a.example.com").await?;
    assert_eq!(x2.status, StatusCode::NOT_FOUND);

    // List isolation.
    let la = get_on_host(&app, "/v1/users", pat_a.actor(), "child-a.example.com").await?;
    let laj = la.json_value();
    assert!(extract_ids(&laj).contains(&ua));
    assert!(!extract_ids(&laj).contains(&ub));

    // Root admin sees both via path-scoped routes.
    let ra = get_on_host(&app, &format!("/v1/instances/child-a/users/{ua}"), admin_pat.actor(), ROOT_HOST).await?;
    assert_eq!(ra.status, StatusCode::OK);
    let rb = get_on_host(&app, &format!("/v1/instances/child-b/users/{ub}"), admin_pat.actor(), ROOT_HOST).await?;
    assert_eq!(rb.status, StatusCode::OK);

    Ok(())
}
