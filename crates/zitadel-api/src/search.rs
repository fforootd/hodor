use crate::{ApiState, response};
use axum::{
    Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<ApiState> {
    Router::new().route("/search", get(search))
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}
fn default_limit() -> i64 {
    20
}

#[derive(Serialize)]
struct SearchResult {
    resource_type: String,
    id: String,
    title: String,
    subtitle: String,
}

#[derive(Serialize)]
struct SearchResponse {
    results: Vec<SearchResult>,
    total: usize,
}

async fn search(State(s): State<ApiState>, Query(p): Query<SearchParams>) -> Response {
    let q = match p.q {
        Some(q) if !q.is_empty() => q,
        _ => return response::bad_request("q parameter required"),
    };
    let scoped = s.db.scoped_default();
    let pattern = format!("%{q}%");

    let mut results = Vec::new();

    // Search users.
    if let Ok(rows) = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, identifier, display_name FROM users WHERE instance_id = $1 AND (identifier LIKE $2 OR display_name LIKE $3) LIMIT $4",
    )
    .bind(scoped.instance_id())
    .bind(&pattern)
    .bind(&pattern)
    .bind(p.limit)
    .fetch_all(scoped.pool())
    .await
    {
        for r in rows {
            results.push(SearchResult {
                resource_type: "user".into(),
                id: r.0,
                title: r.2.clone(),
                subtitle: r.1,
            });
        }
    }

    // Search orgs.
    if let Ok(rows) = sqlx::query_as::<_, (String, String)>(
        "SELECT id, name FROM orgs WHERE instance_id = $1 AND name LIKE $2 LIMIT $3",
    )
    .bind(scoped.instance_id())
    .bind(&pattern)
    .bind(p.limit)
    .fetch_all(scoped.pool())
    .await
    {
        for r in rows {
            results.push(SearchResult {
                resource_type: "org".into(),
                id: r.0.clone(),
                title: r.1,
                subtitle: format!("Organization {}", r.0),
            });
        }
    }

    let total = results.len();
    response::json_ok(SearchResponse { results, total })
}
