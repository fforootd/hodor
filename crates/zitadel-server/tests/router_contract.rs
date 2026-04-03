use std::{
    collections::HashMap,
    sync::{Arc, atomic::AtomicBool},
};

use axum::{Router, http::HeaderMap};
use serde_json::json;
use zitadel_db::DEFAULT_INSTANCE_ID;
use zitadel_server::{AppState, build_router};
use zitadel_testkit::{AuthActor, TestApp, TestContext};

async fn build_test_app() -> anyhow::Result<TestApp> {
    let ctx = TestContext::new().await?;
    let app_state = Arc::new(AppState {
        config: ctx.config.clone(),
        db: ctx.db.db.clone(),
        secret_box: Arc::new(zitadel_crypto::SecretBox::new("", &HashMap::new())?),
        ready: AtomicBool::new(true),
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
async fn public_surfaces_bypass_auth_and_readyz_reflects_state() -> anyhow::Result<()> {
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

    Ok(())
}

#[tokio::test]
async fn protected_routes_follow_the_current_actor_contract() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let user = app
        .ctx
        .create_user("route-user@example.com", "password123")
        .await?;
    let user_session = app.ctx.create_session(&user).await?;
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
        assert_eq!(
            user_response.status,
            axum::http::StatusCode::FORBIDDEN,
            "{path} should be PAT-only",
        );
        assert_eq!(
            user_response.json_value(),
            json!({"error": "personal access token required", "code": 403}),
            "{path} should clearly report the PAT-only boundary",
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
    assert_eq!(oidc_response.status, axum::http::StatusCode::OK);
    assert_eq!(oidc_response.json_value()["token_type"], "oidc");

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

    let custom_model = json!({
        "schema_version": "1.1",
        "type_definitions": [
            {
                "type": "user",
                "relations": {},
                "metadata": { "relations": {} }
            },
            {
                "type": "instance",
                "relations": {
                    "owner": { "this": {} },
                    "admin": { "this": {} },
                    "viewer": { "this": {} },
                    "parent": { "this": {} }
                },
                "metadata": {
                    "relations": {
                        "owner": { "directly_related_user_types": [{ "type": "user" }] },
                        "admin": { "directly_related_user_types": [{ "type": "user" }] },
                        "viewer": { "directly_related_user_types": [{ "type": "user" }] },
                        "parent": { "directly_related_user_types": [{ "type": "user" }] }
                    }
                }
            },
            {
                "type": "org",
                "relations": {
                    "owner": { "this": {} },
                    "admin": { "this": {} },
                    "member": { "this": {} },
                    "viewer": { "this": {} }
                },
                "metadata": {
                    "relations": {
                        "owner": { "directly_related_user_types": [{ "type": "user" }] },
                        "admin": { "directly_related_user_types": [{ "type": "user" }] },
                        "member": { "directly_related_user_types": [{ "type": "user" }] },
                        "viewer": { "directly_related_user_types": [{ "type": "user" }] }
                    }
                }
            },
            {
                "type": "group",
                "relations": {
                    "member": { "this": {} },
                    "admin": { "this": {} }
                },
                "metadata": {
                    "relations": {
                        "member": { "directly_related_user_types": [{ "type": "user" }] },
                        "admin": { "directly_related_user_types": [{ "type": "user" }] }
                    }
                }
            },
            {
                "type": "project",
                "relations": {
                    "owner": { "this": {} },
                    "admin": { "this": {} },
                    "member": { "this": {} }
                },
                "metadata": {
                    "relations": {
                        "owner": { "directly_related_user_types": [{ "type": "user" }] },
                        "admin": { "directly_related_user_types": [{ "type": "user" }] },
                        "member": { "directly_related_user_types": [{ "type": "user" }] }
                    }
                }
            },
            {
                "type": "app",
                "relations": {
                    "admin": { "this": {} },
                    "viewer": { "this": {} }
                },
                "metadata": {
                    "relations": {
                        "admin": { "directly_related_user_types": [{ "type": "user" }] },
                        "viewer": { "directly_related_user_types": [{ "type": "user" }] }
                    }
                }
            },
            {
                "type": "settings",
                "relations": {
                    "admin": { "this": {} },
                    "viewer": { "this": {} }
                },
                "metadata": {
                    "relations": {
                        "admin": { "directly_related_user_types": [{ "type": "user" }] },
                        "viewer": { "directly_related_user_types": [{ "type": "user" }] }
                    }
                }
            },
            {
                "type": "session",
                "relations": {
                    "owner": { "this": {} }
                },
                "metadata": {
                    "relations": {
                        "owner": { "directly_related_user_types": [{ "type": "user" }] }
                    }
                }
            },
            {
                "type": "document",
                "relations": {
                    "viewer": { "this": {} }
                },
                "metadata": {
                    "relations": {
                        "viewer": { "directly_related_user_types": [{ "type": "user" }] }
                    }
                }
            }
        ],
        "conditions": {}
    });

    let written_model = app
        .post_json(
            &format!("/v1/fga/stores/{store_id}/authorization-models"),
            admin_pat.actor(),
            &custom_model,
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
        bad_request.json_value(),
        json!({"error": "identifier is required", "code": 400})
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
                "state": "inactive",
            }),
        )
        .await?;
    assert_eq!(updated.status, axum::http::StatusCode::OK);
    let updated_json = updated.json_value();
    assert_eq!(updated_json["display_name"], "CRUD User Updated");
    assert_eq!(updated_json["state"], "inactive");

    let list = app.get("/v1/users", admin_pat.actor()).await?;
    assert_eq!(list.status, axum::http::StatusCode::OK);
    let list_json = list.json_value();
    assert!(
        list_json["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["id"] == user_id)),
        "created user should be returned by the list endpoint",
    );

    Ok(())
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
