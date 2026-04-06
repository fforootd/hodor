use crate::{ApiState, middleware::Identity, response};
use axum::{Extension, Router, extract::State, response::Response, routing::get};
use serde::Deserialize;
use serde_json::json;

pub fn routes() -> Router<ApiState> {
    Router::new().route("/observability/overview", get(overview))
}

#[derive(Deserialize)]
struct OverviewParams {
    #[serde(default = "default_range")]
    range: String,
}

fn default_range() -> String {
    "12h".into()
}

async fn overview(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    axum::extract::Query(p): axum::extract::Query<OverviewParams>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    if let Err(e) = crate::fga_check(&s, &ctx, "viewer", "observability:*").await {
        return e;
    }

    let hours: u64 = match p.range.as_str() {
        "1h" => 1,
        "12h" => 12,
        "24h" => 24,
        "7d" => 24 * 7,
        "30d" => 24 * 30,
        _ => 12,
    };

    match s
        .app
        .repos
        .observability
        .load_overview(ctx.instance_id(), hours)
        .await
    {
        Ok(o) => {
            let breakdown = |items: &[(String, i64)]| -> Vec<serde_json::Value> {
                items
                    .iter()
                    .map(|(name, count)| json!({ "name": name, "count": count }))
                    .collect()
            };

            response::json_ok(json!({
                "metrics": {
                    "auth": { "current": o.auth_current, "previous": o.auth_previous, "timestamps": o.auth_timestamps },
                    "sessions": { "current": o.sessions_current, "previous": o.sessions_previous, "timestamps": o.session_timestamps },
                    "tokens": { "current": o.tokens_current, "previous": o.tokens_previous, "timestamps": o.token_timestamps },
                    "failed": { "current": o.failures_current, "previous": o.failures_previous, "timestamps": o.failure_timestamps },
                },
                "breakdowns": {
                    "operations": breakdown(&o.top_operations),
                    "users": breakdown(&o.top_users),
                    "ips": breakdown(&o.top_ips),
                    "clients": breakdown(&o.top_clients),
                    "sdks": breakdown(&o.top_sdks),
                    "delegation": breakdown(&o.delegation),
                }
            }))
        }
        Err(error) => response::internal(error),
    }
}
