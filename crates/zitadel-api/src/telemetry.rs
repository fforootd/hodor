use crate::{ApiState, response};
use axum::{
    Json, Router,
    extract::{Query, State},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use zitadel_db::{
    current_instance_id, list_fingerprints as db_list_fingerprints, upsert_fingerprint,
};

/// Authenticated routes (list fingerprints).
pub fn routes() -> Router<ApiState> {
    Router::new().route("/telemetry/fingerprints", get(list_fingerprints))
}

/// Public routes (fingerprint ingest — called during login before session exists).
pub fn public_routes() -> Router<ApiState> {
    Router::new().route("/telemetry/fingerprints", post(ingest_fingerprint))
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
    let cursor = p.cursor.unwrap_or_default();
    let limit = p.limit.min(200);
    match db_list_fingerprints(&s.db, current_instance_id().as_ref(), &cursor, limit + 1).await {
        Ok(rows) => {
            let has_more = rows.len() as i64 > limit;
            let items: Vec<FingerprintResponse> = rows
                .into_iter()
                .take(limit as usize)
                .map(|r| FingerprintResponse {
                    id: r.id,
                    type_: r.type_,
                    raw_data: serde_json::from_str(&r.raw_data_json)
                        .unwrap_or(serde_json::Value::Object(Default::default())),
                    created_at: r.created_at,
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
    let id = if req.id.is_empty() {
        uuid::Uuid::new_v4().to_string()
    } else {
        req.id
    };
    let type_ = if req.type_.is_empty() {
        "fingerprintjs".to_string()
    } else {
        req.type_
    };
    let raw_data = serde_json::to_string(&req.raw_data).unwrap_or_else(|_| "{}".into());
    match upsert_fingerprint(
        &s.db,
        current_instance_id().as_ref(),
        &id,
        &type_,
        &raw_data,
    )
    .await
    {
        Ok(_) => response::json_created(serde_json::json!({"id": id})),
        Err(e) => response::bad_request(format!("ingest fingerprint: {e}")),
    }
}
