mod support;

use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
};

use axum::{
    Router,
    http::{HeaderMap, StatusCode, header},
};
use serde_json::json;
use zitadel_db::DEFAULT_INSTANCE_ID;
use zitadel_fga::core_authorization_model;
use zitadel_oidc::oidc::s256_challenge;
use zitadel_server::{AppState, build_router, routing::InstanceResolver};
use zitadel_testkit::{AuthActor, TestApp, TestContext};

use support::insert_oidc_auth_request;

async fn build_test_app() -> anyhow::Result<TestApp> {
    let ctx = TestContext::new().await?;
    build_test_app_from_context(ctx).await
}

async fn build_dynamic_origin_test_app() -> anyhow::Result<TestApp> {
    let mut ctx = TestContext::new().await?;
    ctx.config.server.public_origin.clear();
    ctx.oidc_state.provider = ctx.oidc_state.provider.clone().with_issuer_override(None);
    ctx.login_state.public_origin_override = None;
    build_test_app_from_context(ctx).await
}

async fn build_test_app_from_context(ctx: TestContext) -> anyhow::Result<TestApp> {
    // Reconcile FGA tuples so the bootstrapped admin has proper permissions
    ctx.api_state
        .fga
        .reconcile_root_hierarchy(zitadel_db::DEFAULT_INSTANCE_ID)
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

async fn assert_named_resource_crud(
    app: &TestApp,
    actor: AuthActor,
    base_path: &str,
    singular_name: &str,
    created_name: &str,
    updated_name: &str,
) -> anyhow::Result<()> {
    let bad_request = app.post_json(base_path, actor.clone(), &json!({})).await?;
    assert_eq!(bad_request.status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(
        bad_request.json_value()["code"],
        serde_json::Value::from(400)
    );
    assert!(
        bad_request.json_value()["error"]
            .as_str()
            .is_some_and(|message| message.contains("name is required")),
        "{singular_name} create should keep the uniform validation shape",
    );

    let create_payload = if base_path == "/v1/apps" {
        json!({
            "name": created_name,
            "client_id": format!("{}-client", created_name.to_lowercase().replace(' ', "-")),
            "app_type": "web",
        })
    } else {
        json!({ "name": created_name })
    };

    let created = app
        .post_json(base_path, actor.clone(), &create_payload)
        .await?;
    assert_eq!(created.status, axum::http::StatusCode::CREATED);
    let created_id = created.json_value()["id"]
        .as_str()
        .expect("created resource id should be present")
        .to_string();

    let loaded = app
        .get(&format!("{base_path}/{created_id}"), actor.clone())
        .await?;
    assert_eq!(loaded.status, axum::http::StatusCode::OK);
    assert_eq!(loaded.json_value()["name"], created_name);

    let updated = app
        .patch_json(
            &format!("{base_path}/{created_id}"),
            actor.clone(),
            &json!({ "name": updated_name }),
        )
        .await?;
    assert_eq!(updated.status, axum::http::StatusCode::OK);
    let updated_json = updated.json_value();
    assert!(
        updated_json["updated"] == serde_json::Value::Bool(true)
            || updated_json["name"] == serde_json::Value::String(updated_name.to_string()),
        "{singular_name} update should either confirm the mutation or return the updated resource",
    );

    let reloaded = app
        .get(&format!("{base_path}/{created_id}"), actor.clone())
        .await?;
    assert_eq!(reloaded.status, axum::http::StatusCode::OK);
    assert_eq!(reloaded.json_value()["name"], updated_name);

    let list = app.get(base_path, actor.clone()).await?;
    assert_eq!(list.status, axum::http::StatusCode::OK);
    assert!(
        list.json_value()["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["id"] == created_id)),
        "created {singular_name} should appear in the list endpoint",
    );

    let deleted = app
        .delete(&format!("{base_path}/{created_id}"), actor.clone())
        .await?;
    assert_eq!(deleted.status, axum::http::StatusCode::NO_CONTENT);

    let missing = app
        .get(&format!("{base_path}/{created_id}"), actor.clone())
        .await?;
    assert_eq!(missing.status, axum::http::StatusCode::NOT_FOUND);

    let post_delete_list = app.get(base_path, actor).await?;
    assert_eq!(post_delete_list.status, axum::http::StatusCode::OK);
    assert!(
        post_delete_list.json_value()["items"]
            .as_array()
            .is_some_and(|items| items.iter().all(|item| item["id"] != created_id)),
        "deleted {singular_name} should disappear from the list endpoint",
    );

    Ok(())
}

fn query_param(input: &str, key: &str) -> Option<String> {
    let query = input.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        if name == key {
            Some(value.to_string())
        } else {
            None
        }
    })
}

#[tokio::test]
async fn public_surfaces_remain_unauthenticated_and_readyz_reflects_state() -> anyhow::Result<()> {
    let app = build_test_app().await?;

    let health = app.get("/healthz", AuthActor::Anonymous).await?;
    assert_eq!(health.status, axum::http::StatusCode::OK);
    assert_eq!(health.text(), "ok");

    let ready = app.get("/readyz", AuthActor::Anonymous).await?;
    assert_eq!(ready.status, axum::http::StatusCode::OK);
    assert_eq!(ready.text(), "ready");

    let discovery = app
        .get("/.well-known/openid-configuration", AuthActor::Anonymous)
        .await?;
    assert_eq!(discovery.status, axum::http::StatusCode::OK);
    let discovery_json = discovery.json_value();
    assert_eq!(
        discovery_json["issuer"],
        serde_json::Value::String("http://localhost:18080".into())
    );

    let openapi = app.get("/openapi.json", AuthActor::Anonymous).await?;
    assert_eq!(openapi.status, axum::http::StatusCode::OK);
    let openapi_json = openapi.json_value();
    let servers = openapi_json["servers"]
        .as_array()
        .expect("openapi servers should be present");
    assert!(
        servers.iter().any(|server| {
            server["url"]
                .as_str()
                .is_some_and(|url| url == "http://localhost:18080")
        }),
        "openapi should advertise the configured public origin",
    );
    let paths = openapi_json["paths"]
        .as_object()
        .expect("openapi paths should be present");
    assert!(
        paths.contains_key("/v1/fga/store"),
        "runtime openapi should expose FGA store discovery",
    );
    assert!(
        paths.contains_key("/v1/fga/stores/{store_id}/check"),
        "runtime openapi should expose canonical FGA check",
    );
    assert!(
        paths.contains_key("/v1/fga/stores/{store_id}/authorization-models"),
        "runtime openapi should expose canonical FGA authorization-model routes",
    );
    assert!(
        paths.contains_key("/v1/internal/fga/platform/store"),
        "runtime openapi should expose the internal platform FGA inspection routes",
    );

    Ok(())
}

#[tokio::test]
async fn unpinned_request_origin_shapes_discovery_and_openapi() -> anyhow::Result<()> {
    let app = build_dynamic_origin_test_app().await?;
    let mut headers = HeaderMap::new();
    headers.insert(header::HOST, "demo.example.com".parse()?);
    headers.insert("X-Forwarded-Proto", "https".parse()?);

    let discovery = app
        .request(
            "GET".parse()?,
            "/.well-known/openid-configuration",
            AuthActor::Anonymous,
            headers.clone(),
            axum::body::Body::empty(),
        )
        .await?;
    assert_eq!(discovery.status, axum::http::StatusCode::OK);
    assert_eq!(discovery.json_value()["issuer"], "https://demo.example.com");

    let openapi = app
        .request(
            "GET".parse()?,
            "/openapi.json",
            AuthActor::Anonymous,
            headers,
            axum::body::Body::empty(),
        )
        .await?;
    assert_eq!(openapi.status, axum::http::StatusCode::OK);
    let openapi_json = openapi.json_value();
    let servers = openapi_json["servers"]
        .as_array()
        .expect("openapi servers should be present");
    assert!(servers.iter().any(|server| {
        server["url"]
            .as_str()
            .is_some_and(|url| url == "https://demo.example.com")
    }));

    Ok(())
}

#[tokio::test]
async fn protected_routes_enforce_actor_contracts() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let user = app
        .ctx
        .create_user("route-user@example.com", "password123")
        .await?;
    // Grant viewer role so FGA checks pass for read endpoints
    support::grant_org_role(&app, &user.org_id, &user.user_id, "viewer").await?;
    app.ctx
        .api_state
        .fga
        .reconcile_root_hierarchy(zitadel_db::DEFAULT_INSTANCE_ID)
        .await?;
    let user_session = app.ctx.create_session(&user).await?;
    let user_pat = app.ctx.create_pat(&user, "route-user").await?;
    let admin_user = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin_user, "route-admin").await?;

    for path in ["/v1/users", "/v1/sessions", "/v1/auth/whoami"] {
        let unauth = app.get(path, AuthActor::Anonymous).await?;
        assert_eq!(
            unauth.status,
            axum::http::StatusCode::UNAUTHORIZED,
            "{path}"
        );
        assert_eq!(
            unauth.json_value(),
            json!({"error": "authentication required", "code": 401}),
            "{path} should keep the uniform 401 shape",
        );

        let user_response = app.get(path, user_session.bearer_actor()).await?;
        assert_ne!(
            user_response.status,
            axum::http::StatusCode::UNAUTHORIZED,
            "{path}"
        );
        assert_ne!(
            user_response.status,
            axum::http::StatusCode::FORBIDDEN,
            "{path}"
        );

        let admin_response = app.get(path, admin_pat.actor()).await?;
        assert_ne!(
            admin_response.status,
            axum::http::StatusCode::UNAUTHORIZED,
            "{path}"
        );
        assert_ne!(
            admin_response.status,
            axum::http::StatusCode::FORBIDDEN,
            "{path}"
        );
    }

    for path in ["/v1/fga/model", "/v1/fga/store"] {
        let unauth = app.get(path, AuthActor::Anonymous).await?;
        assert_eq!(
            unauth.status,
            axum::http::StatusCode::UNAUTHORIZED,
            "{path}"
        );
        assert_eq!(
            unauth.json_value(),
            json!({"error": "authentication required", "code": 401}),
            "{path} should keep the uniform 401 shape",
        );

        let user_response = app.get(path, user_session.bearer_actor()).await?;
        assert_eq!(user_response.status, axum::http::StatusCode::OK, "{path}");

        let user_pat_response = app.get(path, user_pat.actor()).await?;
        assert_eq!(
            user_pat_response.status,
            axum::http::StatusCode::OK,
            "{path}"
        );

        let admin_response = app.get(path, admin_pat.actor()).await?;
        assert_eq!(admin_response.status, axum::http::StatusCode::OK, "{path}");
    }

    for path in ["/v1/internal/fga/platform/store"] {
        let unauth = app.get(path, AuthActor::Anonymous).await?;
        assert_eq!(
            unauth.status,
            axum::http::StatusCode::UNAUTHORIZED,
            "{path}"
        );

        let user_response = app.get(path, user_session.bearer_actor()).await?;
        assert_eq!(
            user_response.status,
            axum::http::StatusCode::FORBIDDEN,
            "{path} should remain PAT-only",
        );
        assert_eq!(
            user_response.json_value(),
            json!({"error": "personal access token required", "code": 403}),
            "{path} should clearly report the PAT-only boundary",
        );

        let user_pat_response = app.get(path, user_pat.actor()).await?;
        assert_eq!(
            user_pat_response.status,
            axum::http::StatusCode::FORBIDDEN,
            "{path} should require operator admin",
        );
        assert_eq!(
            user_pat_response.json_value(),
            json!({"error": "operator admin required", "code": 403}),
            "{path} should clearly report the operator-admin boundary",
        );

        let admin_response = app.get(path, admin_pat.actor()).await?;
        assert_eq!(admin_response.status, axum::http::StatusCode::OK, "{path}");
    }

    Ok(())
}

#[tokio::test]
async fn auth_resolution_accepts_session_pat_cookie_and_oidc_tokens() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let user = app
        .ctx
        .create_user("auth-user@example.com", "password123")
        .await?;
    let session = app.ctx.create_session(&user).await?;
    let pat = app.ctx.create_pat(&user, "auth-user-pat").await?;
    let oidc_token = app.ctx.mint_oidc_access_token_for_user(&user).await?;

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

    let pat_response = app.get("/v1/auth/whoami", pat.actor()).await?;
    assert_eq!(pat_response.status, axum::http::StatusCode::OK);
    assert_eq!(pat_response.json_value()["token_type"], "pat");

    let oidc_response = app
        .get("/v1/auth/whoami", AuthActor::bearer(oidc_token))
        .await?;
    assert_eq!(oidc_response.status, axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(
        oidc_response.json_value(),
        json!({"error": "invalid or expired token", "code": 401})
    );

    let tampered_cookie = app
        .get(
            "/v1/auth/whoami",
            AuthActor::cookie(format!(
                "{}={}",
                app.ctx.cookie_config.cookie_name(),
                "tampered-cookie"
            )),
        )
        .await?;
    assert_eq!(tampered_cookie.status, axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(
        tampered_cookie.json_value(),
        json!({"error": "authentication required", "code": 401})
    );

    let mut extra_headers = HeaderMap::new();
    extra_headers.insert(
        axum::http::header::COOKIE,
        app.ctx
            .cookie_header_for_token(&session.token)
            .parse()
            .unwrap(),
    );
    let precedence = app
        .request(
            axum::http::Method::GET,
            "/v1/auth/whoami",
            pat.actor(),
            extra_headers,
            axum::body::Body::empty(),
        )
        .await?;
    assert_eq!(precedence.status, axum::http::StatusCode::OK);
    assert_eq!(precedence.json_value()["token_type"], "pat");

    app.ctx
        .login_state
        .transient
        .revoke_session(DEFAULT_INSTANCE_ID, &session.session_id)
        .await?;
    let revoked = app.get("/v1/auth/whoami", session.bearer_actor()).await?;
    assert_eq!(revoked.status, axum::http::StatusCode::UNAUTHORIZED);
    assert_eq!(
        revoked.json_value(),
        json!({"error": "invalid or expired token", "code": 401})
    );

    Ok(())
}

#[tokio::test]
async fn canonical_fga_store_routes_support_model_tuple_and_change_queries() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let admin_user = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin_user, "fga-admin").await?;

    let store = app.get("/v1/fga/store", admin_pat.actor()).await?;
    assert_eq!(store.status, axum::http::StatusCode::OK);
    let store_id = store.json_value()["store_id"]
        .as_str()
        .expect("store_id should be present")
        .to_string();

    let latest_model = app
        .get(
            &format!("/v1/fga/stores/{store_id}/authorization-models"),
            admin_pat.actor(),
        )
        .await?;
    assert_eq!(latest_model.status, axum::http::StatusCode::OK);
    assert!(
        latest_model.json_value()["authorization_models"]
            .as_array()
            .is_some_and(|models| !models.is_empty())
    );

    let mut custom_model = core_authorization_model();
    custom_model
        .type_definitions
        .push(serde_json::from_value(json!({
            "type": "document",
            "relations": {
                "viewer": { "this": {} }
            },
            "metadata": {
                "relations": {
                    "viewer": { "directly_related_user_types": [{ "type": "user" }] }
                }
            }
        }))?);
    let custom_model_json = serde_json::to_value(&custom_model)?;

    let written_model = app
        .post_json(
            &format!("/v1/fga/stores/{store_id}/authorization-models"),
            admin_pat.actor(),
            &custom_model_json,
        )
        .await?;
    assert_eq!(written_model.status, axum::http::StatusCode::OK);
    assert!(written_model.json_value()["authorization_model_id"].is_string());

    let write = app
        .post_json(
            &format!("/v1/fga/stores/{store_id}/write"),
            admin_pat.actor(),
            &json!({
                "writes": {
                    "tuple_keys": [
                        {
                            "user": format!("user:{}", admin_user.user_id),
                            "relation": "viewer",
                            "object": "document:architecture"
                        }
                    ]
                },
                "deletes": { "tuple_keys": [] }
            }),
        )
        .await?;
    assert_eq!(write.status, axum::http::StatusCode::OK);
    assert_eq!(write.json_value(), json!({}));

    let check = app
        .post_json(
            &format!("/v1/fga/stores/{store_id}/check"),
            admin_pat.actor(),
            &json!({
                "tuple_key": {
                    "user": format!("user:{}", admin_user.user_id),
                    "relation": "viewer",
                    "object": "document:architecture"
                }
            }),
        )
        .await?;
    assert_eq!(check.status, axum::http::StatusCode::OK);
    assert_eq!(check.json_value(), json!({ "allowed": true }));

    let tuples = app
        .post_json(
            &format!("/v1/fga/stores/{store_id}/read"),
            admin_pat.actor(),
            &json!({
                "tuple_key": {
                    "object": "document:architecture"
                }
            }),
        )
        .await?;
    assert_eq!(tuples.status, axum::http::StatusCode::OK);
    assert!(
        tuples.json_value()["tuples"]
            .as_array()
            .is_some_and(|items| items.len() == 1)
    );

    let changes = app
        .get(
            &format!("/v1/fga/stores/{store_id}/changes?type=document"),
            admin_pat.actor(),
        )
        .await?;
    assert_eq!(changes.status, axum::http::StatusCode::OK);
    assert!(
        changes.json_value()["changes"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );

    let platform_rejected = app
        .post_json(
            "/v1/fga/stores/platform/check",
            admin_pat.actor(),
            &json!({
                "tuple_key": {
                    "user": format!("user:{}", admin_user.user_id),
                    "relation": "viewer",
                    "object": "document:architecture"
                }
            }),
        )
        .await?;
    assert_eq!(platform_rejected.status, axum::http::StatusCode::FORBIDDEN);

    Ok(())
}

#[tokio::test]
async fn users_crud_and_validation_work_through_the_router() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let admin_user = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin_user, "users-admin").await?;

    let bad_request = app
        .post_json(
            "/v1/users",
            admin_pat.actor(),
            &json!({
                "display_name": "Missing identifier",
            }),
        )
        .await?;
    assert_eq!(bad_request.status, axum::http::StatusCode::BAD_REQUEST);
    assert_eq!(
        bad_request.json_value()["code"],
        serde_json::Value::from(400)
    );
    assert!(
        bad_request.json_value()["error"]
            .as_str()
            .is_some_and(|message| message.contains("identifier is required"))
    );

    let created = app
        .post_json(
            "/v1/users",
            admin_pat.actor(),
            &json!({
                "identifier": "crud-user@example.com",
                "display_name": "CRUD User",
                "schema_id": "human_user_v1",
                "metadata": {"team": "qa"},
            }),
        )
        .await?;
    assert_eq!(created.status, axum::http::StatusCode::CREATED);
    let created_json = created.json_value();
    let user_id = created_json["id"]
        .as_str()
        .expect("created user id should be present");

    let loaded = app
        .get(&format!("/v1/users/{user_id}"), admin_pat.actor())
        .await?;
    assert_eq!(loaded.status, axum::http::StatusCode::OK);
    assert_eq!(loaded.json_value()["identifier"], "crud-user@example.com");

    let updated = app
        .patch_json(
            &format!("/v1/users/{user_id}"),
            admin_pat.actor(),
            &json!({
                "display_name": "CRUD User Updated",
                "metadata": {"team": "platform-security"},
            }),
        )
        .await?;
    assert_eq!(updated.status, axum::http::StatusCode::OK);
    let updated_json = updated.json_value();
    assert_eq!(updated_json["display_name"], "CRUD User Updated");
    assert_eq!(updated_json["metadata"]["team"], "platform-security");

    let list = app.get("/v1/users", admin_pat.actor()).await?;
    assert_eq!(list.status, axum::http::StatusCode::OK);
    let list_json = list.json_value();
    assert!(
        list_json["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["id"] == user_id)),
        "created user should be returned by the list endpoint",
    );

    let deleted = app
        .delete(&format!("/v1/users/{user_id}"), admin_pat.actor())
        .await?;
    assert_eq!(deleted.status, axum::http::StatusCode::NO_CONTENT);

    let missing = app
        .get(&format!("/v1/users/{user_id}"), admin_pat.actor())
        .await?;
    assert_eq!(missing.status, axum::http::StatusCode::NOT_FOUND);

    let post_delete_list = app.get("/v1/users", admin_pat.actor()).await?;
    assert_eq!(post_delete_list.status, axum::http::StatusCode::OK);
    assert!(
        post_delete_list.json_value()["items"]
            .as_array()
            .is_some_and(|items| items.iter().all(|item| item["id"] != user_id)),
        "deleted user should disappear from the list endpoint",
    );

    Ok(())
}

#[tokio::test]
async fn groups_crud_and_validation_work_through_the_router() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let admin_user = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin_user, "groups-admin").await?;

    assert_named_resource_crud(
        &app,
        admin_pat.actor(),
        "/v1/groups",
        "group",
        "Platform Engineers",
        "Platform Security Engineers",
    )
    .await
}

#[tokio::test]
async fn projects_crud_and_validation_work_through_the_router() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let admin_user = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin_user, "projects-admin").await?;

    assert_named_resource_crud(
        &app,
        admin_pat.actor(),
        "/v1/projects",
        "project",
        "Customer Portal",
        "Customer Portal Renamed",
    )
    .await
}

#[tokio::test]
async fn apps_crud_and_validation_work_through_the_router() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let admin_user = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin_user, "apps-admin").await?;

    assert_named_resource_crud(
        &app,
        admin_pat.actor(),
        "/v1/apps",
        "application",
        "Console Frontend",
        "Console Frontend Renamed",
    )
    .await
}

#[tokio::test]
async fn login_flows_create_sessions_and_support_session_reuse() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let user = app
        .ctx
        .create_user("login-user@example.com", "password123")
        .await?;

    let created_flow = app
        .post_json("/v1/login/flows", AuthActor::Anonymous, &json!({}))
        .await?;
    assert_eq!(created_flow.status, axum::http::StatusCode::CREATED);
    let created_flow_json = created_flow.json_value();
    assert_eq!(created_flow_json["step"], "identifier");
    let flow_id = created_flow_json["flow_id"]
        .as_str()
        .expect("login flow id should be present")
        .to_string();

    let advanced = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            AuthActor::Anonymous,
            &json!({
                "action": "identifier",
                "identifier": user.identifier,
            }),
        )
        .await?;
    assert_eq!(advanced.status, axum::http::StatusCode::OK);
    assert_eq!(advanced.json_value()["step"], "password");

    let invalid_password = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            AuthActor::Anonymous,
            &json!({
                "action": "password",
                "password": "wrong-password",
            }),
        )
        .await?;
    assert_eq!(invalid_password.status, axum::http::StatusCode::OK);
    assert_eq!(invalid_password.json_value()["step"], "password");

    let completed = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            AuthActor::Anonymous,
            &json!({
                "action": "password",
                "password": "password123",
            }),
        )
        .await?;
    assert_eq!(completed.status, axum::http::StatusCode::OK);
    let completed_json = completed.json_value();
    assert_eq!(completed_json["step"], "complete");
    assert_eq!(completed_json["redirect_uri"], "/console");
    let cookie = completed
        .set_cookie()
        .expect("password login should set a session cookie");
    let request_cookie = cookie
        .split(';')
        .next()
        .expect("set-cookie should contain the cookie pair")
        .to_string();

    let whoami = app
        .get("/v1/auth/whoami", AuthActor::cookie(request_cookie))
        .await?;
    assert_eq!(whoami.status, axum::http::StatusCode::OK);
    assert_eq!(whoami.json_value()["token_type"], "session");

    let existing_session = app.ctx.create_session(&user).await?;
    let reuse_flow = app
        .post_json(
            "/v1/login/flows",
            app.ctx.cookie_actor_for_token(&existing_session.token),
            &json!({}),
        )
        .await?;
    assert_eq!(reuse_flow.status, axum::http::StatusCode::CREATED);
    let reuse_json = reuse_flow.json_value();
    assert_eq!(reuse_json["step"], "session_reuse");
    let reuse_flow_id = reuse_json["flow_id"]
        .as_str()
        .expect("reuse flow id should be present");

    let reused = app
        .post_json(
            &format!("/v1/login/flows/{reuse_flow_id}/submit"),
            app.ctx.cookie_actor_for_token(&existing_session.token),
            &json!({
                "action": "use_session",
            }),
        )
        .await?;
    assert_eq!(reused.status, axum::http::StatusCode::OK);
    assert_eq!(reused.json_value()["step"], "complete");
    assert_eq!(reused.json_value()["redirect_uri"], "/console");

    Ok(())
}

#[tokio::test]
async fn login_flows_respect_prompt_login_even_with_an_existing_session() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let user = app
        .ctx
        .create_user("prompt-login@example.com", "password123")
        .await?;
    let existing_session = app.ctx.create_session(&user).await?;

    insert_oidc_auth_request(
        &app,
        DEFAULT_INSTANCE_ID,
        "prompt-login-auth",
        "client-1",
        "https://rp.example/callback",
        "prompt-login-state",
        r#"["login"]"#,
    )
    .await?;

    let created_flow = app
        .post_json(
            "/v1/login/flows",
            app.ctx.cookie_actor_for_token(&existing_session.token),
            &json!({
                "auth_request_id": "prompt-login-auth",
            }),
        )
        .await?;
    assert_eq!(created_flow.status, axum::http::StatusCode::CREATED);
    let created_flow_json = created_flow.json_value();
    assert_eq!(created_flow_json["step"], "identifier");

    let flow_id = created_flow_json["flow_id"]
        .as_str()
        .expect("prompt=login flow id should be present");
    let advanced = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            AuthActor::Anonymous,
            &json!({
                "action": "identifier",
                "identifier": user.identifier,
            }),
        )
        .await?;
    assert_eq!(advanced.status, axum::http::StatusCode::OK);
    assert_eq!(advanced.json_value()["step"], "password");

    let completed = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            AuthActor::Anonymous,
            &json!({
                "action": "password",
                "password": "password123",
            }),
        )
        .await?;
    assert_eq!(completed.status, axum::http::StatusCode::OK);
    assert_eq!(completed.json_value()["step"], "complete");
    let completed_json = completed.json_value();
    let redirect_uri = completed_json["redirect_uri"].as_str().unwrap_or_default();
    assert!(
        redirect_uri.starts_with("https://rp.example/callback?"),
        "prompt=login completion should still finish the OIDC redirect",
    );

    Ok(())
}

#[tokio::test]
async fn oidc_authorization_code_pkce_round_trip_survives_login_completion() -> anyhow::Result<()>
{
    let app = build_test_app().await?;
    let user = app
        .ctx
        .create_user("oidc-code@example.com", "password123")
        .await?;
    let client = app.ctx.create_oidc_client(&["authorization_code"]).await?;
    let state = "state-1";
    let nonce = "nonce-1";
    let code_verifier = "verifier-1";
    let authorize = app
        .get(
            &format!(
                "/authorize?client_id={}&redirect_uri={}&response_type=code&scope=openid%20profile%20email&state={state}&nonce={nonce}&code_challenge={}&code_challenge_method=S256",
                client.client_id,
                client.redirect_uri,
                s256_challenge(code_verifier),
            ),
            AuthActor::Anonymous,
        )
        .await?;
    assert_eq!(authorize.status, StatusCode::SEE_OTHER);

    let auth_request_id = query_param(
        authorize
            .headers
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default(),
        "auth_request_id",
    )
    .expect("authorize redirect should include auth_request_id");

    let created_flow = app
        .post_json(
            "/v1/login/flows",
            AuthActor::Anonymous,
            &json!({
                "auth_request_id": auth_request_id,
            }),
        )
        .await?;
    assert_eq!(created_flow.status, StatusCode::CREATED);

    let flow_id = created_flow.json_value()["flow_id"]
        .as_str()
        .expect("flow id should be present")
        .to_string();

    let advanced = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            AuthActor::Anonymous,
            &json!({
                "action": "identifier",
                "identifier": user.identifier,
            }),
        )
        .await?;
    assert_eq!(advanced.status, StatusCode::OK);
    assert_eq!(advanced.json_value()["step"], "password");

    let completed = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            AuthActor::Anonymous,
            &json!({
                "action": "password",
                "password": "password123",
            }),
        )
        .await?;
    assert_eq!(completed.status, StatusCode::OK, "{}", completed.text());
    assert_eq!(completed.json_value()["step"], "complete");

    let callback = completed.json_value()["redirect_uri"]
        .as_str()
        .expect("callback redirect should be present")
        .to_string();
    assert!(
        callback.starts_with(&format!("{}?", client.redirect_uri)),
        "completion should redirect back to the client callback",
    );
    assert_eq!(query_param(&callback, "state").as_deref(), Some(state));

    let code = query_param(&callback, "code").expect("callback redirect should include code");

    let token = app
        .post_form(
            "/oauth/token",
            AuthActor::Anonymous,
            &format!(
                "grant_type=authorization_code&code={code}&redirect_uri={}&client_id={}&client_secret={}&code_verifier={code_verifier}",
                client.redirect_uri, client.client_id, client.client_secret,
            ),
        )
        .await?;
    assert_eq!(token.status, StatusCode::OK, "{}", token.text());
    let token_json = token.json_value();
    let access_token = token_json["access_token"]
        .as_str()
        .expect("access token should be present")
        .to_string();
    assert!(token_json["id_token"].as_str().is_some());
    assert_eq!(token_json["token_type"], "Bearer");

    let userinfo = app
        .get("/userinfo", AuthActor::bearer(access_token))
        .await?;
    assert_eq!(userinfo.status, StatusCode::OK, "{}", userinfo.text());
    let userinfo_json = userinfo.json_value();
    assert_eq!(userinfo_json["email"], user.identifier);
    assert_eq!(userinfo_json["email_verified"], true);

    let reused = app
        .post_form(
            "/oauth/token",
            AuthActor::Anonymous,
            &format!(
                "grant_type=authorization_code&code={code}&redirect_uri={}&client_id={}&client_secret={}&code_verifier={code_verifier}",
                client.redirect_uri, client.client_id, client.client_secret,
            ),
        )
        .await?;
    assert_eq!(reused.status, StatusCode::BAD_REQUEST, "{}", reused.text());
    assert_eq!(reused.json_value()["error"], "invalid_grant");

    Ok(())
}

#[tokio::test]
async fn login_flows_reject_foreign_auth_requests_without_creating_a_session() -> anyhow::Result<()>
{
    let app = build_test_app().await?;
    let user = app
        .ctx
        .create_user("cross-instance-login@example.com", "password123")
        .await?;

    insert_oidc_auth_request(
        &app,
        "foreign-instance",
        "foreign-auth",
        "client-foreign",
        "https://rp.example/callback",
        "foreign-state",
        "[]",
    )
    .await?;

    let created_flow = app
        .post_json(
            "/v1/login/flows",
            AuthActor::Anonymous,
            &json!({
                "auth_request_id": "foreign-auth",
            }),
        )
        .await?;
    assert_eq!(created_flow.status, axum::http::StatusCode::CREATED);
    let created_flow_json = created_flow.json_value();
    let flow_id = created_flow_json["flow_id"]
        .as_str()
        .expect("cross-instance flow id should be present")
        .to_string();

    let advanced = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            AuthActor::Anonymous,
            &json!({
                "action": "identifier",
                "identifier": user.identifier,
            }),
        )
        .await?;
    assert_eq!(advanced.status, axum::http::StatusCode::OK);
    assert_eq!(advanced.json_value()["step"], "password");

    let failed = app
        .post_json(
            &format!("/v1/login/flows/{flow_id}/submit"),
            AuthActor::Anonymous,
            &json!({
                "action": "password",
                "password": "password123",
            }),
        )
        .await?;
    assert_eq!(
        failed.status,
        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        "cross-instance auth requests must fail closed",
    );

    let scoped = app.ctx.db.scoped_default();
    let session_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND user_id = $2")
            .bind(scoped.instance_id())
            .bind(&user.user_id)
            .fetch_one(scoped.pool())
            .await?;
    assert_eq!(session_count.0, 0);

    let foreign_scoped = app.ctx.db.db.scoped("foreign-instance".to_string());
    let foreign_row: (String, String, i64) = sqlx::query_as(&format!(
        "SELECT COALESCE(user_id, ''), COALESCE(code, ''), {} FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2",
        foreign_scoped.bool_as_int("done"),
    ))
    .bind(foreign_scoped.instance_id())
    .bind("foreign-auth")
    .fetch_one(foreign_scoped.pool())
    .await?;
    assert_eq!(foreign_row.0, "");
    assert_eq!(foreign_row.1, "");
    assert_eq!(foreign_row.2, 0);

    Ok(())
}

#[tokio::test]
async fn sso_callback_errors_redirect_back_to_login() -> anyhow::Result<()> {
    let app = build_test_app().await?;

    let callback_error = app
        .get(
            "/v1/auth/sso/callback?error=access_denied&error_description=upstream+denied",
            AuthActor::Anonymous,
        )
        .await?;
    assert_eq!(
        callback_error.status,
        axum::http::StatusCode::TEMPORARY_REDIRECT
    );
    let location = callback_error
        .headers
        .get(axum::http::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(
        location.starts_with("/login?error=sso_failed"),
        "unexpected callback redirect: {location}",
    );
    assert!(location.contains("error_description="));
    assert!(location.contains("upstream"));

    Ok(())
}
