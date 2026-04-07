mod support;

use axum::http::StatusCode;
use serde_json::json;
use support::{
    CHILD_INSTANCE_ID, CHILD_ORG_ID, ROOT_HOST, build_cloud_test_app, get_on_host,
    post_json_on_host, setup_child,
};
use zitadel_db::DEFAULT_INSTANCE_ID;

async fn enable_custom_domains(
    app: &zitadel_testkit::TestApp,
    instance_id: &str,
) -> anyhow::Result<()> {
    let scoped = app.ctx.db.scoped_default();
    let sql = format!(
        "UPDATE instances SET feature_overrides = {} WHERE instance_id = $1",
        scoped.json_bind(2),
    );
    sqlx::query(&sql)
        .bind(instance_id)
        .bind(r#"{"custom_domains":true}"#)
        .execute(scoped.pool())
        .await?;
    Ok(())
}

#[tokio::test]
async fn custom_domain_routes_require_feature_flag() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "domains-feature-admin").await?;

    let response = get_on_host(&app, "/v1/domains", admin_pat.actor(), ROOT_HOST).await?;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
    assert!(
        response.json_value()["error"]
            .as_str()
            .is_some_and(|message| message.contains("custom domains not enabled")),
    );

    Ok(())
}

#[tokio::test]
async fn custom_domain_routes_keep_instance_and_org_scopes_separate() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    enable_custom_domains(&app, DEFAULT_INSTANCE_ID).await?;

    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "domains-scope-admin").await?;
    let org_id = app.ctx.db.default_org_id().await?;

    let instance_created = post_json_on_host(
        &app,
        "/v1/domains",
        admin_pat.actor(),
        ROOT_HOST,
        &json!({
            "domain": "portal.example.com",
            "purpose": "served",
        }),
    )
    .await?;
    assert_eq!(instance_created.status, StatusCode::CREATED);
    assert_eq!(
        instance_created.json_value()["org_id"],
        serde_json::Value::Null
    );

    let org_created = post_json_on_host(
        &app,
        &format!("/v1/orgs/{org_id}/domains"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({
            "domain": "org.example.com",
            "purpose": "allowed",
        }),
    )
    .await?;
    assert_eq!(org_created.status, StatusCode::CREATED);
    assert_eq!(
        org_created.json_value()["org_id"],
        serde_json::Value::String(org_id.clone())
    );

    let instance_list = get_on_host(&app, "/v1/domains", admin_pat.actor(), ROOT_HOST).await?;
    assert_eq!(instance_list.status, StatusCode::OK);
    let instance_items = instance_list.json_value()["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        instance_items
            .iter()
            .any(|item| item["domain"] == "portal.example.com")
    );
    assert!(
        !instance_items
            .iter()
            .any(|item| item["domain"] == "org.example.com")
    );

    let org_list = get_on_host(
        &app,
        &format!("/v1/orgs/{org_id}/domains"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(org_list.status, StatusCode::OK);
    let org_items = org_list.json_value()["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        org_items
            .iter()
            .any(|item| item["domain"] == "org.example.com")
    );
    assert!(
        !org_items
            .iter()
            .any(|item| item["domain"] == "portal.example.com")
    );

    let wrong_scope = get_on_host(
        &app,
        &format!("/v1/orgs/{org_id}/domains/portal.example.com"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(wrong_scope.status, StatusCode::NOT_FOUND);

    Ok(())
}

#[tokio::test]
async fn nested_instance_domain_routes_follow_the_same_hierarchy() -> anyhow::Result<()> {
    let app = build_cloud_test_app().await?;
    let _ = setup_child(
        &app,
        CHILD_INSTANCE_ID,
        "child.instance.example.com",
        CHILD_ORG_ID,
    )
    .await?;
    enable_custom_domains(&app, CHILD_INSTANCE_ID).await?;

    let admin = app.ctx.admin_user().await?;
    let admin_pat = app.ctx.create_pat(&admin, "nested-domains-admin").await?;

    let instance_created = post_json_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/domains"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({
            "domain": "api.child-custom.example.com",
            "purpose": "served",
        }),
    )
    .await?;
    assert_eq!(instance_created.status, StatusCode::CREATED);

    let org_created = post_json_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/orgs/{CHILD_ORG_ID}/domains"),
        admin_pat.actor(),
        ROOT_HOST,
        &json!({
            "domain": "org.child-custom.example.com",
            "purpose": "allowed",
        }),
    )
    .await?;
    assert_eq!(org_created.status, StatusCode::CREATED);

    let instance_list = get_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/domains"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(instance_list.status, StatusCode::OK);
    let instance_items = instance_list.json_value()["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        instance_items
            .iter()
            .any(|item| item["domain"] == "api.child-custom.example.com")
    );
    assert!(
        !instance_items
            .iter()
            .any(|item| item["domain"] == "org.child-custom.example.com")
    );

    let org_list = get_on_host(
        &app,
        &format!("/v1/instances/{CHILD_INSTANCE_ID}/orgs/{CHILD_ORG_ID}/domains"),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(org_list.status, StatusCode::OK);
    let org_items = org_list.json_value()["items"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(
        org_items
            .iter()
            .any(|item| item["domain"] == "org.child-custom.example.com")
    );
    assert!(
        !org_items
            .iter()
            .any(|item| item["domain"] == "api.child-custom.example.com")
    );

    let wrong_scope = get_on_host(
        &app,
        &format!(
            "/v1/instances/{CHILD_INSTANCE_ID}/orgs/{CHILD_ORG_ID}/domains/api.child-custom.example.com"
        ),
        admin_pat.actor(),
        ROOT_HOST,
    )
    .await?;
    assert_eq!(wrong_scope.status, StatusCode::NOT_FOUND);

    Ok(())
}
