use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};

pub fn routes() -> Router<ApiState> {
    Router::new().route(
        "/telemetry/fingerprints",
        get(list_fingerprints).post(ingest_fingerprint),
    )
}

#[derive(Serialize)]
struct FingerprintResponse {
    id: String,
    #[serde(rename = "type")]
    type_: String,
    raw_data: serde_json::Value,
    created_at: String,
}

#[derive(Deserialize)]
struct IngestFingerprintRequest {
    #[serde(default)]
    id: String,
    #[serde(default, rename = "type")]
    type_: String,
    #[serde(default)]
    raw_data: serde_json::Value,
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
                    raw_data: serde_json::from_str(&r.2)
                        .unwrap_or(serde_json::Value::Object(Default::default())),
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

/// POST /v1/telemetry/fingerprints — ingest a raw device fingerprint for analytics.
async fn ingest_fingerprint(
    State(s): State<ApiState>,
    Json(req): Json<IngestFingerprintRequest>,
) -> Response {
    let scoped = s.db.scoped_default();
    let id = if req.id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        req.id
    };
    let type_ = if req.type_.is_empty() {
        "thumbmark".to_string()
    } else {
        req.type_
    };
    let raw_data = serde_json::to_string(&req.raw_data).unwrap_or_else(|_| "{}".into());
    let sql = format!(
        "INSERT INTO fingerprints (id, instance_id, type, raw_data, created_at) \
         VALUES ($1, $2, $3, {}, {})",
        scoped.json_bind(4),
        scoped.timestamp_now(),
    );
    match sqlx::query(&sql)
        .bind(&id)
        .bind(scoped.instance_id())
        .bind(&type_)
        .bind(&raw_data)
        .execute(scoped.pool())
        .await
    {
        Ok(_) => response::json_created(serde_json::json!({"id": id})),
        Err(e) => response::bad_request(format!("ingest fingerprint: {e}")),
    }
}
