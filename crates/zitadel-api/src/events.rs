use crate::{ApiState, response};
use axum::{
    Router,
    extract::{Query, State},
    response::{Response, Sse, sse::Event as SseEvent},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::convert::Infallible;
use tokio_stream::StreamExt;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/events", get(list_events))
        .route("/events/stream", get(stream_events))
}

#[derive(Deserialize)]
pub struct EventParams {
    #[serde(default = "default_limit")]
    pub limit: i64,
    pub cursor: Option<String>,
    #[serde(alias = "types")]
    pub event_type: Option<String>,
    pub session_id: Option<String>,
    pub fingerprint: Option<String>,
    pub aggregate_id: Option<String>,
}
fn default_limit() -> i64 {
    50
}

#[derive(Serialize)]
pub struct EventResponse {
    pub id: String,
    pub event_type: String,
    pub category: String,
    pub org_id: String,
    pub actor_id: Option<String>,
    pub actor_type: Option<String>,
    pub aggregate_id: Option<String>,
    pub aggregate_type: Option<String>,
    pub resource_type: Option<String>,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub flow_id: Option<String>,
    pub fingerprint: Option<String>,
    pub client_id: Option<String>,
    pub token_id: Option<String>,
    pub delegation_type: Option<String>,
    pub sdk_name: Option<String>,
    pub sdk_version: Option<String>,
    pub sequence: Option<i64>,
    pub created_at: String,
}

async fn list_events(State(s): State<ApiState>, Query(p): Query<EventParams>) -> Response {
    let scoped = s.db.scoped_default();
    let cursor = p.cursor.unwrap_or_default();
    let (created_at, _) = scoped.select_timestamps();
    let payload = scoped.as_text("payload");
    let metadata = scoped.as_text("metadata");

    // Build dynamic WHERE clause with optional filters.
    let mut conditions = vec!["instance_id = $1".to_string(), "id > $2".to_string()];
    let mut bind_idx = 3u32;
    let mut extra_binds: Vec<String> = Vec::new();

    if let Some(ref et) = p.event_type {
        conditions.push(format!("event_type = ${bind_idx}"));
        extra_binds.push(et.clone());
        bind_idx += 1;
    }
    if let Some(ref sid) = p.session_id {
        conditions.push(format!("session_id = ${bind_idx}"));
        extra_binds.push(sid.clone());
        bind_idx += 1;
    }
    if let Some(ref fp) = p.fingerprint {
        conditions.push(format!("fingerprint = ${bind_idx}"));
        extra_binds.push(fp.clone());
        bind_idx += 1;
    }
    if let Some(ref aid) = p.aggregate_id {
        conditions.push(format!("aggregate_id = ${bind_idx}"));
        extra_binds.push(aid.clone());
        bind_idx += 1;
    }

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        "SELECT id, event_type, category, org_id, actor_id, actor_type, \
         aggregate_id, aggregate_type, resource_type, \
         {payload}, {metadata}, \
         request_id, session_id, flow_id, fingerprint, \
         client_id, token_id, delegation_type, \
         sdk_name, sdk_version, sequence, {created_at} \
         FROM events WHERE {where_clause} ORDER BY created_at DESC LIMIT ${bind_idx}"
    );

    let mut query = sqlx::query(&sql).bind(scoped.instance_id()).bind(&cursor);
    for val in &extra_binds {
        query = query.bind(val);
    }
    query = query.bind(p.limit.min(500));

    match query.fetch_all(scoped.pool()).await {
        Ok(rows) => {
            let items: Vec<EventResponse> = rows
                .into_iter()
                .map(|r| {
                    let payload_str: String = r.get(9);
                    let metadata_str: String = r.get(10);
                    EventResponse {
                        id: r.get(0),
                        event_type: r.get(1),
                        category: r.get(2),
                        org_id: r.get(3),
                        actor_id: r.get(4),
                        actor_type: r.get(5),
                        aggregate_id: r.get(6),
                        aggregate_type: r.get(7),
                        resource_type: r.get(8),
                        payload: serde_json::from_str(&payload_str).unwrap_or_default(),
                        metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
                        request_id: r.get(11),
                        session_id: r.get(12),
                        flow_id: r.get(13),
                        fingerprint: r.get(14),
                        client_id: r.get(15),
                        token_id: r.get(16),
                        delegation_type: r.get(17),
                        sdk_name: r.get(18),
                        sdk_version: r.get(19),
                        sequence: r.get(20),
                        created_at: r.get(21),
                    }
                })
                .collect();
            let next_cursor = items.last().map(|e| e.id.clone());
            response::json_ok(response::ListResponse {
                items,
                next_cursor,
                total: None,
            })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

/// SSE event stream — polls for new events every 2 seconds.
async fn stream_events(
    State(_s): State<ApiState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<SseEvent, Infallible>>> {
    let stream = tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(
        std::time::Duration::from_secs(2),
    ))
    .map(move |_| {
        let event = SseEvent::default()
            .event("ping")
            .data(format!("{{\"ts\":\"{}\"}}", chrono_now()));
        Ok::<_, Infallible>(event)
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

fn chrono_now() -> String {
    // Simple UTC timestamp without chrono dependency.
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    format!("{}", d.as_secs())
}
