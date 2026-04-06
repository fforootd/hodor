//! Integration tests verifying CRUD operations are properly isolated
//! between root and child instances.
//!
//! Tests exercise the full HTTP stack (router -> middleware -> handler -> DB)
//! with cloud mode enabled, host-based routing, and path-scoped instance access.

mod support;

use axum::http::StatusCode;
use serde_json::json;
use zitadel_db::DEFAULT_INSTANCE_ID;
use zitadel_testkit::{AuthActor, TestResponse};

use support::{
    CHILD_HOST, CHILD_INSTANCE_ID, CHILD_ORG_ID, ROOT_HOST, build_cloud_test_app,
    create_root_user_in_org, create_session_for_instance, delete_on_host, extract_ids,
    get_on_host, insert_child_instance, insert_oidc_auth_request, insert_user_with_password,
    patch_json_on_host, post_json_on_host, setup_child,
};

const SIBLING_INSTANCE_ID: &str = "child-b";
const SIBLING_HOST: &str = "child-b.example.com";
const SIBLING_ORG_ID: &str = "child-b-org";

fn cookie_pair(response: &TestResponse) -> String {
    response
        .set_cookie()
        .expect("login response should set a cookie")
        .split(';')
        .next()
        .expect("cookie header should contain a cookie pair")
        .to_string()
}

async fn create_named_resource_on_host(
    app: &zitadel_testkit::TestApp,
    base_path: &str,
    actor: AuthActor,
    host: &str,
    name: &str,
) -> anyhow::Result<String> {
    let payload = if base_path == "/v1/apps" {
        let client_id = name
            .to_lowercase()
            .chars()
            .map(|char| if char.is_ascii_alphanumeric() { char } else { '-' })
            .collect::<String>();
        json!({
            "name": name,
            "client_id": client_id,
            "app_type": "web",
        })
    } else {
        json!({ "name": name })
    };
    let created = post_json_on_host(app, base_path, actor, host, &payload).await?;
    assert_eq!(created.status, StatusCode::CREATED);
    Ok(created.json_value()["id"].as_str().unwrap().to_string())
}

async fn assert_named_resource_isolation(
    base_path: &str,
    root_name: &str,
    child_name: &str,
    sibling_name: &str,
    updated_name: &str,
) -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let instance_scoped_base = base_path.trim_start_matches("/v1");
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "root-admin").await?;
    let (_child_session, child_pat) =
        setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;
    let (_sibling_session, sibling_pat) =
        setup_child(&app, SIBLING_INSTANCE_ID, SIBLING_HOST, SIBLING_ORG_ID).await?;

    let root_id =
        create_named_resource_on_host(&app, base_path, admin_pat.actor(), ROOT_HOST, root_name)
            .await?;
    let child_id =
        create_named_resource_on_host(&app, base_path, child_pat.actor(), CHILD_HOST, child_name)
            .await?;
    let sibling_id = create_named_resource_on_host(
        &app,
        base_path,
        sibling_pat.actor(),
        SIBLING_HOST,
        sibling_name,
    )
    .await?;

    let root_list = get_on_host(&app, base_path, admin_pat.actor(), ROOT_HOST).await?;
    assert_eq!(root_list.status, StatusCode::OK);
    let root_ids = extract_ids(&root_list.json_value());
    assert!(root_ids.contains(&root_id));
    assert!(!root_ids.contains(&child_id));
    assert!(!root_ids.contains(&sibling_id));

    let child_list = get_on_host(&app, base_path, child_pat.actor(), CHILD_HOST).await?;
    assert_eq!(child_list.status, StatusCode::OK);
    let child_ids = extract_ids(&child_list.json_value());
    assert!(child_ids.contains(&child_id));
    assert!(!child_ids.contains(&root_id));
    assert!(!child_ids.contains(&sibling_id));

    let sibling_list = get_on_host(&app, base_path, sibling_pat.actor(), SIBLING_HOST).await?;
    assert_eq!(sibling_list.status, StatusCode::OK);
    let sibling_ids = extract_ids(&sibling_list.json_value());
    assert!(sibling_ids.contains(&sibling_id));
    assert!(!sibling_ids.contains(&root_id));
    assert!(!sibling_ids.contains(&child_id));

    let cross_root_get =
        get_on_host(&app, &format!("{base_path}/{child_id}"), admin_pat.actor(), ROOT_HOST).await?;
    assert_eq!(cross_root_get.status, StatusCode::NOT_FOUND);
    let cross_child_get =
        get_on_host(&app, &format!("{base_path}/{root_id}"), child_pat.actor(), CHILD_HOST).await?;
    assert_eq!(cross_child_get.status, StatusCode::NOT_FOUND);
    let cross_sibling_get = get_on_host(
        &app,
        &format!("{base_path}/{child_id}"),
        sibling_pat.actor(),
        SIBLING_HOST,
    )
    .await?;
    assert_eq!(cross_sibling_get.status, StatusCode::NOT_FOUND);

    let cross_root_update = patch_json_on_host(
        &app,
        &format!("{base_path}/{child_id}"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({ "name": "should-not-update" }),
    )
    .await?;
    assert_eq!(cross_root_update.status, StatusCode::NOT_FOUND);

    let cross_sibling_update = patch_json_on_host(
        &app,
        &format!("{base_path}/{child_id}"),
        sibling_pat.actor(),
        SIBLING_HOST,
        &json!({ "name": "should-not-update" }),
    )
    .await?;
    assert_eq!(cross_sibling_update.status, StatusCode::NOT_FOUND);

    let cross_root_delete =
        delete_on_host(&app, &format!("{base_path}/{child_id}"), admin_pat.actor(), ROOT_HOST)
            .await?;
    assert_eq!(cross_root_delete.status, StatusCode::NOT_FOUND);

    let cross_sibling_delete = delete_on_host(
        &app,
        &format!("{base_path}/{child_id}"),
        sibling_pat.actor(),
        SIBLING_HOST,
    )
    .await?;
    assert_eq!(cross_sibling_delete.status, StatusCode::NOT_FOUND);

    let path_scoped_get = get_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}{instance_scoped_base}/{child_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(path_scoped_get.status, StatusCode::OK);
    assert_eq!(path_scoped_get.json_value()["name"], child_name);

    let path_scoped_update = patch_json_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}{instance_scoped_base}/{child_id}"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({ "name": updated_name }),
    )
    .await?;
    assert_eq!(path_scoped_update.status, StatusCode::OK);
    assert!(
        path_scoped_update.json_value()["updated"] == serde_json::Value::Bool(true)
            || path_scoped_update.json_value()["name"]
                == serde_json::Value::String(updated_name.to_string()),
        "path-scoped update should confirm the mutation or return the updated resource",
    );

    let reloaded_child =
        get_on_host(&app, &format!("{base_path}/{child_id}"), child_pat.actor(), CHILD_HOST).await?;
    assert_eq!(reloaded_child.status, StatusCode::OK);
    assert_eq!(reloaded_child.json_value()["name"], updated_name);

    let path_scoped_delete = delete_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}{instance_scoped_base}/{child_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(path_scoped_delete.status, StatusCode::NO_CONTENT);

    let child_missing =
        get_on_host(&app, &format!("{base_path}/{child_id}"), child_pat.actor(), CHILD_HOST).await?;
    assert_eq!(child_missing.status, StatusCode::NOT_FOUND);

    let child_list_after = get_on_host(&app, base_path, child_pat.actor(), CHILD_HOST).await?;
    let child_ids_after = extract_ids(&child_list_after.json_value());
    assert!(!child_ids_after.contains(&child_id));

    let root_list_after = get_on_host(&app, base_path, admin_pat.actor(), ROOT_HOST).await?;
    let root_ids_after = extract_ids(&root_list_after.json_value());
    assert!(root_ids_after.contains(&root_id));
    assert!(!root_ids_after.contains(&child_id));
    assert!(!root_ids_after.contains(&sibling_id));

    let sibling_still_visible =
        get_on_host(&app, &format!("{base_path}/{sibling_id}"), sibling_pat.actor(), SIBLING_HOST)
            .await?;
    assert_eq!(sibling_still_visible.status, StatusCode::OK);

    Ok(())
}

#[tokio::test]
async fn users_crud_isolated_between_root_child_and_sibling_instances() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "root-admin").await?;
    let (_child_session, child_pat) =
        setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;
    let (_sibling_session, sibling_pat) =
        setup_child(&app, SIBLING_INSTANCE_ID, SIBLING_HOST, SIBLING_ORG_ID).await?;

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

    let sibling_created = post_json_on_host(
        &app,
        "/v1/users",
        sibling_pat.actor(),
        SIBLING_HOST,
        &json!({ "identifier": "sibling-user@example.com", "display_name": "Sibling User" }),
    )
    .await?;
    assert_eq!(sibling_created.status, StatusCode::CREATED);
    let sibling_user_id = sibling_created.json_value()["id"].as_str().unwrap().to_string();

    let root_list = get_on_host(&app, "/v1/users", admin_pat.actor(), ROOT_HOST).await?;
    let root_ids = extract_ids(&root_list.json_value());
    assert!(root_ids.contains(&root_user_id));
    assert!(!root_ids.contains(&child_user_id));
    assert!(!root_ids.contains(&sibling_user_id));

    let child_list = get_on_host(&app, "/v1/users", child_pat.actor(), CHILD_HOST).await?;
    let child_ids = extract_ids(&child_list.json_value());
    assert!(child_ids.contains(&child_user_id));
    assert!(!child_ids.contains(&root_user_id));
    assert!(!child_ids.contains(&sibling_user_id));

    let sibling_list = get_on_host(&app, "/v1/users", sibling_pat.actor(), SIBLING_HOST).await?;
    let sibling_ids = extract_ids(&sibling_list.json_value());
    assert!(sibling_ids.contains(&sibling_user_id));
    assert!(!sibling_ids.contains(&root_user_id));
    assert!(!sibling_ids.contains(&child_user_id));

    let cross_root_get =
        get_on_host(&app, &format!("/v1/users/{child_user_id}"), admin_pat.actor(), ROOT_HOST)
            .await?;
    assert_eq!(cross_root_get.status, StatusCode::NOT_FOUND);

    let cross_child_get =
        get_on_host(&app, &format!("/v1/users/{root_user_id}"), child_pat.actor(), CHILD_HOST)
            .await?;
    assert_eq!(cross_child_get.status, StatusCode::NOT_FOUND);

    let cross_sibling_get = get_on_host(
        &app,
        &format!("/v1/users/{child_user_id}"),
        sibling_pat.actor(),
        SIBLING_HOST,
    )
    .await?;
    assert_eq!(cross_sibling_get.status, StatusCode::NOT_FOUND);

    let cross_root_update = patch_json_on_host(
        &app,
        &format!("/v1/users/{child_user_id}"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({ "display_name": "Should not update" }),
    )
    .await?;
    assert_eq!(cross_root_update.status, StatusCode::NOT_FOUND);

    let cross_sibling_update = patch_json_on_host(
        &app,
        &format!("/v1/users/{child_user_id}"),
        sibling_pat.actor(),
        SIBLING_HOST,
        &json!({ "display_name": "Should not update" }),
    )
    .await?;
    assert_eq!(cross_sibling_update.status, StatusCode::NOT_FOUND);

    let cross_root_delete =
        delete_on_host(&app, &format!("/v1/users/{child_user_id}"), admin_pat.actor(), ROOT_HOST)
            .await?;
    assert_eq!(cross_root_delete.status, StatusCode::NOT_FOUND);

    let cross_sibling_delete = delete_on_host(
        &app,
        &format!("/v1/users/{child_user_id}"),
        sibling_pat.actor(),
        SIBLING_HOST,
    )
    .await?;
    assert_eq!(cross_sibling_delete.status, StatusCode::NOT_FOUND);

    let path_scoped_get = get_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/users/{child_user_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(path_scoped_get.status, StatusCode::OK);
    assert_eq!(path_scoped_get.json_value()["identifier"], "child-user@example.com");

    let path_scoped_update = patch_json_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/users/{child_user_id}"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({ "display_name": "Updated Path User" }),
    )
    .await?;
    assert_eq!(path_scoped_update.status, StatusCode::OK);
    assert_eq!(path_scoped_update.json_value()["display_name"], "Updated Path User");

    let updated_child =
        get_on_host(&app, &format!("/v1/users/{child_user_id}"), child_pat.actor(), CHILD_HOST)
            .await?;
    assert_eq!(updated_child.status, StatusCode::OK);
    assert_eq!(updated_child.json_value()["display_name"], "Updated Path User");

    let path_scoped_delete = delete_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/users/{child_user_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(path_scoped_delete.status, StatusCode::NO_CONTENT);

    let deleted_child =
        get_on_host(&app, &format!("/v1/users/{child_user_id}"), child_pat.actor(), CHILD_HOST)
            .await?;
    assert_eq!(deleted_child.status, StatusCode::NOT_FOUND);

    let child_list_after = get_on_host(&app, "/v1/users", child_pat.actor(), CHILD_HOST).await?;
    assert!(!extract_ids(&child_list_after.json_value()).contains(&child_user_id));

    let root_list_after = get_on_host(&app, "/v1/users", admin_pat.actor(), ROOT_HOST).await?;
    let root_ids_after = extract_ids(&root_list_after.json_value());
    assert!(root_ids_after.contains(&root_user_id));
    assert!(!root_ids_after.contains(&child_user_id));
    assert!(!root_ids_after.contains(&sibling_user_id));

    Ok(())
}

#[tokio::test]
async fn path_scoped_child_routes_require_child_instance_access() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let outsider =
        create_root_user_in_org(&app, "org-outsider", "org-outsider", "outsider@example.com")
            .await?;
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

#[tokio::test]
async fn orgs_crud_isolated_between_instances() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "root-admin").await?;
    let (_child_session, child_pat) =
        setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;

    let root_org = post_json_on_host(
        &app,
        "/v1/orgs",
        admin_pat.actor(),
        ROOT_HOST,
        &json!({ "name": "Root Org" }),
    )
    .await?;
    assert_eq!(root_org.status, StatusCode::CREATED);
    let root_org_id = root_org.json_value()["id"].as_str().unwrap().to_string();

    let child_org = post_json_on_host(
        &app,
        "/v1/orgs",
        child_pat.actor(),
        CHILD_HOST,
        &json!({ "name": "Child Org Two" }),
    )
    .await?;
    assert_eq!(child_org.status, StatusCode::CREATED);
    let child_org_id = child_org.json_value()["id"].as_str().unwrap().to_string();

    let root_list = get_on_host(&app, "/v1/orgs", admin_pat.actor(), ROOT_HOST).await?;
    let root_ids = extract_ids(&root_list.json_value());
    assert!(root_ids.contains(&root_org_id));
    assert!(!root_ids.contains(&child_org_id));

    let child_list = get_on_host(&app, "/v1/orgs", child_pat.actor(), CHILD_HOST).await?;
    let child_ids = extract_ids(&child_list.json_value());
    assert!(child_ids.contains(&child_org_id));
    assert!(!child_ids.contains(&root_org_id));

    let cross = get_on_host(
        &app,
        &format!("/v1/orgs/{root_org_id}"),
        child_pat.actor(),
        CHILD_HOST,
    )
    .await?;
    assert_eq!(cross.status, StatusCode::NOT_FOUND);

    let path_scoped = get_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/orgs/{child_org_id}"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(path_scoped.status, StatusCode::OK);
    assert_eq!(path_scoped.json_value()["name"], "Child Org Two");

    Ok(())
}

#[tokio::test]
async fn groups_crud_isolated_between_root_child_and_sibling_instances()
-> anyhow::Result<()> {
    assert_named_resource_isolation(
        "/v1/groups",
        "Root Engineers",
        "Child Engineers",
        "Sibling Engineers",
        "Child Engineers Renamed",
    )
    .await
}

#[tokio::test]
async fn projects_crud_isolated_between_root_child_and_sibling_instances()
-> anyhow::Result<()> {
    assert_named_resource_isolation(
        "/v1/projects",
        "Root Project",
        "Child Project",
        "Sibling Project",
        "Child Project Renamed",
    )
    .await
}

#[tokio::test]
async fn apps_crud_isolated_between_root_child_and_sibling_instances() -> anyhow::Result<()> {
    assert_named_resource_isolation(
        "/v1/apps",
        "Root Application",
        "Child Application",
        "Sibling Application",
        "Child Application Renamed",
    )
    .await
}

#[tokio::test]
async fn instance_management_blocks_child_and_outsider_contexts() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let owner =
        create_root_user_in_org(&app, "org-owner", "Owner Org", "owner@example.com").await?;
    let owner_session = create_session_for_instance(&app, DEFAULT_INSTANCE_ID, &owner).await?;
    let outsider = create_root_user_in_org(
        &app,
        "org-outsider",
        "Outsider Org",
        "instance-outsider@example.com",
    )
    .await?;
    let outsider_session = create_session_for_instance(&app, DEFAULT_INSTANCE_ID, &outsider).await?;

    insert_child_instance(
        &app,
        "managed-child",
        &owner.org_id,
        "managed-child.example.com",
    )
    .await?;
    let (child_session, _child_pat) =
        setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;

    let owner_visible = get_on_host(
        &app,
        "/v1/instances/managed-child",
        owner_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(owner_visible.status, StatusCode::OK);

    let outsider_get = get_on_host(
        &app,
        "/v1/instances/managed-child",
        outsider_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(outsider_get.status, StatusCode::NOT_FOUND);

    let outsider_update = patch_json_on_host(
        &app,
        "/v1/instances/managed-child",
        outsider_session.bearer_actor(),
        ROOT_HOST,
        &json!({ "placement_mode": "regional", "region_key": "europe-west1" }),
    )
    .await?;
    assert_eq!(outsider_update.status, StatusCode::NOT_FOUND);

    let outsider_delete = delete_on_host(
        &app,
        "/v1/instances/managed-child",
        outsider_session.bearer_actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(outsider_delete.status, StatusCode::NOT_FOUND);

    let child_get = get_on_host(
        &app,
        "/v1/instances/managed-child",
        child_session.bearer_actor(),
        CHILD_HOST,
    )
    .await?;
    assert_eq!(child_get.status, StatusCode::FORBIDDEN);
    assert_eq!(
        child_get.json_value(),
        json!({"error": "instance management is only available from a parent instance", "code": 403})
    );

    let child_update = patch_json_on_host(
        &app,
        "/v1/instances/managed-child",
        child_session.bearer_actor(),
        CHILD_HOST,
        &json!({ "placement_mode": "regional", "region_key": "europe-west1" }),
    )
    .await?;
    assert_eq!(child_update.status, StatusCode::FORBIDDEN);

    let child_delete = delete_on_host(
        &app,
        "/v1/instances/managed-child",
        child_session.bearer_actor(),
        CHILD_HOST,
    )
    .await?;
    assert_eq!(child_delete.status, StatusCode::FORBIDDEN);

    Ok(())
}

#[tokio::test]
async fn login_is_host_scoped_when_identifiers_overlap_between_instances()
-> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;

    let root_org_id = app.ctx.db.default_org_id().await?;
    let root_user = insert_user_with_password(
        &app,
        DEFAULT_INSTANCE_ID,
        &root_org_id,
        "shared-login@example.com",
        "Root Shared User",
        "root-password",
    )
    .await?;
    let child_user = insert_user_with_password(
        &app,
        CHILD_INSTANCE_ID,
        CHILD_ORG_ID,
        "shared-login@example.com",
        "Child Shared User",
        "child-password",
    )
    .await?;

    let root_flow = post_json_on_host(
        &app,
        "/v1/login/flows",
        AuthActor::Anonymous,
        ROOT_HOST,
        &json!({}),
    )
    .await?;
    assert_eq!(root_flow.status, StatusCode::CREATED);
    assert_eq!(root_flow.json_value()["step"], "identifier");
    let root_flow_id = root_flow.json_value()["flow_id"].as_str().unwrap().to_string();

    let root_identifier = post_json_on_host(
        &app,
        &format!("/v1/login/flows/{root_flow_id}/submit"),
        AuthActor::Anonymous,
        ROOT_HOST,
        &json!({ "action": "identifier", "identifier": "shared-login@example.com" }),
    )
    .await?;
    assert_eq!(root_identifier.status, StatusCode::OK);
    assert_eq!(root_identifier.json_value()["step"], "password");

    let root_wrong_password = post_json_on_host(
        &app,
        &format!("/v1/login/flows/{root_flow_id}/submit"),
        AuthActor::Anonymous,
        ROOT_HOST,
        &json!({ "action": "password", "password": "child-password" }),
    )
    .await?;
    assert_eq!(root_wrong_password.status, StatusCode::OK);
    assert_eq!(root_wrong_password.json_value()["step"], "password");

    let root_completed = post_json_on_host(
        &app,
        &format!("/v1/login/flows/{root_flow_id}/submit"),
        AuthActor::Anonymous,
        ROOT_HOST,
        &json!({ "action": "password", "password": "root-password" }),
    )
    .await?;
    assert_eq!(root_completed.status, StatusCode::OK);
    assert_eq!(root_completed.json_value()["step"], "complete");
    let root_cookie = AuthActor::cookie(cookie_pair(&root_completed));

    let root_whoami = get_on_host(&app, "/v1/auth/whoami", root_cookie.clone(), ROOT_HOST).await?;
    assert_eq!(root_whoami.status, StatusCode::OK);
    assert_eq!(root_whoami.json_value()["user_id"], root_user.user_id);

    let root_cookie_on_child =
        get_on_host(&app, "/v1/auth/whoami", root_cookie.clone(), CHILD_HOST).await?;
    assert_eq!(root_cookie_on_child.status, StatusCode::UNAUTHORIZED);

    let child_flow_from_root_cookie = post_json_on_host(
        &app,
        "/v1/login/flows",
        root_cookie,
        CHILD_HOST,
        &json!({}),
    )
    .await?;
    assert_eq!(child_flow_from_root_cookie.status, StatusCode::CREATED);
    assert_eq!(child_flow_from_root_cookie.json_value()["step"], "identifier");

    let child_flow = post_json_on_host(
        &app,
        "/v1/login/flows",
        AuthActor::Anonymous,
        CHILD_HOST,
        &json!({}),
    )
    .await?;
    assert_eq!(child_flow.status, StatusCode::CREATED);
    assert_eq!(child_flow.json_value()["step"], "identifier");
    let child_flow_id = child_flow.json_value()["flow_id"].as_str().unwrap().to_string();

    let child_identifier = post_json_on_host(
        &app,
        &format!("/v1/login/flows/{child_flow_id}/submit"),
        AuthActor::Anonymous,
        CHILD_HOST,
        &json!({ "action": "identifier", "identifier": "shared-login@example.com" }),
    )
    .await?;
    assert_eq!(child_identifier.status, StatusCode::OK);
    assert_eq!(child_identifier.json_value()["step"], "password");

    let child_wrong_password = post_json_on_host(
        &app,
        &format!("/v1/login/flows/{child_flow_id}/submit"),
        AuthActor::Anonymous,
        CHILD_HOST,
        &json!({ "action": "password", "password": "root-password" }),
    )
    .await?;
    assert_eq!(child_wrong_password.status, StatusCode::OK);
    assert_eq!(child_wrong_password.json_value()["step"], "password");

    let child_completed = post_json_on_host(
        &app,
        &format!("/v1/login/flows/{child_flow_id}/submit"),
        AuthActor::Anonymous,
        CHILD_HOST,
        &json!({ "action": "password", "password": "child-password" }),
    )
    .await?;
    assert_eq!(child_completed.status, StatusCode::OK);
    assert_eq!(child_completed.json_value()["step"], "complete");
    let child_cookie = AuthActor::cookie(cookie_pair(&child_completed));

    let child_whoami =
        get_on_host(&app, "/v1/auth/whoami", child_cookie.clone(), CHILD_HOST).await?;
    assert_eq!(child_whoami.status, StatusCode::OK);
    assert_eq!(child_whoami.json_value()["user_id"], child_user.user_id);

    let child_cookie_on_root =
        get_on_host(&app, "/v1/auth/whoami", child_cookie.clone(), ROOT_HOST).await?;
    assert_eq!(child_cookie_on_root.status, StatusCode::UNAUTHORIZED);

    let root_flow_from_child_cookie = post_json_on_host(
        &app,
        "/v1/login/flows",
        child_cookie,
        ROOT_HOST,
        &json!({}),
    )
    .await?;
    assert_eq!(root_flow_from_child_cookie.status, StatusCode::CREATED);
    assert_eq!(root_flow_from_child_cookie.json_value()["step"], "identifier");

    Ok(())
}

#[tokio::test]
async fn cross_instance_session_reuse_does_not_complete_root_oidc_auth_requests()
-> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let (child_session, _child_pat) =
        setup_child(&app, CHILD_INSTANCE_ID, CHILD_HOST, CHILD_ORG_ID).await?;

    insert_oidc_auth_request(
        &app,
        DEFAULT_INSTANCE_ID,
        "root-auth-request",
        "client-1",
        "https://rp.example/callback",
        "root-state",
        "[]",
    )
    .await?;

    let foreign_cookie_actor = app.ctx.cookie_actor_for_token(&child_session.token);

    let plain_flow = post_json_on_host(
        &app,
        "/v1/login/flows",
        foreign_cookie_actor.clone(),
        ROOT_HOST,
        &json!({}),
    )
    .await?;
    assert_eq!(plain_flow.status, StatusCode::CREATED);
    assert_eq!(plain_flow.json_value()["step"], "identifier");

    let oidc_flow = post_json_on_host(
        &app,
        "/v1/login/flows",
        foreign_cookie_actor.clone(),
        ROOT_HOST,
        &json!({ "auth_request_id": "root-auth-request" }),
    )
    .await?;
    assert_eq!(oidc_flow.status, StatusCode::CREATED);
    assert_eq!(oidc_flow.json_value()["step"], "identifier");
    let oidc_flow_id = oidc_flow.json_value()["flow_id"].as_str().unwrap().to_string();

    let rejected_reuse = post_json_on_host(
        &app,
        &format!("/v1/login/flows/{oidc_flow_id}/submit"),
        foreign_cookie_actor,
        ROOT_HOST,
        &json!({ "action": "use_session" }),
    )
    .await?;
    assert_eq!(rejected_reuse.status, StatusCode::OK);
    assert_eq!(rejected_reuse.json_value()["step"], "identifier");
    assert!(rejected_reuse.json_value()["redirect_uri"].is_null());

    let scoped = app.ctx.db.scoped_default();
    let auth_request_row: (String, String, String, i64) = sqlx::query_as(&format!(
        "SELECT COALESCE(user_id, ''), COALESCE(session_id, ''), COALESCE(code, ''), {} \
         FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2",
        scoped.bool_as_int("done"),
    ))
    .bind(scoped.instance_id())
    .bind("root-auth-request")
    .fetch_one(scoped.pool())
    .await?;
    assert_eq!(auth_request_row.0, "");
    assert_eq!(auth_request_row.1, "");
    assert_eq!(auth_request_row.2, "");
    assert_eq!(auth_request_row.3, 0);

    Ok(())
}
