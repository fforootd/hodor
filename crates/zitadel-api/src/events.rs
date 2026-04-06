use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_db::current_instance_id;
use zitadel_storage::{AnalyticsQuery, AnalyticsQueryResult};

pub fn routes() -> Router<ApiState> {
    Router::new().route("/events", get(list_events))
    // TODO: SSE event stream removed — will be re-implemented when event consumption worker is ready.
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

async fn list_events(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(p): Query<EventParams>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    if let Err(e) = crate::fga_check(&s, &ctx, "viewer", "events:*").await {
        return e;
    }
    tracing::debug!(actor = %ctx.user_id(), "list_events");
    let cursor = decode_cursor(p.cursor.as_deref());

    // Build parameterized WHERE clause — all user-supplied values use $N bind params.
    let mut conditions = Vec::new();
    let mut params: Vec<String> = Vec::new();

    params.push(current_instance_id().to_string());
    conditions.push(format!("instance_id = ${}", params.len()));

    if let Some(ref et) = p.event_type {
        params.push(et.clone());
        conditions.push(format!("event_type = ${}", params.len()));
    } else {
        conditions.push("event_type NOT LIKE 'log.%'".to_string());
    }
    if let Some(ref sid) = p.session_id {
        params.push(sid.clone());
        conditions.push(format!("session_id = ${}", params.len()));
    }
    if let Some(ref fp) = p.fingerprint {
        params.push(fp.clone());
        conditions.push(format!("fingerprint = ${}", params.len()));
    }
    if let Some(ref aid) = p.aggregate_id {
        params.push(aid.clone());
        conditions.push(format!("aggregate_id = ${}", params.len()));
    }
    if let Some((cursor_created_at, cursor_id)) = &cursor {
        params.push(cursor_created_at.clone());
        let ts_idx = params.len();
        params.push(cursor_id.clone());
        let id_idx = params.len();
        conditions.push(format!(
            "(created_at < ${ts_idx} OR (created_at = ${ts_idx} AND id < ${id_idx}))"
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
            params,
            limit: Some(p.limit.min(500)),
        })
        .await
    {
        Ok(result) => {
            if let Some(error) = result.error {
                return response::internal(error);
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
        Err(e) => response::internal(e),
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

fn event_from_analytics_row(
    result: &AnalyticsQueryResult,
    row: &[serde_json::Value],
) -> EventResponse {
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
    result
        .columns
        .iter()
        .position(|candidate| candidate == column)
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

fn row_i64(result: &AnalyticsQueryResult, row: &[serde_json::Value], column: &str) -> Option<i64> {
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
        Some(serde_json::Value::String(raw)) => {
            serde_json::from_str(raw).unwrap_or_else(|_| serde_json::Value::String(raw.clone()))
        }
        Some(other) => other.clone(),
        None => serde_json::Value::Null,
    }
}
