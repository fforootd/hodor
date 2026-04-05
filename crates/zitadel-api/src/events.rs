use crate::{ApiState, response};
use axum::{
    Router,
    extract::{Query, State},
    response::{Response, Sse, sse::Event as SseEvent},
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use tokio_stream::StreamExt;
use zitadel_db::current_instance_id;
use zitadel_storage::{AnalyticsQuery, AnalyticsQueryResult};

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
    let cursor = decode_cursor(p.cursor.as_deref());

    let mut conditions = vec![format!(
        "instance_id = {}",
        sql_string_literal(current_instance_id().as_ref())
    )];

    if let Some(ref et) = p.event_type {
        conditions.push(format!("event_type = {}", sql_string_literal(et)));
    } else {
        conditions.push("event_type NOT LIKE 'log.%'".to_string());
    }
    if let Some(ref sid) = p.session_id {
        conditions.push(format!("session_id = {}", sql_string_literal(sid)));
    }
    if let Some(ref fp) = p.fingerprint {
        conditions.push(format!("fingerprint = {}", sql_string_literal(fp)));
    }
    if let Some(ref aid) = p.aggregate_id {
        conditions.push(format!("aggregate_id = {}", sql_string_literal(aid)));
    }
    if let Some((cursor_created_at, cursor_id)) = &cursor {
        let cursor_ts_expr = timestamp_literal(s.db.dialect(), cursor_created_at);
        conditions.push(format!(
            "(created_at < {cursor_ts_expr} OR (created_at = {cursor_ts_expr} AND id < {}))",
            sql_string_literal(cursor_id)
        ));
    }

    let where_clause = conditions.join(" AND ");
    let sql = format!(
        "SELECT id, event_type, category, org_id, actor_id, actor_type, \
         aggregate_id, aggregate_type, resource_type, payload, metadata, \
         request_id, session_id, flow_id, fingerprint, \
         client_id, token_id, delegation_type, sdk_name, sdk_version, sequence, created_at \
         FROM events WHERE {where_clause} ORDER BY created_at DESC, id DESC LIMIT {}",
        p.limit.min(500),
    );

    match s
        .analytics
        .query(&AnalyticsQuery {
            sql,
            limit: Some(p.limit.min(500)),
        })
        .await
    {
        Ok(result) => {
            if let Some(error) = result.error {
                return response::internal_error(error);
            }
            let items: Vec<EventResponse> = result
                .rows
                .iter()
                .map(|row| event_from_analytics_row(&result, row))
                .collect();
            let next_cursor = items.last().map(|e| encode_cursor(&e.created_at, &e.id));
            response::json_ok(response::ListResponse {
                items,
                next_cursor,
                total: None,
            })
        }
        Err(e) => response::internal_error(format!("{e}")),
    }
}

fn encode_cursor(created_at: &str, id: &str) -> String {
    format!("{created_at}|{id}")
}

fn decode_cursor(cursor: Option<&str>) -> Option<(String, String)> {
    let raw = cursor?.trim();
    if raw.is_empty() || raw == "now" {
        return None;
    }
    let (created_at, id) = raw.split_once('|')?;
    Some((created_at.to_string(), id.to_string()))
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

fn event_from_analytics_row(result: &AnalyticsQueryResult, row: &[serde_json::Value]) -> EventResponse {
    EventResponse {
        id: row_string(result, row, "id").unwrap_or_default(),
        event_type: row_string(result, row, "event_type").unwrap_or_default(),
        category: row_string(result, row, "category").unwrap_or_default(),
        org_id: row_string(result, row, "org_id").unwrap_or_default(),
        actor_id: row_optional_string(result, row, "actor_id"),
        actor_type: row_optional_string(result, row, "actor_type"),
        aggregate_id: row_optional_string(result, row, "aggregate_id"),
        aggregate_type: row_optional_string(result, row, "aggregate_type"),
        resource_type: row_optional_string(result, row, "resource_type"),
        payload: row_json(result, row, "payload"),
        metadata: row_json(result, row, "metadata"),
        request_id: row_optional_string(result, row, "request_id"),
        session_id: row_optional_string(result, row, "session_id"),
        flow_id: row_optional_string(result, row, "flow_id"),
        fingerprint: row_optional_string(result, row, "fingerprint"),
        client_id: row_optional_string(result, row, "client_id"),
        token_id: row_optional_string(result, row, "token_id"),
        delegation_type: row_optional_string(result, row, "delegation_type"),
        sdk_name: row_optional_string(result, row, "sdk_name"),
        sdk_version: row_optional_string(result, row, "sdk_version"),
        sequence: row_i64(result, row, "sequence"),
        created_at: row_string(result, row, "created_at").unwrap_or_default(),
    }
}

fn column_index(result: &AnalyticsQueryResult, column: &str) -> Option<usize> {
    result.columns.iter().position(|candidate| candidate == column)
}

fn row_value<'a>(
    result: &'a AnalyticsQueryResult,
    row: &'a [serde_json::Value],
    column: &str,
) -> Option<&'a serde_json::Value> {
    column_index(result, column).and_then(|index| row.get(index))
}

fn row_string(
    result: &AnalyticsQueryResult,
    row: &[serde_json::Value],
    column: &str,
) -> Option<String> {
    match row_value(result, row, column)? {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn row_optional_string(
    result: &AnalyticsQueryResult,
    row: &[serde_json::Value],
    column: &str,
) -> Option<String> {
    row_string(result, row, column)
}

fn row_i64(
    result: &AnalyticsQueryResult,
    row: &[serde_json::Value],
    column: &str,
) -> Option<i64> {
    row_value(result, row, column).and_then(|value| match value {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(raw) => raw.parse().ok(),
        _ => None,
    })
}

fn row_json(
    result: &AnalyticsQueryResult,
    row: &[serde_json::Value],
    column: &str,
) -> serde_json::Value {
    match row_value(result, row, column) {
        Some(serde_json::Value::String(raw)) => serde_json::from_str(raw)
            .unwrap_or_else(|_| serde_json::Value::String(raw.clone())),
        Some(other) => other.clone(),
        None => serde_json::Value::Null,
    }
}

fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn timestamp_literal(dialect: zitadel_db::Dialect, value: &str) -> String {
    match dialect {
        zitadel_db::Dialect::Sqlite => format!("datetime({})", sql_string_literal(value)),
        zitadel_db::Dialect::Postgres => format!("CAST({} AS TIMESTAMPTZ)", sql_string_literal(value)),
        zitadel_db::Dialect::Spanner => format!("TIMESTAMP({})", sql_string_literal(value)),
    }
}
