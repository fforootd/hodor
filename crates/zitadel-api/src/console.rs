use crate::{ApiState, middleware::Identity, response};
use axum::{Extension, Router, extract::State, response::Response, routing::get};
use zitadel_app::{FeatureMap, feature_enabled, merge_feature_overrides};

const META_SCHEMA: &str = include_str!("meta_schema.json");
const ALLOWED_INSTANCE_FEATURES: &[&str] = &["instance_management", "billing"];

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/console/bootstrap", get(bootstrap))
        .route("/counts", get(entity_counts))
}

/// Console bootstrap — initial config for the web console.
/// Returns meta (x-catalog, x-groups), entity counts, and orgs list
/// so the frontend can build the sidebar navigation.
async fn bootstrap(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
) -> Response {
    // Parse meta schema to extract x-catalog and x-groups for the frontend.
    let meta: serde_json::Value = serde_json::from_str(META_SCHEMA).unwrap_or_default();
    let meta_obj = serde_json::json!({
        "x-catalog": meta.get("x-catalog").cloned().unwrap_or(serde_json::Value::Object(Default::default())),
        "x-groups": meta.get("x-groups").cloned().unwrap_or(serde_json::Value::Object(Default::default())),
    });

    // Count entities for sidebar badges.
    // Keys must match the x-catalog type names so the frontend's applyCounts()
    // can resolve them — aggregate parents (e.g. "users") sum their children
    // (human_user + service_user + ai_agent), so we return per-subtype counts.
    let ctx = response::build_actor_context(&identity);
    let bootstrap = match s
        .app
        .runner
        .run(&ctx, "console.bootstrap", || {
            s.app.load_console_bootstrap.execute(&ctx)
        })
        .await
    {
        Ok(data) => data,
        Err(error) => return response::app_error(error),
    };

    let counts = bootstrap
        .counts
        .iter()
        .map(|(name, count)| (name.clone(), serde_json::Value::Number((*count).into())))
        .collect::<serde_json::Map<String, serde_json::Value>>();

    let org_items: Vec<serde_json::Value> = bootstrap
        .orgs
        .iter()
        .map(|org| {
            serde_json::json!({
                "id": org.id,
                "name": org.name,
                "display_name": org.name,
                "state": org.state,
            })
        })
        .collect();

    // Instance info and feature flags for the frontend.
    let instance_id = bootstrap.instance.instance_id.clone();
    let inst_kind = bootstrap.instance.kind.clone();
    let feature_overrides_raw = bootstrap.instance.feature_overrides_json.clone();
    let parent_id = bootstrap.instance.parent_instance_id.clone();

    let feature_overrides: serde_json::Value =
        serde_json::from_str(&feature_overrides_raw).unwrap_or_default();

    let is_root = inst_kind == "root";
    let default_features = FeatureMap::from([
        ("instance_management".into(), is_root),
        ("billing".into(), is_root),
    ]);
    let merged_features = merge_feature_overrides(
        default_features.clone(),
        &feature_overrides,
        ALLOWED_INSTANCE_FEATURES,
    )
    .unwrap_or_else(|error| {
        tracing::warn!(error = %error, "invalid instance feature overrides; falling back to defaults");
        default_features.clone()
    });

    let mut features = serde_json::Map::new();
    for (key, enabled) in merged_features {
        features.insert(key, serde_json::Value::Bool(enabled));
    }

    let instance_management = feature_enabled(
        &default_features,
        &feature_overrides,
        ALLOWED_INSTANCE_FEATURES,
        "instance_management",
    )
    .unwrap_or(is_root)
        || feature_overrides
            .get("instance_management")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let billing = feature_enabled(
        &default_features,
        &feature_overrides,
        ALLOWED_INSTANCE_FEATURES,
        "billing",
    )
    .unwrap_or(is_root)
        || feature_overrides
            .get("billing")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
    let capabilities = serde_json::json!({
        "instance_management": instance_management,
        "operator_admin": is_root && identity.operator_admin,
        "billing": billing,
    });

    response::json_ok(serde_json::json!({
        "meta": meta_obj,
        "counts": serde_json::Value::Object(counts),
        "orgs": {
            "items": org_items,
        },
        "features": serde_json::Value::Object(features),
        "instance": {
            "id": instance_id,
            "kind": inst_kind,
            "is_root": is_root,
            "has_parent": parent_id.is_some(),
        },
        "capabilities": capabilities,
    }))
}

/// Entity counts for sidebar badges.
async fn entity_counts(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "console.entity_counts", || {
            s.app.load_entity_counts.execute(&ctx)
        })
        .await
    {
        Ok(counts) => {
            let result = counts
                .into_iter()
                .map(|(name, count)| (name, serde_json::Value::Number(count.into())))
                .collect::<serde_json::Map<String, serde_json::Value>>();
            response::json_ok(serde_json::Value::Object(result))
        }
        Err(error) => response::app_error(error),
    }
}
