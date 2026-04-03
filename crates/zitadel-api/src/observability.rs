use crate::{ApiState, response};
use axum::{Router, extract::State, response::Response, routing::get};
use serde::Deserialize;

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

/// GET /v1/observability/overview — single endpoint for the Overview dashboard.
/// Returns all metrics, sparkline timestamps, and breakdown tables in one response.
async fn overview(
    State(s): State<ApiState>,
    axum::extract::Query(p): axum::extract::Query<OverviewParams>,
) -> Response {
    let scoped = s.db.scoped_default();
    let pool = scoped.pool();

    let hours: i64 = match p.range.as_str() {
        "1h" => 1,
        "12h" => 12,
        "24h" => 24,
        "7d" => 24 * 7,
        "30d" => 24 * 30,
        _ => 12,
    };

    // Compute thresholds for current and previous periods.
    let cur_threshold = format!("-{hours} hours");
    let prev_threshold = format!("-{} hours", hours * 2);

    // Run all count queries in a single pass using CASE expressions.
    // This avoids 14 separate queries — one scan through events covers everything.
    let counts_sql = format!(
        "SELECT \
           SUM(CASE WHEN event_type LIKE 'auth.%' AND created_at >= datetime('now', $1) THEN 1 ELSE 0 END) as auth_cur, \
           SUM(CASE WHEN event_type LIKE 'auth.%' AND created_at >= datetime('now', $2) AND created_at < datetime('now', $1) THEN 1 ELSE 0 END) as auth_prev, \
           SUM(CASE WHEN event_type = 'auth.token_issued' AND created_at >= datetime('now', $1) THEN 1 ELSE 0 END) as tok_cur, \
           SUM(CASE WHEN event_type = 'auth.token_issued' AND created_at >= datetime('now', $2) AND created_at < datetime('now', $1) THEN 1 ELSE 0 END) as tok_prev, \
           SUM(CASE WHEN event_type = 'auth.login_failed' AND created_at >= datetime('now', $1) THEN 1 ELSE 0 END) as fail_cur, \
           SUM(CASE WHEN event_type = 'auth.login_failed' AND created_at >= datetime('now', $2) AND created_at < datetime('now', $1) THEN 1 ELSE 0 END) as fail_prev \
         FROM events WHERE instance_id = $3 AND event_type NOT LIKE 'log.%' AND created_at >= datetime('now', $2)"
    );
    let counts: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(&counts_sql)
        .bind(&cur_threshold)
        .bind(&prev_threshold)
        .bind(scoped.instance_id())
        .fetch_one(pool)
        .await
        .unwrap_or((0, 0, 0, 0, 0, 0));

    // Active sessions count for current and previous period.
    let sess_cur: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND revoked_at IS NULL AND expires_at > datetime('now') AND created_at >= datetime('now', $2)",
    )
    .bind(scoped.instance_id())
    .bind(&cur_threshold)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    let sess_prev: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM sessions WHERE instance_id = $1 AND revoked_at IS NULL AND created_at >= datetime('now', $2) AND created_at < datetime('now', $3)",
    )
    .bind(scoped.instance_id())
    .bind(&prev_threshold)
    .bind(&cur_threshold)
    .fetch_one(pool)
    .await
    .unwrap_or((0,));

    // Sparkline timestamps — just the created_at values for current period.
    // Return as epoch milliseconds so the frontend can bucket them.
    let auth_ts = fetch_timestamps(
        pool,
        scoped.instance_id(),
        "event_type LIKE 'auth.%'",
        &cur_threshold,
    )
    .await;
    let sess_ts = fetch_session_timestamps(pool, scoped.instance_id(), &cur_threshold).await;
    let tok_ts = fetch_timestamps(
        pool,
        scoped.instance_id(),
        "event_type = 'auth.token_issued'",
        &cur_threshold,
    )
    .await;
    let fail_ts = fetch_timestamps(
        pool,
        scoped.instance_id(),
        "event_type = 'auth.login_failed'",
        &cur_threshold,
    )
    .await;

    // Breakdown queries — top operations, users, IPs, clients, SDKs, delegation.
    let top_ops = fetch_breakdown(
        pool,
        scoped.instance_id(),
        "event_type",
        "event_type != '' AND event_type NOT LIKE 'log.%'",
        &cur_threshold,
        "events",
    )
    .await;
    let top_users = fetch_breakdown(
        pool,
        scoped.instance_id(),
        "COALESCE(NULLIF(actor_id, ''), 'Anonymous')",
        "event_type NOT LIKE 'log.%'",
        &cur_threshold,
        "events",
    )
    .await;
    let top_ips = fetch_breakdown(
        pool,
        scoped.instance_id(),
        "ip_address",
        "ip_address IS NOT NULL AND ip_address != ''",
        &cur_threshold,
        "sessions",
    )
    .await;
    let top_clients = fetch_breakdown(
        pool,
        scoped.instance_id(),
        "COALESCE(NULLIF(client_id, ''), 'Console')",
        "event_type NOT LIKE 'log.%'",
        &cur_threshold,
        "events",
    )
    .await;
    let top_sdks = fetch_breakdown(
        pool,
        scoped.instance_id(),
        "COALESCE(NULLIF(sdk_name, ''), 'Browser')",
        "event_type NOT LIKE 'log.%'",
        &cur_threshold,
        "events",
    )
    .await;
    let delegation = fetch_breakdown(
        pool,
        scoped.instance_id(),
        "COALESCE(NULLIF(delegation_type, ''), 'direct')",
        "event_type NOT LIKE 'log.%'",
        &cur_threshold,
        "events",
    )
    .await;

    response::json_ok(serde_json::json!({
        "metrics": {
            "auth": { "current": counts.0, "previous": counts.1, "timestamps": auth_ts },
            "sessions": { "current": sess_cur.0, "previous": sess_prev.0, "timestamps": sess_ts },
            "tokens": { "current": counts.2, "previous": counts.3, "timestamps": tok_ts },
            "failed": { "current": counts.4, "previous": counts.5, "timestamps": fail_ts },
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

async fn fetch_timestamps(
    pool: &sqlx::AnyPool,
    instance_id: &str,
    filter: &str,
    threshold: &str,
) -> Vec<i64> {
    let sql = format!(
        "SELECT created_at FROM events WHERE instance_id = $1 AND {filter} AND event_type NOT LIKE 'log.%' AND created_at >= datetime('now', $2)"
    );
    let rows: Vec<(String,)> = sqlx::query_as(&sql)
        .bind(instance_id)
        .bind(threshold)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.into_iter()
        .filter_map(|(ts,)| parse_ts_ms(&ts))
        .collect()
}

async fn fetch_session_timestamps(
    pool: &sqlx::AnyPool,
    instance_id: &str,
    threshold: &str,
) -> Vec<i64> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT created_at FROM sessions WHERE instance_id = $1 AND revoked_at IS NULL AND expires_at > datetime('now') AND created_at >= datetime('now', $2)",
    )
    .bind(instance_id)
    .bind(threshold)
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    rows.into_iter()
        .filter_map(|(ts,)| parse_ts_ms(&ts))
        .collect()
}

async fn fetch_breakdown(
    pool: &sqlx::AnyPool,
    instance_id: &str,
    group_expr: &str,
    extra_filter: &str,
    threshold: &str,
    table: &str,
) -> Vec<serde_json::Value> {
    let sql = format!(
        "SELECT {group_expr} as name, COUNT(*) as count FROM {table} \
         WHERE instance_id = $1 AND created_at >= datetime('now', $2) AND {extra_filter} \
         GROUP BY {group_expr} ORDER BY count DESC LIMIT 8"
    );
    let rows: Vec<(String, i64)> = sqlx::query_as(&sql)
        .bind(instance_id)
        .bind(threshold)
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.into_iter()
        .map(|(name, count)| serde_json::json!({"name": name, "count": count}))
        .collect()
}

/// Parse "YYYY-MM-DD HH:MM:SS" or ISO 8601 to epoch milliseconds.
fn parse_ts_ms(ts: &str) -> Option<i64> {
    let s = ts.trim().trim_end_matches('Z');
    let s = s.replace('T', " ");
    // "2026-04-03 17:00:00" → split on non-numeric chars
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

    // Days from year 1970 (simplified, no leap-second precision needed for sparklines)
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
