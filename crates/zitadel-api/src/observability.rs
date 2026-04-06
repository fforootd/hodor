use crate::{ApiState, response};
use axum::{Router, extract::State, response::Response, routing::get};
use serde::Deserialize;
use serde_json::{Value, json};
use zitadel_db::{Dialect, current_instance_id};
use zitadel_storage::AnalyticsQuery;

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
    axum::extract::Query(p): axum::extract::Query<OverviewParams>,
) -> Response {
    let hours = match p.range.as_str() {
        "1h" => 1,
        "12h" => 12,
        "24h" => 24,
        "7d" => 24 * 7,
        "30d" => 24 * 30,
        _ => 12,
    };

    let instance = sql_string_literal(current_instance_id().as_ref());
    let cur_since = recent_timestamp_expr(s.db.dialect(), hours);
    let prev_since = recent_timestamp_expr(s.db.dialect(), hours * 2);
    let cur_window = format!("created_at >= {cur_since}");
    let prev_window = format!("created_at >= {prev_since} AND created_at < {cur_since}");

    let counts_sql = format!(
        "SELECT \
           SUM(CASE WHEN event_type LIKE 'auth.%' AND {cur_window} THEN 1 ELSE 0 END) AS auth_cur, \
           SUM(CASE WHEN event_type LIKE 'auth.%' AND {prev_window} THEN 1 ELSE 0 END) AS auth_prev, \
           SUM(CASE WHEN event_type = 'auth.token_issued' AND {cur_window} THEN 1 ELSE 0 END) AS tok_cur, \
           SUM(CASE WHEN event_type = 'auth.token_issued' AND {prev_window} THEN 1 ELSE 0 END) AS tok_prev, \
           SUM(CASE WHEN event_type = 'auth.login_failed' AND {cur_window} THEN 1 ELSE 0 END) AS fail_cur, \
           SUM(CASE WHEN event_type = 'auth.login_failed' AND {prev_window} THEN 1 ELSE 0 END) AS fail_prev \
         FROM events \
         WHERE instance_id = {instance} AND event_type NOT LIKE 'log.%' AND created_at >= {prev_since}"
    );
    let counts = match analytics_row_map(&s.analytics, counts_sql).await {
        Ok(map) => map,
        Err(error) => return response::internal_error(format!("{error}")),
    };

    let sessions_current_sql = format!(
        "SELECT COUNT(*) AS total FROM sessions \
         WHERE instance_id = {instance} AND revoked_at IS NULL AND expires_at > {now} AND {cur_window}",
        now = current_timestamp_expr(s.db.dialect())
    );
    let sessions_prev_sql = format!(
        "SELECT COUNT(*) AS total FROM sessions \
         WHERE instance_id = {instance} AND revoked_at IS NULL AND {prev_window}",
    );

    let auth_ts = match fetch_timestamps(
        &s.analytics,
        format!(
            "SELECT created_at FROM events WHERE instance_id = {instance} AND event_type LIKE 'auth.%' AND event_type NOT LIKE 'log.%' AND {cur_window}"
        ),
    )
    .await
    {
        Ok(values) => values,
        Err(error) => return response::internal_error(format!("{error}")),
    };
    let sess_ts = match fetch_timestamps(
        &s.analytics,
        format!(
            "SELECT created_at FROM sessions WHERE instance_id = {instance} AND revoked_at IS NULL AND expires_at > {now} AND {cur_window}",
            now = current_timestamp_expr(s.db.dialect())
        ),
    )
    .await
    {
        Ok(values) => values,
        Err(error) => return response::internal_error(format!("{error}")),
    };
    let tok_ts = match fetch_timestamps(
        &s.analytics,
        format!(
            "SELECT created_at FROM events WHERE instance_id = {instance} AND event_type = 'auth.token_issued' AND {cur_window}"
        ),
    )
    .await
    {
        Ok(values) => values,
        Err(error) => return response::internal_error(format!("{error}")),
    };
    let fail_ts = match fetch_timestamps(
        &s.analytics,
        format!(
            "SELECT created_at FROM events WHERE instance_id = {instance} AND event_type = 'auth.login_failed' AND {cur_window}"
        ),
    )
    .await
    {
        Ok(values) => values,
        Err(error) => return response::internal_error(format!("{error}")),
    };

    let top_ops = match fetch_breakdown(
        &s.analytics,
        format!(
            "SELECT event_type AS name, COUNT(*) AS count FROM events \
             WHERE instance_id = {instance} AND event_type != '' AND event_type NOT LIKE 'log.%' AND {cur_window} \
             GROUP BY event_type ORDER BY count DESC LIMIT 8"
        ),
    )
    .await
    {
        Ok(values) => values,
        Err(error) => return response::internal_error(format!("{error}")),
    };
    let top_users = match fetch_breakdown(
        &s.analytics,
        format!(
            "SELECT COALESCE(NULLIF(actor_id, ''), 'Anonymous') AS name, COUNT(*) AS count FROM events \
             WHERE instance_id = {instance} AND event_type NOT LIKE 'log.%' AND {cur_window} \
             GROUP BY COALESCE(NULLIF(actor_id, ''), 'Anonymous') ORDER BY count DESC LIMIT 8"
        ),
    )
    .await
    {
        Ok(values) => values,
        Err(error) => return response::internal_error(format!("{error}")),
    };
    let top_ips = match fetch_breakdown(
        &s.analytics,
        format!(
            "SELECT ip_address AS name, COUNT(*) AS count FROM sessions \
             WHERE instance_id = {instance} AND ip_address IS NOT NULL AND ip_address != '' AND {cur_window} \
             GROUP BY ip_address ORDER BY count DESC LIMIT 8"
        ),
    )
    .await
    {
        Ok(values) => values,
        Err(error) => return response::internal_error(format!("{error}")),
    };
    let top_clients = match fetch_breakdown(
        &s.analytics,
        format!(
            "SELECT COALESCE(NULLIF(client_id, ''), 'Console') AS name, COUNT(*) AS count FROM events \
             WHERE instance_id = {instance} AND event_type NOT LIKE 'log.%' AND {cur_window} \
             GROUP BY COALESCE(NULLIF(client_id, ''), 'Console') ORDER BY count DESC LIMIT 8"
        ),
    )
    .await
    {
        Ok(values) => values,
        Err(error) => return response::internal_error(format!("{error}")),
    };
    let top_sdks = match fetch_breakdown(
        &s.analytics,
        format!(
            "SELECT COALESCE(NULLIF(sdk_name, ''), 'Browser') AS name, COUNT(*) AS count FROM events \
             WHERE instance_id = {instance} AND event_type NOT LIKE 'log.%' AND {cur_window} \
             GROUP BY COALESCE(NULLIF(sdk_name, ''), 'Browser') ORDER BY count DESC LIMIT 8"
        ),
    )
    .await
    {
        Ok(values) => values,
        Err(error) => return response::internal_error(format!("{error}")),
    };
    let delegation = match fetch_breakdown(
        &s.analytics,
        format!(
            "SELECT COALESCE(NULLIF(delegation_type, ''), 'direct') AS name, COUNT(*) AS count FROM events \
             WHERE instance_id = {instance} AND event_type NOT LIKE 'log.%' AND {cur_window} \
             GROUP BY COALESCE(NULLIF(delegation_type, ''), 'direct') ORDER BY count DESC LIMIT 8"
        ),
    )
    .await
    {
        Ok(values) => values,
        Err(error) => return response::internal_error(format!("{error}")),
    };

    let sessions_current = analytics_scalar_i64(&s.analytics, sessions_current_sql)
        .await
        .unwrap_or(0);
    let sessions_previous = analytics_scalar_i64(&s.analytics, sessions_prev_sql)
        .await
        .unwrap_or(0);

    response::json_ok(json!({
        "metrics": {
            "auth": { "current": map_i64(&counts, "auth_cur"), "previous": map_i64(&counts, "auth_prev"), "timestamps": auth_ts },
            "sessions": { "current": sessions_current, "previous": sessions_previous, "timestamps": sess_ts },
            "tokens": { "current": map_i64(&counts, "tok_cur"), "previous": map_i64(&counts, "tok_prev"), "timestamps": tok_ts },
            "failed": { "current": map_i64(&counts, "fail_cur"), "previous": map_i64(&counts, "fail_prev"), "timestamps": fail_ts },
        },
        "breakdowns": {
            "operations": top_ops,
            "users": top_users,
            "ips": top_ips,
            "clients": top_clients,
            "sdks": top_sdks,
            "delegation": delegation,
        }
    }))
}

async fn analytics_row_map(
    analytics: &zitadel_storage::DefaultAnalyticsStorage,
    sql: String,
) -> anyhow::Result<std::collections::BTreeMap<String, Value>> {
    let result = analytics
        .query(&AnalyticsQuery {
            sql,
            params: vec![],
            limit: Some(1),
        })
        .await?;
    if let Some(error) = result.error {
        anyhow::bail!("{error}");
    }
    let Some(row) = result.rows.first() else {
        return Ok(Default::default());
    };
    Ok(result
        .columns
        .iter()
        .cloned()
        .zip(row.iter().cloned())
        .collect())
}

async fn analytics_scalar_i64(
    analytics: &zitadel_storage::DefaultAnalyticsStorage,
    sql: String,
) -> anyhow::Result<i64> {
    let map = analytics_row_map(analytics, sql).await?;
    Ok(map.get("total").and_then(value_as_i64).unwrap_or(0))
}

async fn fetch_timestamps(
    analytics: &zitadel_storage::DefaultAnalyticsStorage,
    sql: String,
) -> anyhow::Result<Vec<i64>> {
    let result = analytics
        .query(&AnalyticsQuery {
            sql,
            params: vec![],
            limit: Some(5000),
        })
        .await?;
    if let Some(error) = result.error {
        anyhow::bail!("{error}");
    }
    let created_idx = result
        .columns
        .iter()
        .position(|column| column == "created_at")
        .unwrap_or(0);
    Ok(result
        .rows
        .into_iter()
        .filter_map(|row| row.get(created_idx).and_then(value_as_string))
        .filter_map(|ts| parse_ts_ms(&ts))
        .collect())
}

async fn fetch_breakdown(
    analytics: &zitadel_storage::DefaultAnalyticsStorage,
    sql: String,
) -> anyhow::Result<Vec<Value>> {
    let result = analytics
        .query(&AnalyticsQuery {
            sql,
            params: vec![],
            limit: Some(8),
        })
        .await?;
    if let Some(error) = result.error {
        anyhow::bail!("{error}");
    }
    let name_idx = result
        .columns
        .iter()
        .position(|column| column == "name")
        .unwrap_or(0);
    let count_idx = result
        .columns
        .iter()
        .position(|column| column == "count")
        .unwrap_or(1);
    Ok(result
        .rows
        .into_iter()
        .map(|row| {
            json!({
                "name": row.get(name_idx).and_then(value_as_string).unwrap_or_default(),
                "count": row.get(count_idx).and_then(value_as_i64).unwrap_or(0),
            })
        })
        .collect())
}

fn map_i64(map: &std::collections::BTreeMap<String, Value>, key: &str) -> i64 {
    map.get(key).and_then(value_as_i64).unwrap_or(0)
}

fn value_as_string(value: &Value) -> Option<String> {
    match value {
        Value::String(raw) => Some(raw.clone()),
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn value_as_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(number) => number.as_i64(),
        Value::String(raw) => raw.parse().ok(),
        _ => None,
    }
}

fn sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn current_timestamp_expr(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Sqlite => "datetime('now')",
        Dialect::Postgres => "NOW()",
        Dialect::Spanner => "CURRENT_TIMESTAMP()",
    }
}

fn recent_timestamp_expr(dialect: Dialect, hours: i64) -> String {
    match dialect {
        Dialect::Sqlite => format!("datetime('now', '-{hours} hours')"),
        Dialect::Postgres => format!("NOW() - INTERVAL '{hours} hours'"),
        Dialect::Spanner => format!("TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL {hours} HOUR)"),
    }
}

fn parse_ts_ms(ts: &str) -> Option<i64> {
    let s = ts.trim().trim_end_matches('Z');
    let s = s.replace('T', " ");
    let parts: Vec<&str> = s.splitn(6, |c: char| !c.is_ascii_digit()).collect();
    if parts.len() < 6 {
        return None;
    }
    let y: i64 = parts[0].parse().ok()?;
    let mo: i64 = parts[1].parse().ok()?;
    let d: i64 = parts[2].parse().ok()?;
    let h: i64 = parts[3].parse().ok()?;
    let mi: i64 = parts[4].parse().ok()?;
    let se: i64 = parts[5].parse().ok()?;

    let mut days = 0i64;
    for yr in 1970..y {
        days += if yr % 4 == 0 && (yr % 100 != 0 || yr % 400 == 0) {
            366
        } else {
            365
        };
    }
    let month_days = [
        31,
        if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            29
        } else {
            28
        },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for m in 0..(mo - 1) as usize {
        days += month_days.get(m).copied().unwrap_or(30) as i64;
    }
    days += d - 1;

    Some((days * 86400 + h * 3600 + mi * 60 + se) * 1000)
}
