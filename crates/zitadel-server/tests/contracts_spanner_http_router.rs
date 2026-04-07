use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};

use axum::Router;
use serde_json::json;
use uuid::Uuid;
use zitadel_config::Config;
use zitadel_db::{DEFAULT_INSTANCE_ID, append_event};
use zitadel_server::{AppState, build_router, routing::InstanceResolver};
use zitadel_testkit::{TestApp, TestContext};

async fn build_spanner_test_app(suite: &str) -> anyhow::Result<Option<TestApp>> {
    let Some(ctx) = TestContext::spanner_from_env(suite).await? else {
        return Ok(None);
    };
    build_test_app_from_context(ctx).await.map(Some)
}

async fn build_seeded_spanner_test_app(suite: &str) -> anyhow::Result<Option<TestApp>> {
    let Some(stateful) = zitadel_db::test_support::spanner_stateful_config_from_env(suite).await?
    else {
        return Ok(None);
    };

    let mut config = Config::default();
    config.storage.primary = stateful;
    config.dev.seed_file = frontend_seed_file().to_string_lossy().into_owned();

    let ctx = TestContext::with_config(config).await?;
    build_test_app_from_context(ctx).await.map(Some)
}

fn frontend_seed_file() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/seeds/frontend.yaml")
        .canonicalize()
        .expect("resolve frontend seed file")
}

async fn build_test_app_from_context(ctx: TestContext) -> anyhow::Result<TestApp> {
    ctx.api_state
        .fga
        .reconcile_root_hierarchy(DEFAULT_INSTANCE_ID)
        .await?;

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

#[tokio::test]
async fn spanner_auth_routes_accept_session_cookie_and_pat_when_configured() -> anyhow::Result<()> {
    let Some(app) = build_spanner_test_app("server-contracts-auth").await? else {
        return Ok(());
    };

    let user = app
        .ctx
        .create_user("spanner-contract-user@example.com", "Password123!")
        .await?;
    let session = app.ctx.create_session(&user).await?;
    let user_pat = app.ctx.create_pat(&user, "spanner-user-pat").await?;

    let admin = app.ctx.admin_user().await?;
    app.ctx.grant_operator_admin(&admin).await?;
    let admin_pat = app.ctx.create_pat(&admin, "spanner-admin-pat").await?;

    let bearer_session = app.get("/v1/auth/whoami", session.bearer_actor()).await?;
    assert_eq!(bearer_session.status, axum::http::StatusCode::OK);
    assert_eq!(bearer_session.json_value()["token_type"], "session");

    let cookie_session = app
        .get(
            "/v1/auth/whoami",
            app.ctx.cookie_actor_for_token(&session.token),
        )
        .await?;
    assert_eq!(cookie_session.status, axum::http::StatusCode::OK);
    assert_eq!(cookie_session.json_value()["token_type"], "session");

    let pat_whoami = app.get("/v1/auth/whoami", user_pat.actor()).await?;
    assert_eq!(pat_whoami.status, axum::http::StatusCode::OK);
    assert_eq!(pat_whoami.json_value()["token_type"], "pat");

    let session_on_pat_only = app
        .get("/v1/internal/fga/platform/store", session.bearer_actor())
        .await?;
    assert_eq!(
        session_on_pat_only.status,
        axum::http::StatusCode::FORBIDDEN
    );

    let non_operator_pat = app
        .get("/v1/internal/fga/platform/store", user_pat.actor())
        .await?;
    assert_eq!(non_operator_pat.status, axum::http::StatusCode::FORBIDDEN);

    let operator_pat = app
        .get("/v1/internal/fga/platform/store", admin_pat.actor())
        .await?;
    assert_eq!(operator_pat.status, axum::http::StatusCode::OK);

    app.ctx
        .login_state
        .transient
        .revoke_session(DEFAULT_INSTANCE_ID, &session.session_id)
        .await?;
    let revoked = app.get("/v1/auth/whoami", session.bearer_actor()).await?;
    assert_eq!(revoked.status, axum::http::StatusCode::UNAUTHORIZED);

    Ok(())
}

#[tokio::test]
async fn spanner_observability_routes_expose_events_overview_and_analytics_when_configured()
-> anyhow::Result<()> {
    let Some(app) = build_spanner_test_app("server-contracts-observability").await? else {
        return Ok(());
    };

    let admin = app.ctx.admin_user().await?;
    app.ctx.grant_operator_admin(&admin).await?;
    let admin_pat = app.ctx.create_pat(&admin, "spanner-observability").await?;

    let user = app
        .ctx
        .create_user("spanner-observability-user@example.com", "Password123!")
        .await?;
    let _session = app.ctx.create_session(&user).await?;

    for event_type in [
        "auth.login_succeeded",
        "auth.token_issued",
        "auth.login_failed",
    ] {
        append_event(
            &app.ctx.db.db,
            DEFAULT_INSTANCE_ID,
            &Uuid::now_v7().to_string(),
            event_type,
            "auth",
            "flow-contracts",
            "fp-contracts",
            "{\"kind\":\"contract\"}",
            "{\"source\":\"spanner-contract\"}",
        )
        .await?;
    }

    let events = app.get("/v1/events?limit=10", admin_pat.actor()).await?;
    assert_eq!(events.status, axum::http::StatusCode::OK);
    let items = events.json_value()["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        items
            .iter()
            .any(|item| item["event_type"] == "auth.login_succeeded"),
        "event list should surface Spanner-backed auth events",
    );

    let overview = app
        .get("/v1/observability/overview?range=12h", admin_pat.actor())
        .await?;
    assert_eq!(overview.status, axum::http::StatusCode::OK);
    let overview_json = overview.json_value();
    assert!(
        overview_json["metrics"]["auth"]["current"]
            .as_i64()
            .is_some_and(|value| value >= 3),
        "overview auth metric should count recent auth events from Spanner",
    );
    assert!(
        overview_json["metrics"]["sessions"]["current"]
            .as_i64()
            .is_some_and(|value| value >= 1),
        "overview session metric should count live sessions from Spanner",
    );

    let schema = app.get("/v1/analytics/schema", admin_pat.actor()).await?;
    assert_eq!(schema.status, axum::http::StatusCode::OK);
    let schema_json = schema.json_value();
    assert!(schema_json.get("events").is_some());
    assert!(schema_json.get("sessions").is_some());

    let query = app
        .post_json(
            "/v1/analytics/query",
            admin_pat.actor(),
            &json!({
                "sql": "SELECT event_type, COUNT(*) AS total \
                         FROM events \
                         WHERE instance_id = 'default' \
                         GROUP BY event_type \
                         ORDER BY total DESC",
                "limit": 10
            }),
        )
        .await?;
    assert_eq!(query.status, axum::http::StatusCode::OK);
    let query_json = query.json_value();
    assert_eq!(query_json["error"], serde_json::Value::Null);
    assert!(
        query_json["rows"]
            .as_array()
            .is_some_and(|rows| rows.iter().any(|row| row[0] == "auth.login_succeeded")),
        "analytics query should decode grouped Spanner rows without TO_JSON_STRING hacks",
    );

    Ok(())
}

#[tokio::test]
async fn spanner_seeded_admin_can_log_in_with_default_password_when_configured()
-> anyhow::Result<()> {
    let Some(app) = build_seeded_spanner_test_app("server-contracts-seeded-admin").await? else {
        return Ok(());
    };

    let created_flow = app
        .post_json(
            "/v1/login/flows",
            zitadel_testkit::AuthActor::Anonymous,
            &json!({}),
        )
        .await?;
    assert_eq!(created_flow.status, axum::http::StatusCode::CREATED);

    let flow_id = created_flow.json_value()["flow_id"]
        .as_str()
        .expect("seeded admin login flow id should be present")
        .to_string();

    let advanced = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            zitadel_testkit::AuthActor::Anonymous,
            &json!({
                "action": "identifier",
                "identifier": "admin",
            }),
        )
        .await?;
    assert_eq!(advanced.status, axum::http::StatusCode::OK);
    assert_eq!(advanced.json_value()["step"], "password");

    let completed = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            zitadel_testkit::AuthActor::Anonymous,
            &json!({
                "action": "password",
                "password": "admin123",
            }),
        )
        .await?;
    assert_eq!(completed.status, axum::http::StatusCode::OK);
    assert_eq!(completed.json_value()["step"], "complete");

    Ok(())
}
