use axum::{Router, extract::State, response::Response, routing::get};
use crate::{ApiState, response};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/console/bootstrap", get(bootstrap))
        .route("/counts", get(entity_counts))
}

/// Console bootstrap — initial config for the web console.
async fn bootstrap(State(s): State<ApiState>) -> Response {
    let scoped = s.db.scoped_default();
    // Count entities for sidebar badges.
    let user_count: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM users WHERE instance_id = ?")
        .bind(scoped.instance_id()).fetch_one(scoped.pool()).await.map(|r| r.0).unwrap_or(0);
    let org_count: i64 = sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM orgs WHERE instance_id = ?")
        .bind(scoped.instance_id()).fetch_one(scoped.pool()).await.map(|r| r.0).unwrap_or(0);

    response::json_ok(serde_json::json!({
        "instance_id": scoped.instance_id(),
        "user_count": user_count,
        "org_count": org_count,
        "features": {
            "oidc": true,
            "saml": false,
            "passkeys": false,
            "actions": false,
        }
    }))
}

/// Entity counts for sidebar badges.
async fn entity_counts(State(s): State<ApiState>) -> Response {
    let scoped = s.db.scoped_default();
    let counts = vec![
        ("users", "SELECT COUNT(*) FROM users WHERE instance_id = ?"),
        ("orgs", "SELECT COUNT(*) FROM orgs WHERE instance_id = ?"),
        ("groups", "SELECT COUNT(*) FROM groups WHERE instance_id = ?"),
        ("projects", "SELECT COUNT(*) FROM projects WHERE instance_id = ?"),
        ("apps", "SELECT COUNT(*) FROM apps WHERE instance_id = ?"),
        ("providers", "SELECT COUNT(*) FROM providers WHERE instance_id = ?"),
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
