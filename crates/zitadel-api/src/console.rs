use crate::{ApiState, response};
use axum::{Router, extract::State, response::Response, routing::get};

const META_SCHEMA: &str = include_str!("meta_schema.json");

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/console/bootstrap", get(bootstrap))
        .route("/counts", get(entity_counts))
}

/// Console bootstrap — initial config for the web console.
/// Returns meta (x-catalog, x-groups), entity counts, and orgs list
/// so the frontend can build the sidebar navigation.
async fn bootstrap(State(s): State<ApiState>) -> Response {
    let scoped = s.db.scoped_default();

    // Parse meta schema to extract x-catalog and x-groups for the frontend.
    let meta: serde_json::Value = serde_json::from_str(META_SCHEMA).unwrap_or_default();
    let meta_obj = serde_json::json!({
        "x-catalog": meta.get("x-catalog").cloned().unwrap_or(serde_json::Value::Object(Default::default())),
        "x-groups": meta.get("x-groups").cloned().unwrap_or(serde_json::Value::Object(Default::default())),
    });

    // Count entities for sidebar badges.
    let counts_queries: Vec<(&str, &str)> = vec![
        ("users", "SELECT COUNT(*) FROM users WHERE instance_id = $1"),
        ("org", "SELECT COUNT(*) FROM orgs WHERE instance_id = $1"),
        (
            "groups",
            "SELECT COUNT(*) FROM groups WHERE instance_id = $1",
        ),
        (
            "projects",
            "SELECT COUNT(*) FROM projects WHERE instance_id = $1",
        ),
        ("apps", "SELECT COUNT(*) FROM apps WHERE instance_id = $1"),
        (
            "providers",
            "SELECT COUNT(*) FROM providers WHERE instance_id = $1",
        ),
    ];
    let mut counts = serde_json::Map::new();
    for (name, query) in counts_queries {
        let count: i64 = sqlx::query_as::<_, (i64,)>(query)
            .bind(scoped.instance_id())
            .fetch_one(scoped.pool())
            .await
            .map(|r| r.0)
            .unwrap_or(0);
        counts.insert(name.to_string(), serde_json::Value::Number(count.into()));
    }

    // List orgs for the org switcher.
    let orgs: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, name, state FROM orgs WHERE instance_id = $1 ORDER BY name LIMIT 50",
    )
    .bind(scoped.instance_id())
    .fetch_all(scoped.pool())
    .await
    .unwrap_or_default();

    let org_items: Vec<serde_json::Value> = orgs
        .iter()
        .map(|(id, name, state)| {
            serde_json::json!({
                "id": id,
                "name": name,
                "display_name": name,
                "state": state,
            })
        })
        .collect();

    response::json_ok(serde_json::json!({
        "meta": meta_obj,
        "counts": serde_json::Value::Object(counts),
        "orgs": {
            "items": org_items,
        }
    }))
}

/// Entity counts for sidebar badges.
async fn entity_counts(State(s): State<ApiState>) -> Response {
    let scoped = s.db.scoped_default();
    let counts = vec![
        ("users", "SELECT COUNT(*) FROM users WHERE instance_id = $1"),
        ("orgs", "SELECT COUNT(*) FROM orgs WHERE instance_id = $1"),
        (
            "groups",
            "SELECT COUNT(*) FROM groups WHERE instance_id = $1",
        ),
        (
            "projects",
            "SELECT COUNT(*) FROM projects WHERE instance_id = $1",
        ),
        ("apps", "SELECT COUNT(*) FROM apps WHERE instance_id = $1"),
        (
            "providers",
            "SELECT COUNT(*) FROM providers WHERE instance_id = $1",
        ),
    ];
    let mut result = serde_json::Map::new();
    for (name, query) in counts {
        let count: i64 = sqlx::query_as::<_, (i64,)>(query)
            .bind(scoped.instance_id())
            .fetch_one(scoped.pool())
            .await
            .map(|r| r.0)
            .unwrap_or(0);
        result.insert(name.to_string(), serde_json::Value::Number(count.into()));
    }
    response::json_ok(serde_json::Value::Object(result))
}
