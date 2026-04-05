use crate::{ApiState, response};
use axum::{
    Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_db::{current_instance_id, search_records};

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
    match search_records(&s.db, current_instance_id().as_ref(), &q, p.limit).await {
        Ok(results) => {
            let total = results.len();
            let results = results
                .into_iter()
                .map(|record| SearchResult {
                    resource_type: record.resource_type,
                    id: record.id,
                    title: record.title,
                    subtitle: record.subtitle,
                })
                .collect();
            response::json_ok(SearchResponse { results, total })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}
