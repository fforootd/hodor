mod support;

use anyhow::Context;
use axum::{body::Body, http::Method};
use serde_json::json;
use zitadel_db::DEFAULT_INSTANCE_ID;

use support::{
    ROOT_HOST, build_cloud_test_app, create_root_user_in_org, create_session_for_instance,
    delete_on_host, get_on_host, grant_org_role, grant_org_role_in_instance, host_headers,
    insert_child_instance, insert_instance_trust_link, insert_instance_with_parent,
    patch_json_on_host, post_json_on_host, rebuild_platform_fga, setup_child,
};

async fn build_test_app() -> anyhow::Result<zitadel_testkit::TestApp> {
    build_cloud_test_app().await
}

async fn create_child_user_session(
    app: &zitadel_testkit::TestApp,
    instance_id: &str,
    org_id: &str,
    identifier: &str,
) -> anyhow::Result<zitadel_testkit::SessionFixture> {
    let user =
        support::insert_user_with_password(app, instance_id, org_id, identifier, identifier, "password123")
            .await?;
    create_session_for_instance(app, instance_id, &user).await
}

#[tokio::test]
async fn root_users_are_owner_scoped_and_operators_are_unscoped() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let owner_one = app
        .ctx
        .create_user("owner-one@example.com", "password123")
        .await?;
    grant_org_role(&app, &owner_one.org_id, &owner_one.user_id, "owner").await?;
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
    assert_eq!(owner_admin_list.status, axum::http::StatusCode::NOT_FOUND);

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
    assert_eq!(operator_admin_list.status, axum::http::StatusCode::NOT_FOUND);

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
    grant_org_role(&app, &owner.org_id, &owner.user_id, "owner").await?;
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

    insert_child_instance(
        &app,
        "child-bootstrap",
        &owner.org_id,
        "child-bootstrap.example.com",
    )
    .await?;

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

    let child_session = create_child_user_session(
        &app,
        "child-bootstrap",
        "child-org",
        "child-user@example.com",
    )
    .await?;

    let child_bootstrap = app
        .request(
            Method::GET,
            "/v1/console/bootstrap",
            child_session.bearer_actor(),
            host_headers("child-bootstrap.example.com"),
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
            host_headers("child-bootstrap.example.com"),
            Body::empty(),
        )
        .await?;
    assert_eq!(child_instances.status, axum::http::StatusCode::FORBIDDEN);
    assert_eq!(
        child_instances.json_value(),
        json!({"error": "instance management is only available from a parent instance", "code": 403})
    );

    Ok(())
}

#[tokio::test]
async fn portal_instances_can_manage_their_own_children_when_instance_management_is_enabled()
-> anyhow::Result<()> {
    let app = build_test_app().await?;
    let portal_org = "portal-org";
    let portal_host = "portal.example.com";
    let portal_instance = "portal-inst";
    let tenant_instance = "portal-child";

    let scoped = app.ctx.db.scoped_default();
    sqlx::query("INSERT INTO orgs (instance_id, id, name, state) VALUES ($1, $2, $3, 'active')")
        .bind(DEFAULT_INSTANCE_ID)
        .bind(portal_org)
        .bind("Portal Org")
        .execute(scoped.pool())
        .await
        .context("insert portal org")?;

    insert_instance_with_parent(
        &app,
        portal_instance,
        DEFAULT_INSTANCE_ID,
        portal_org,
        portal_host,
        "managed",
        r#"{"instance_management":true,"billing":true}"#,
    )
    .await?;

    let portal_user = support::insert_user_with_password(
        &app,
        portal_instance,
        portal_org,
        "portal-owner@example.com",
        "portal-owner@example.com",
        "password123",
    )
    .await?;
    grant_org_role_in_instance(&app, portal_instance, portal_org, &portal_user.user_id, "owner")
        .await?;
    insert_instance_with_parent(
        &app,
        tenant_instance,
        portal_instance,
        portal_org,
        "portal-child.example.com",
        "managed",
        "{}",
    )
    .await?;
    let portal_session = create_session_for_instance(&app, portal_instance, &portal_user).await?;

    let bootstrap = get_on_host(
        &app,
        "/v1/console/bootstrap",
        portal_session.bearer_actor(),
        portal_host,
    )
    .await?;
    assert_eq!(bootstrap.status, axum::http::StatusCode::OK);
    assert_eq!(
        bootstrap.json_value()["capabilities"]["instance_management"],
        true
    );

    let list = get_on_host(
        &app,
        "/v1/instances",
        portal_session.bearer_actor(),
        portal_host,
    )
    .await?;
    assert_eq!(list.status, axum::http::StatusCode::OK);
    assert_eq!(list.json_value()["items"].as_array().unwrap().len(), 1);
    assert_eq!(list.json_value()["items"][0]["instance_id"], tenant_instance);

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
    rebuild_platform_fga(&app.ctx.api_state)
        .await
        .context("rebuild platform store after moving root user org")?;

    let after_move = get_on_host(
        &app,
        "/v1/instances",
        root_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(after_move.status, axum::http::StatusCode::OK);
    assert_eq!(after_move.json_value()["items"].as_array().unwrap().len(), 1);

    zitadel_db::remove_membership(
        &app.ctx.db.db,
        DEFAULT_INSTANCE_ID,
        "org",
        "org-a",
        &root_user.user_id,
    )
    .await
    .context("remove org-a owner membership")?;
    rebuild_platform_fga(&app.ctx.api_state)
        .await
        .context("rebuild platform store after membership removal")?;

    let after_membership_change = get_on_host(
        &app,
        "/v1/instances",
        root_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(after_membership_change.status, axum::http::StatusCode::OK);
    assert_eq!(after_membership_change.json_value()["items"], json!([]));

    Ok(())
}

#[tokio::test]
async fn instance_pagination_skips_hidden_rows_without_losing_visible_instances() -> anyhow::Result<()>
{
    let app = build_test_app().await?;
    let owner = create_root_user_in_org(&app, "org-visible", "Visible Org", "pager@example.com")
        .await?;
    let owner_session = create_session_for_instance(&app, DEFAULT_INSTANCE_ID, &owner).await?;

    insert_child_instance(&app, "a-hidden", "1", "a-hidden.example.com").await?;
    insert_child_instance(&app, "b-visible", "org-visible", "b-visible.example.com").await?;
    insert_child_instance(&app, "c-hidden", "1", "c-hidden.example.com").await?;
    insert_child_instance(&app, "d-visible", "org-visible", "d-visible.example.com").await?;

    let first_page = get_on_host(
        &app,
        "/v1/instances?limit=1",
        owner_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(first_page.status, axum::http::StatusCode::OK);
    assert_eq!(first_page.json_value()["items"][0]["instance_id"], "b-visible");
    assert_eq!(first_page.json_value()["next_cursor"], "b-visible");

    let second_page = get_on_host(
        &app,
        "/v1/instances?limit=1&cursor=b-visible",
        owner_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(second_page.status, axum::http::StatusCode::OK);
    assert_eq!(second_page.json_value()["items"][0]["instance_id"], "d-visible");

    let third_page = get_on_host(
        &app,
        "/v1/instances?limit=1&cursor=d-visible",
        owner_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(third_page.status, axum::http::StatusCode::OK);
    assert_eq!(third_page.json_value()["items"], json!([]));
    assert_eq!(third_page.json_value()["next_cursor"], serde_json::Value::Null);

    Ok(())
}

#[tokio::test]
async fn instances_support_get_update_and_deprovision_through_management_routes()
-> anyhow::Result<()> {
    let app = build_test_app().await?;
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "instance-lifecycle-admin").await?;

    let created = post_json_on_host(
        &app,
        "/v1/instances",
        admin_pat.actor(),
        ROOT_HOST,
        &json!({
            "domain": "instance-lifecycle.example.com",
        }),
    )
    .await?;
    assert_eq!(created.status, axum::http::StatusCode::CREATED);
    let created_json = created.json_value();
    let instance_id = created_json["instance_id"]
        .as_str()
        .expect("created instance id should be present")
        .to_string();
    assert_eq!(created_json["primary_domain"], "instance-lifecycle.example.com");
    assert_eq!(created_json["state"], "active");

    let loaded = get_on_host(
        &app,
        &format!("/v1/instances/{instance_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(loaded.status, axum::http::StatusCode::OK);
    assert_eq!(loaded.json_value()["instance_id"], instance_id);

    let listed = get_on_host(&app, "/v1/instances", admin_pat.actor(), ROOT_HOST).await?;
    assert_eq!(listed.status, axum::http::StatusCode::OK);
    assert!(
        listed.json_value()["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["instance_id"] == instance_id)),
        "created instance should appear in the management list",
    );

    let updated = patch_json_on_host(
        &app,
        &format!("/v1/instances/{instance_id}"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({
            "placement_mode": "regional",
            "region_key": "europe-west1",
            "feature_overrides": {
                "custom_domains": true,
            },
        }),
    )
    .await?;
    assert_eq!(updated.status, axum::http::StatusCode::OK);
    assert_eq!(updated.json_value()["placement_mode"], "regional");
    assert_eq!(updated.json_value()["region_key"], "europe-west1");
    assert_eq!(
        updated.json_value()["feature_overrides"]["custom_domains"],
        serde_json::Value::Bool(true)
    );

    let deleted = delete_on_host(
        &app,
        &format!("/v1/instances/{instance_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(deleted.status, axum::http::StatusCode::NO_CONTENT);

    let after_delete = get_on_host(
        &app,
        &format!("/v1/instances/{instance_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(after_delete.status, axum::http::StatusCode::OK);
    assert_eq!(after_delete.json_value()["state"], "deprovisioning");

    let listed_after_delete =
        get_on_host(&app, "/v1/instances", admin_pat.actor(), ROOT_HOST).await?;
    assert_eq!(listed_after_delete.status, axum::http::StatusCode::OK);
    assert!(
        listed_after_delete.json_value()["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| {
                item["instance_id"] == instance_id
                    && item["state"] == serde_json::Value::String("deprovisioning".into())
            })),
        "deprovisioned instances should remain visible with their updated state",
    );

    Ok(())
}

#[tokio::test]
async fn managed_support_grants_enable_and_revoke_child_instance_access() -> anyhow::Result<()> {
    let app = build_test_app().await?;
    let operator = app.ctx.admin_user().await?;
    let operator_pat = app.ctx.create_pat(&operator, "support-grant-admin").await?;
    let support_user =
        create_root_user_in_org(&app, "support-org", "Support Org", "support@example.com").await?;
    let support_session = create_session_for_instance(&app, DEFAULT_INSTANCE_ID, &support_user).await?;

    let _ = setup_child(&app, "support-managed-child", "support-managed.example.com", "child-org").await?;

    let before = get_on_host(
        &app,
        "/v1/instances/support-managed-child/sessions",
        support_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(before.status, axum::http::StatusCode::NOT_FOUND);

    let created = post_json_on_host(
        &app,
        "/v1/support/grants",
        operator_pat.actor(),
        ROOT_HOST,
        &json!({
            "instance_id": "support-managed-child",
            "role": "SUPPORT_READ",
            "reason": "SUPPORT-123",
            "duration_secs": 3600,
            "principal_ref": format!("user:{}", support_user.user_id),
        }),
    )
    .await?;
    assert_eq!(created.status, axum::http::StatusCode::CREATED);
    assert_eq!(created.json_value()["source_kind"], "support_grant_managed");
    let grant_id = created.json_value()["grant_id"]
        .as_str()
        .expect("grant id")
        .to_string();

    let after = get_on_host(
        &app,
        "/v1/instances/support-managed-child/sessions",
        support_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(after.status, axum::http::StatusCode::OK);

    let revoked = delete_on_host(
        &app,
        &format!("/v1/support/grants/{grant_id}"),
        operator_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(revoked.status, axum::http::StatusCode::NO_CONTENT);

    let denied_again = get_on_host(
        &app,
        "/v1/instances/support-managed-child/sessions",
        support_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(denied_again.status, axum::http::StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn federated_support_grants_issue_tokens_that_respect_trust_and_revocation()
-> anyhow::Result<()> {
    let app = build_test_app().await?;
    let operator = app.ctx.admin_user().await?;
    let operator_pat = app.ctx.create_pat(&operator, "support-federated-admin").await?;
    let federated_instance = "support-federated-child";
    let federated_host = "support-federated.example.com";

    insert_instance_with_parent(
        &app,
        federated_instance,
        DEFAULT_INSTANCE_ID,
        &app.ctx.db.default_org_id().await?,
        federated_host,
        "federated",
        "{}",
    )
    .await?;

    let child_scoped = app.ctx.db.db.scoped(federated_instance.to_string());
    sqlx::query("INSERT INTO orgs (instance_id, id, name, state) VALUES ($1, $2, $3, 'active')")
        .bind(federated_instance)
        .bind("federated-org")
        .bind("Federated Org")
        .execute(child_scoped.pool())
        .await
        .context("insert federated org")?;
    let child_user = support::insert_user_with_password(
        &app,
        federated_instance,
        "federated-org",
        "federated-admin@example.com",
        "Federated Admin",
        "password123",
    )
    .await?;
    let _child_session = create_session_for_instance(&app, federated_instance, &child_user).await?;

    let issuer = app.ctx.api_state.oidc.provider.issuer().into_owned();
    let audience = format!("instance:{federated_instance}");
    insert_instance_trust_link(
        &app,
        federated_instance,
        &issuer,
        &audience,
        r#"["support_grant"]"#,
    )
    .await?;

    let created = post_json_on_host(
        &app,
        "/v1/support/grants",
        operator_pat.actor(),
        ROOT_HOST,
        &json!({
            "instance_id": federated_instance,
            "role": "SUPPORT_READ",
            "reason": "SUPPORT-456",
            "duration_secs": 3600
        }),
    )
    .await?;
    assert_eq!(created.status, axum::http::StatusCode::CREATED);
    assert_eq!(created.json_value()["source_kind"], "support_grant_federated");
    let grant_id = created.json_value()["grant_id"]
        .as_str()
        .expect("grant id")
        .to_string();
    let access_token = created.json_value()["access_token"]
        .as_str()
        .expect("federated access token")
        .to_string();

    let allowed = get_on_host(
        &app,
        "/v1/sessions",
        zitadel_testkit::AuthActor::bearer(access_token.as_str()),
        federated_host,
    )
    .await?;
    assert_eq!(allowed.status, axum::http::StatusCode::OK);

    let revoked = delete_on_host(
        &app,
        &format!("/v1/support/grants/{grant_id}"),
        operator_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(revoked.status, axum::http::StatusCode::NO_CONTENT);

    let denied = get_on_host(
        &app,
        "/v1/sessions",
        zitadel_testkit::AuthActor::bearer(access_token.as_str()),
        federated_host,
    )
    .await?;
    assert_eq!(denied.status, axum::http::StatusCode::UNAUTHORIZED);

    Ok(())
}
