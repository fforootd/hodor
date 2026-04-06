use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::Response,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use zitadel_app::telemetry::UpsertFingerprintCommand;

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
    Extension(identity): Extension<Identity>,
    Query(p): Query<response::PaginationParams>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cursor = p.cursor.unwrap_or_default();
    let limit = p.limit.min(200);
    match s.app.runner.run_fn(&ctx, "telemetry.list_fingerprints", || {
        s.app.list_fingerprints.execute(&ctx, &cursor, limit + 1)
    }).await {
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
        Err(e) => response::app_error(e),
    }
}

/// POST /v1/telemetry/fingerprints — ingest a raw device fingerprint for analytics.
async fn ingest_fingerprint(
    State(s): State<ApiState>,
    Json(req): Json<IngestFingerprintRequest>,
) -> Response {
    let instance_id = zitadel_db::current_instance_id();
    let ctx = zitadel_app::ActorContext {
        auth: zitadel_app::AuthContext {
            identity: zitadel_app::Identity {
                user_id: String::new(),
                session_id: String::new(),
                token_type: "anonymous".to_string(),
                org_id: String::new(),
            },
            capabilities: vec![],
        },
        instance: zitadel_app::InstanceContext {
            instance_id: instance_id.into_owned(),
            placement_mode: String::new(),
            region_key: None,
            feature_overrides: std::collections::HashMap::new(),
            host: String::new(),
        },
    };
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
    let cmd = UpsertFingerprintCommand {
        id: id.clone(),
        type_,
        raw_data,
    };
    match s.app.runner.run_fn(&ctx, "telemetry.upsert_fingerprint", || {
        s.app.upsert_fingerprint.execute(&ctx, cmd)
    }).await {
        Ok(()) => response::json_created(serde_json::json!({"id": id})),
        Err(e) => response::app_error(e),
    }
}
