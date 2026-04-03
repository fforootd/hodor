use crate::{ApiState, response};
use axum::{
    Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};
use serde::Serialize;

pub fn routes() -> Router<ApiState> {
    Router::new().route("/telemetry/fingerprints", get(list_fingerprints))
}

#[derive(Serialize)]
struct FingerprintResponse {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    raw_data: String,
    created_at: String,
}

/// GET /v1/telemetry/fingerprints — list device fingerprints with cursor pagination.
async fn list_fingerprints(
    State(s): State<ApiState>,
    Query(p): Query<response::PaginationParams>,
) -> Response {
    let scoped = s.db.scoped_default();
    let cursor = p.cursor.unwrap_or_default();
    let limit = p.limit.min(200);
    let created_at = scoped.as_text("created_at");
    let sql = format!(
        "SELECT id, type, raw_data, {created_at} \
         FROM fingerprints WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
    );
    match sqlx::query_as::<_, (String, String, String, String)>(&sql)
        .bind(scoped.instance_id())
        .bind(&cursor)
        .bind(limit + 1)
        .fetch_all(scoped.pool())
        .await
    {
        Ok(rows) => {
            let has_more = rows.len() as i64 > limit;
            let items: Vec<FingerprintResponse> = rows
                .into_iter()
                .take(limit as usize)
                .map(|r| FingerprintResponse {
                    id: r.0,
                    type_: r.1,
                    raw_data: r.2,
                    created_at: r.3,
                })
                .collect();
            let next_cursor = if has_more {
                items.last().map(|f| f.id.clone())
            } else {
                None
            };
            response::json_ok(response::ListResponse {
                items,
                next_cursor,
                total: None,
            })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}
