use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_app::search::SearchEntitiesCommand;

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

async fn search(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(p): Query<SearchParams>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let q = match p.q {
        Some(q) if !q.is_empty() => q,
        _ => return response::bad_request("q parameter required"),
    };
    let cmd = SearchEntitiesCommand {
        query: q,
        limit: Some(p.limit as u32),
    };
    match s.app.runner.run_fn(&ctx, "search", || s.app.search_entities.execute(&ctx, cmd)).await {
        Ok(results) => {
            let total = results.len();
            let results = results
                .into_iter()
                .map(|record| SearchResult {
                    resource_type: record.resource_type,
                    id: record.id,
                    title: record.title,
                    subtitle: record.subtitle.unwrap_or_default(),
                })
                .collect();
            response::json_ok(SearchResponse { results, total })
        }
        Err(e) => response::app_error(e),
    }
}
