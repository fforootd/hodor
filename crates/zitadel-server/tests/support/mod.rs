use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
};

use anyhow::Context;
use axum::{
    Router,
    body::Body,
    http::{
        HeaderMap, Method,
        header::{CONTENT_TYPE, HOST},
    },
};
use serde_json::Value;
use uuid::Uuid;
use zitadel_api::ApiState;
use zitadel_authn::password::encode_credential_json;
use zitadel_config::Config;
use zitadel_db::DEFAULT_INSTANCE_ID;
use zitadel_fga::StoreResolver;
use zitadel_server::{AppState, build_router, routing::InstanceResolver};
use zitadel_testkit::{AuthActor, PatFixture, SessionFixture, TestApp, TestContext, UserFixture};

pub const ROOT_HOST: &str = "root.example.com";
pub const CHILD_HOST: &str = "child.example.com";
pub const CHILD_INSTANCE_ID: &str = "child-inst";
pub const CHILD_ORG_ID: &str = "child-org";

pub async fn build_cloud_test_app() -> anyhow::Result<TestApp> {
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

    rebuild_platform_fga(&ctx.api_state)
        .await
        .context("reconcile root fga tuples")?;

    let app_state = Arc::new(AppState {
        config: ctx.config.clone(),
        db: ctx.db.db.clone(),
        secret_box: Arc::new(zitadel_crypto::SecretBox::new("", &HashMap::new())?),
        ready: AtomicBool::new(true),
        instance_resolver: Arc::new(InstanceResolver::new(&ctx.config, ctx.db.db.clone())),
        app: ctx.api_state.app.clone(),
    });
    let router: Router = build_router(
        app_state,
        ctx.api_state.clone(),
        ctx.oidc_state.clone(),
        ctx.login_state.clone(),
    );

    Ok(TestApp::new(ctx, router))
}

pub async fn grant_org_role(
    app: &TestApp,
    org_id: &str,
    user_id: &str,
    role: &str,
) -> anyhow::Result<()> {
    grant_org_role_in_instance(app, DEFAULT_INSTANCE_ID, org_id, user_id, role).await
}

pub async fn grant_org_role_in_instance(
    app: &TestApp,
    instance_id: &str,
    org_id: &str,
    user_id: &str,
    role: &str,
) -> anyhow::Result<()> {
    zitadel_db::add_membership(&app.ctx.db.db, instance_id, "org", org_id, user_id, role)
        .await
        .with_context(|| format!("grant {role} membership for org {org_id} in {instance_id}"))?;
    rebuild_platform_fga(&app.ctx.api_state)
        .await
        .context("rebuild platform fga after membership grant")?;
    Ok(())
}

pub async fn create_root_user_in_org(
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

    grant_org_role(app, org_id, &user_id, "owner").await?;

    Ok(UserFixture {
        user_id,
        org_id: org_id.to_string(),
        identifier: identifier.to_string(),
    })
}

pub async fn create_session_for_instance(
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
            "server-test-support",
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

pub async fn insert_child_instance(
    app: &TestApp,
    instance_id: &str,
    owner_org_id: &str,
    domain: &str,
) -> anyhow::Result<()> {
    insert_instance_with_parent(
        app,
        instance_id,
        DEFAULT_INSTANCE_ID,
        owner_org_id,
        domain,
        "managed",
        "{}",
    )
    .await
}

pub async fn insert_instance_with_parent(
    app: &TestApp,
    instance_id: &str,
    parent_instance_id: &str,
    owner_org_id: &str,
    domain: &str,
    kind: &str,
    feature_overrides_json: &str,
) -> anyhow::Result<()> {
    let scoped = app.ctx.db.scoped_default();
    let sql = format!(
        "INSERT INTO instances (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, feature_overrides) \
         VALUES ($1, $2, $3, $4, 'active', 'global', {})",
        scoped.json_bind(5),
    );
    sqlx::query(&sql)
        .bind(instance_id)
        .bind(parent_instance_id)
        .bind(owner_org_id)
        .bind(kind)
        .bind(feature_overrides_json)
        .execute(scoped.pool())
        .await
        .context("insert instance")?;
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
    fga.initialize_instance(instance_id)
        .await
        .context("init instance fga")?;
    rebuild_platform_fga(&app.ctx.api_state)
        .await
        .context("rebuild platform fga after instance insert")?;

    Ok(())
}

pub async fn insert_instance_trust_link(
    app: &TestApp,
    child_instance_id: &str,
    issuer: &str,
    audience: &str,
    allowed_scopes_json: &str,
) -> anyhow::Result<()> {
    let scoped = app.ctx.db.scoped_default();
    let sql = format!(
        "INSERT INTO instance_trust_links (child_instance_id, issuer, audience, allowed_scopes, state) \
         VALUES ($1, $2, $3, {}, 'active')",
        scoped.json_bind(4),
    );
    sqlx::query(&sql)
        .bind(child_instance_id)
        .bind(issuer)
        .bind(audience)
        .bind(allowed_scopes_json)
        .execute(scoped.pool())
        .await
        .context("insert instance trust link")?;
    Ok(())
}

pub async fn setup_child(
    app: &TestApp,
    instance_id: &str,
    domain: &str,
    org_id: &str,
) -> anyhow::Result<(SessionFixture, PatFixture)> {
    insert_child_instance(
        app,
        instance_id,
        &app.ctx.db.default_org_id().await?,
        domain,
    )
    .await?;

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

    // Grant owner role so FGA checks pass for this child admin
    zitadel_db::add_membership(
        &app.ctx.db.db,
        instance_id,
        "org",
        org_id,
        &user_id,
        "owner",
    )
    .await
    .context("grant child admin org owner")?;
    rebuild_platform_fga(&app.ctx.api_state)
        .await
        .context("reconcile child fga tuples")?;

    let user = UserFixture {
        user_id: user_id.clone(),
        org_id: org_id.to_string(),
        identifier: "child-admin".into(),
    };

    let session = app
        .ctx
        .login_state
        .transient
        .create_session(
            instance_id,
            &user.user_id,
            &user.org_id,
            "test",
            "127.0.0.1",
            "",
        )
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

pub async fn insert_user_with_password(
    app: &TestApp,
    instance_id: &str,
    org_id: &str,
    identifier: &str,
    display_name: &str,
    password: &str,
) -> anyhow::Result<UserFixture> {
    let scoped = app.ctx.db.db.scoped(instance_id.to_string());
    sqlx::query(
        "INSERT OR IGNORE INTO orgs (instance_id, id, name, state) VALUES ($1, $2, $3, 'active')",
    )
    .bind(instance_id)
    .bind(org_id)
    .bind(org_id)
    .execute(scoped.pool())
    .await
    .context("ensure org for test user")?;

    let user_id = Uuid::new_v4().to_string();
    let credential_id = format!("cred-{user_id}");
    let password_hash = app
        .ctx
        .login_state
        .passwords
        .hash(password)
        .context("hash test password")?;
    let credential_json = encode_credential_json(&password_hash);
    let credential_sql = format!(
        "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, $2, $3, 'password', {})",
        scoped.json_bind(4),
    );

    sqlx::query(
        "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state) \
         VALUES ($1, $2, $3, $4, $5, 'human', 'active')",
    )
    .bind(&user_id)
    .bind(instance_id)
    .bind(org_id)
    .bind(identifier)
    .bind(display_name)
    .execute(scoped.pool())
    .await
    .context("insert test user")?;

    sqlx::query(&credential_sql)
        .bind(&credential_id)
        .bind(instance_id)
        .bind(&user_id)
        .bind(&credential_json)
        .execute(scoped.pool())
        .await
        .context("insert test password credential")?;

    Ok(UserFixture {
        user_id,
        org_id: org_id.to_string(),
        identifier: identifier.to_string(),
    })
}

pub async fn insert_oidc_auth_request(
    app: &TestApp,
    instance_id: &str,
    auth_request_id: &str,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    prompt_json: &str,
) -> anyhow::Result<()> {
    let scoped = app.ctx.db.db.scoped(instance_id.to_string());
    sqlx::query(
        "INSERT OR IGNORE INTO instances (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, feature_overrides) \
         VALUES ($1, $2, $3, 'managed', 'active', 'global', '{}')",
    )
    .bind(instance_id)
    .bind(DEFAULT_INSTANCE_ID)
    .bind(app.ctx.db.default_org_id().await?)
    .execute(scoped.pool())
    .await
    .context("ensure instance for auth request")?;

    sqlx::query(
        "INSERT INTO oidc_auth_requests (id, instance_id, client_id, redirect_uri, state, prompt) \
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(auth_request_id)
    .bind(instance_id)
    .bind(client_id)
    .bind(redirect_uri)
    .bind(state)
    .bind(prompt_json)
    .execute(scoped.pool())
    .await
    .context("insert oidc auth request")?;

    Ok(())
}

pub fn host_headers(host: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(HOST, host.parse().unwrap());
    headers
}

pub fn json_host_headers(host: &str) -> HeaderMap {
    let mut headers = host_headers(host);
    headers.insert(CONTENT_TYPE, "application/json".parse().unwrap());
    headers
}

pub async fn get_on_host(
    app: &TestApp,
    path: &str,
    actor: AuthActor,
    host: &str,
) -> anyhow::Result<zitadel_testkit::TestResponse> {
    app.request(Method::GET, path, actor, host_headers(host), Body::empty())
        .await
}

pub async fn post_json_on_host(
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

pub async fn patch_json_on_host(
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

pub async fn delete_on_host(
    app: &TestApp,
    path: &str,
    actor: AuthActor,
    host: &str,
) -> anyhow::Result<zitadel_testkit::TestResponse> {
    app.request(
        Method::DELETE,
        path,
        actor,
        host_headers(host),
        Body::empty(),
    )
    .await
}

pub fn extract_ids(value: &Value) -> Vec<String> {
    value["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| item["id"].as_str().map(ToOwned::to_owned))
        .collect()
}

pub async fn rebuild_platform_fga(api_state: &ApiState) -> anyhow::Result<()> {
    api_state
        .fga
        .rebuild_platform_store()
        .await
        .context("rebuild platform store")?;
    Ok(())
}
