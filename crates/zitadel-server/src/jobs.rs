use std::time::{Duration, Instant};

use serde_json::json;
use sqlx::Row;
use tracing::{info, warn};
use uuid::Uuid;
use zitadel_config::{Config, RetentionConfig, WorkersConfig};
use zitadel_db::{DEFAULT_INSTANCE_ID, Db, Dialect};

#[derive(Clone)]
struct JobSpec {
    name: &'static str,
    display_name: &'static str,
    description: &'static str,
    cron: &'static str,
    cadence: &'static str,
    strategy: &'static str,
    targets: &'static [&'static str],
    retention: String,
}

struct JobBudget {
    batch_size: u32,
    max_rows_per_run: u32,
    max_run_duration: Duration,
}

#[derive(Default)]
struct JobRunResult {
    removed: i64,
}

pub async fn start(config: &Config, db: Db) -> anyhow::Result<()> {
    let jobs = builtins(config, db.dialect());
    reconcile_jobs(&db, &jobs).await?;
    if db.dialect() == Dialect::Postgres {
        ensure_event_partitions(&db, config.workers.event_partition_premake_days).await?;
    }

    if !config.workers.scheduler_enabled {
        return Ok(());
    }

    let db_for_task = db.clone();
    let config_for_task = config.clone();
    let jobs_for_task = jobs.clone();
    let owner = format!("scheduler:{}:{}", std::process::id(), Uuid::new_v4());
    tokio::spawn(async move {
        let poll_interval = parse_duration_spec(&config_for_task.workers.scheduler_poll_interval)
            .unwrap_or_else(|| Duration::from_secs(30));
        loop {
            if let Err(error) =
                run_due_jobs(&db_for_task, &config_for_task, &jobs_for_task, &owner).await
            {
                warn!(%error, "background jobs tick failed");
            }
            tokio::time::sleep(poll_interval).await;
        }
    });

    Ok(())
}

fn builtins(config: &Config, dialect: Dialect) -> Vec<JobSpec> {
    vec![
        JobSpec {
            name: "token_gc",
            display_name: "Token Cleanup",
            description: "Physically deletes revoked and expired tokens in bounded batches.",
            cron: "*/15 * * * *",
            cadence: "15m",
            strategy: "chunked_delete",
            targets: &["tokens"],
            retention: config.storage.retention.tokens.retain_terminal_for.clone(),
        },
        JobSpec {
            name: "session_gc",
            display_name: "Session Cleanup",
            description: "Physically deletes revoked and expired sessions in bounded batches.",
            cron: "*/15 * * * *",
            cadence: "15m",
            strategy: "chunked_delete",
            targets: &["sessions"],
            retention: config
                .storage
                .retention
                .sessions
                .retain_terminal_for
                .clone(),
        },
        JobSpec {
            name: "transient_state_gc",
            display_name: "Transient State Cleanup",
            description: "Deletes expired auth and login runtime state after a short safety buffer.",
            cron: "*/5 * * * *",
            cadence: "5m",
            strategy: "chunked_delete",
            targets: &["auth_states", "oidc_auth_requests", "oidc_rp_auth_states"],
            retention: config
                .storage
                .retention
                .transient_auth_state
                .retain_after_expiry
                .clone(),
        },
        JobSpec {
            name: "sink_inbox_gc",
            display_name: "Sink Inbox Cleanup",
            description: "Deletes stale sink inbox rows after bounded retry retention.",
            cron: "*/5 * * * *",
            cadence: "5m",
            strategy: "chunked_delete",
            targets: &["storage_sink_inbox"],
            retention: config
                .storage
                .retention
                .sink_inbox
                .retain_failed_for
                .clone(),
        },
        JobSpec {
            name: "event_partition_maint",
            display_name: "Event Partition Maintenance",
            description: "Premakes event partitions and drops old partitions past retention.",
            cron: "0 * * * *",
            cadence: "1h",
            strategy: match dialect {
                Dialect::Postgres => "partition_drop",
                Dialect::Sqlite => "chunked_delete",
            },
            targets: &["events"],
            retention: config.storage.retention.events.keep_for.clone(),
        },
    ]
}

async fn reconcile_jobs(db: &Db, jobs: &[JobSpec]) -> anyhow::Result<()> {
    let scoped = db.scoped_default();
    for job in jobs {
        let cadence = parse_duration_spec(job.cadence).unwrap_or_else(|| Duration::from_secs(60));
        let next_run_expr = timestamp_plus_expr(db.dialect(), cadence.as_secs());
        let config_json = json!({
            "strategy": job.strategy,
            "targets": job.targets,
            "retention": job.retention,
            "cadence": job.cadence,
        });
        let sql = format!(
            "INSERT INTO jobs (instance_id, name, display_name, description, cron, enabled, next_run_at, last_status, last_error, run_count, config_json, created_at, updated_at, last_rows_removed) \
             VALUES ($1, $2, $3, $4, $5, {}, {}, 'scheduled', '', 0, {}, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, 0) \
             ON CONFLICT(instance_id, name) DO UPDATE SET \
               display_name = EXCLUDED.display_name, \
               description = EXCLUDED.description, \
               cron = EXCLUDED.cron, \
               config_json = EXCLUDED.config_json, \
               next_run_at = COALESCE(jobs.next_run_at, EXCLUDED.next_run_at), \
               updated_at = CURRENT_TIMESTAMP",
            bool_true_sql(db.dialect()),
            next_run_expr,
            scoped.json_bind(6),
        );
        sqlx::query(&sql)
            .bind(scoped.instance_id())
            .bind(job.name)
            .bind(job.display_name)
            .bind(job.description)
            .bind(job.cron)
            .bind(config_json.to_string())
            .execute(scoped.pool())
            .await?;
    }

    Ok(())
}

async fn run_due_jobs(
    db: &Db,
    config: &Config,
    jobs: &[JobSpec],
    owner: &str,
) -> anyhow::Result<()> {
    let due = due_job_names(db, jobs).await?;
    for name in due {
        let Some(spec) = jobs.iter().find(|job| job.name == name) else {
            continue;
        };
        if !try_acquire_job(db, spec, owner, &config.workers).await? {
            continue;
        }

        let result = execute_job(db, config, spec).await;
        match result {
            Ok(run) => complete_job(db, spec, owner, "ok", "", run.removed).await?,
            Err(error) => complete_job(db, spec, owner, "error", &error.to_string(), 0).await?,
        }
    }

    Ok(())
}

async fn due_job_names(db: &Db, jobs: &[JobSpec]) -> anyhow::Result<Vec<String>> {
    if jobs.is_empty() {
        return Ok(Vec::new());
    }

    let names = jobs
        .iter()
        .map(|job| format!("'{}'", job.name.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT name FROM jobs \
         WHERE instance_id = $1 AND enabled = {} \
           AND name IN ({names}) \
           AND (next_run_at IS NULL OR next_run_at <= CURRENT_TIMESTAMP) \
         ORDER BY COALESCE(next_run_at, created_at) ASC",
        bool_true_sql(db.dialect()),
    );

    let rows = sqlx::query(&sql)
        .bind(DEFAULT_INSTANCE_ID)
        .fetch_all(db.pool())
        .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>(0))
        .collect())
}

async fn try_acquire_job(
    db: &Db,
    job: &JobSpec,
    owner: &str,
    workers: &WorkersConfig,
) -> anyhow::Result<bool> {
    let lease_ttl = parse_duration_spec(&workers.scheduler_lease_ttl)
        .unwrap_or_else(|| Duration::from_secs(90))
        .as_secs();
    let lease_expr = timestamp_plus_expr(db.dialect(), lease_ttl);
    let sql = format!(
        "UPDATE jobs \
         SET lease_owner = $1, \
             lease_expires_at = {lease_expr}, \
             last_status = 'running', \
             updated_at = CURRENT_TIMESTAMP \
         WHERE instance_id = $2 AND name = $3 AND enabled = {} \
           AND (next_run_at IS NULL OR next_run_at <= CURRENT_TIMESTAMP) \
           AND (lease_expires_at IS NULL OR lease_expires_at <= CURRENT_TIMESTAMP)",
        bool_true_sql(db.dialect()),
    );
    let result = sqlx::query(&sql)
        .bind(owner)
        .bind(DEFAULT_INSTANCE_ID)
        .bind(job.name)
        .execute(db.pool())
        .await?;
    Ok(result.rows_affected() > 0)
}

async fn complete_job(
    db: &Db,
    job: &JobSpec,
    owner: &str,
    status: &str,
    error: &str,
    removed: i64,
) -> anyhow::Result<()> {
    let cadence = parse_duration_spec(job.cadence).unwrap_or_else(|| Duration::from_secs(60));
    let next_run_expr = timestamp_plus_expr(db.dialect(), cadence.as_secs());
    let sql = format!(
        "UPDATE jobs \
         SET lease_owner = '', \
             lease_expires_at = NULL, \
             last_run_at = CURRENT_TIMESTAMP, \
             next_run_at = {next_run_expr}, \
             last_status = $1, \
             last_error = $2, \
             run_count = run_count + 1, \
             last_rows_removed = $3, \
             updated_at = CURRENT_TIMESTAMP \
         WHERE instance_id = $4 AND name = $5 AND lease_owner = $6"
    );
    sqlx::query(&sql)
        .bind(status)
        .bind(error)
        .bind(removed)
        .bind(DEFAULT_INSTANCE_ID)
        .bind(job.name)
        .bind(owner)
        .execute(db.pool())
        .await?;
    Ok(())
}

async fn execute_job(db: &Db, config: &Config, job: &JobSpec) -> anyhow::Result<JobRunResult> {
    let budget = JobBudget {
        batch_size: config.workers.cleanup_batch_size.max(1),
        max_rows_per_run: config.workers.cleanup_max_rows_per_run.max(1),
        max_run_duration: parse_duration_spec(&config.workers.cleanup_max_run_duration)
            .unwrap_or_else(|| Duration::from_secs(2)),
    };

    let removed = match job.name {
        "token_gc" => delete_terminal_tokens(db, &config.storage.retention, &budget).await?,
        "session_gc" => delete_terminal_sessions(db, &config.storage.retention, &budget).await?,
        "transient_state_gc" => {
            delete_transient_state(db, &config.storage.retention, &budget).await?
        }
        "sink_inbox_gc" => delete_sink_inbox(db, &config.storage.retention, &budget).await?,
        "event_partition_maint" => maintain_event_storage(db, config, &budget).await?,
        other => anyhow::bail!("unsupported built-in job: {other}"),
    };

    Ok(JobRunResult { removed })
}

async fn delete_terminal_tokens(
    db: &Db,
    retention: &RetentionConfig,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    let cutoff = timestamp_minus_expr(
        db.dialect(),
        parse_duration_spec(&retention.tokens.retain_terminal_for)
            .unwrap_or_else(|| Duration::from_secs(7 * 24 * 60 * 60))
            .as_secs(),
    );
    delete_scoped_batches(
        db,
        "tokens",
        "id",
        &format!(
            "((revoked_at IS NOT NULL AND revoked_at <= {cutoff}) OR (expires_at IS NOT NULL AND expires_at <= {cutoff}))"
        ),
        "COALESCE(revoked_at, expires_at)",
        budget,
    )
    .await
}

async fn delete_terminal_sessions(
    db: &Db,
    retention: &RetentionConfig,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    let cutoff = timestamp_minus_expr(
        db.dialect(),
        parse_duration_spec(&retention.sessions.retain_terminal_for)
            .unwrap_or_else(|| Duration::from_secs(7 * 24 * 60 * 60))
            .as_secs(),
    );
    delete_scoped_batches(
        db,
        "sessions",
        "id",
        &format!(
            "((revoked_at IS NOT NULL AND revoked_at <= {cutoff}) OR (expires_at IS NOT NULL AND expires_at <= {cutoff}))"
        ),
        "COALESCE(revoked_at, expires_at)",
        budget,
    )
    .await
}

async fn delete_transient_state(
    db: &Db,
    retention: &RetentionConfig,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    let cutoff = timestamp_minus_expr(
        db.dialect(),
        parse_duration_spec(&retention.transient_auth_state.retain_after_expiry)
            .unwrap_or_else(|| Duration::from_secs(60 * 60))
            .as_secs(),
    );
    let mut removed = 0;
    removed += delete_scoped_batches(
        db,
        "auth_states",
        "id",
        &format!("expires_at <= {cutoff}"),
        "expires_at",
        budget,
    )
    .await?;
    removed += delete_scoped_batches(
        db,
        "oidc_auth_requests",
        "id",
        &format!("expires_at <= {cutoff}"),
        "expires_at",
        budget,
    )
    .await?;
    removed += delete_scoped_batches(
        db,
        "oidc_rp_auth_states",
        "id",
        &format!("expires_at <= {cutoff}"),
        "expires_at",
        budget,
    )
    .await?;
    Ok(removed)
}

async fn delete_sink_inbox(
    db: &Db,
    retention: &RetentionConfig,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    let cutoff = timestamp_minus_expr(
        db.dialect(),
        parse_duration_spec(&retention.sink_inbox.retain_failed_for)
            .unwrap_or_else(|| Duration::from_secs(24 * 60 * 60))
            .as_secs(),
    );
    delete_unscoped_batches(
        db,
        "storage_sink_inbox",
        "id",
        &format!("created_at <= {cutoff}"),
        "created_at",
        budget,
    )
    .await
}

async fn maintain_event_storage(
    db: &Db,
    config: &Config,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    if db.dialect() == Dialect::Postgres {
        ensure_event_partitions(db, config.workers.event_partition_premake_days).await?;
        let dropped =
            drop_old_event_partitions(db, &config.storage.retention.events.keep_for).await?;
        let pruned =
            prune_event_default_partition(db, &config.storage.retention.events.keep_for, budget)
                .await?;
        return Ok(dropped + pruned);
    }

    let cutoff = timestamp_minus_expr(
        db.dialect(),
        parse_duration_spec(&config.storage.retention.events.keep_for)
            .unwrap_or_else(|| Duration::from_secs(14 * 24 * 60 * 60))
            .as_secs(),
    );
    delete_scoped_batches(
        db,
        "events",
        "id",
        &format!("created_at <= {cutoff}"),
        "created_at",
        budget,
    )
    .await
}

async fn delete_scoped_batches(
    db: &Db,
    table: &str,
    id_column: &str,
    predicate: &str,
    order_expr: &str,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    let mut removed = 0i64;
    let started = Instant::now();

    while removed < budget.max_rows_per_run as i64 && started.elapsed() < budget.max_run_duration {
        let remaining = (budget.max_rows_per_run as i64 - removed) as u32;
        let batch = budget.batch_size.min(remaining.max(1));
        let sql = format!(
            "DELETE FROM {table} \
             WHERE instance_id = $1 AND {id_column} IN ( \
                SELECT {id_column} FROM {table} \
                WHERE instance_id = $1 AND {predicate} \
                ORDER BY {order_expr} ASC \
                LIMIT $2 \
             )"
        );
        let deleted = sqlx::query(&sql)
            .bind(DEFAULT_INSTANCE_ID)
            .bind(batch as i64)
            .execute(db.pool())
            .await?
            .rows_affected() as i64;
        removed += deleted;
        if deleted == 0 {
            break;
        }
    }

    Ok(removed)
}

async fn delete_unscoped_batches(
    db: &Db,
    table: &str,
    id_column: &str,
    predicate: &str,
    order_expr: &str,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    let mut removed = 0i64;
    let started = Instant::now();

    while removed < budget.max_rows_per_run as i64 && started.elapsed() < budget.max_run_duration {
        let remaining = (budget.max_rows_per_run as i64 - removed) as u32;
        let batch = budget.batch_size.min(remaining.max(1));
        let sql = format!(
            "DELETE FROM {table} \
             WHERE {id_column} IN ( \
                SELECT {id_column} FROM {table} \
                WHERE {predicate} \
                ORDER BY {order_expr} ASC \
                LIMIT $1 \
             )"
        );
        let deleted = sqlx::query(&sql)
            .bind(batch as i64)
            .execute(db.pool())
            .await?
            .rows_affected() as i64;
        removed += deleted;
        if deleted == 0 {
            break;
        }
    }

    Ok(removed)
}

async fn ensure_event_partitions(db: &Db, premake_days: u32) -> anyhow::Result<()> {
    if db.dialect() != Dialect::Postgres {
        return Ok(());
    }

    let partitioned: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM pg_partitioned_table WHERE partrelid = 'events'::regclass")
            .fetch_optional(db.pool())
            .await?;
    if partitioned.is_none() {
        return Ok(());
    }

    for offset in -1..=(premake_days as i32) {
        let (suffix, start_date, end_date): (String, String, String) = sqlx::query_as(
            "SELECT TO_CHAR(CURRENT_DATE + CAST($1 AS INT), 'YYYYMMDD'), \
                    CAST(CURRENT_DATE + CAST($1 AS INT) AS TEXT), \
                    CAST(CURRENT_DATE + CAST($2 AS INT) AS TEXT)",
        )
        .bind(offset)
        .bind(offset + 1)
        .fetch_one(db.pool())
        .await?;

        let table_name = format!("events_p{suffix}");
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {table_name} PARTITION OF events \
             FOR VALUES FROM ('{start_date}') TO ('{end_date}')"
        );
        sqlx::query(&sql).execute(db.pool()).await?;
    }

    Ok(())
}

async fn drop_old_event_partitions(db: &Db, keep_for: &str) -> anyhow::Result<i64> {
    if db.dialect() != Dialect::Postgres {
        return Ok(0);
    }

    let retention_days = retention_days(keep_for);
    let (cutoff_suffix,): (String,) =
        sqlx::query_as("SELECT TO_CHAR(CURRENT_DATE - CAST($1 AS INT), 'YYYYMMDD')")
            .bind(retention_days as i32)
            .fetch_one(db.pool())
            .await?;
    let partitions = sqlx::query(
        "SELECT c.relname \
         FROM pg_class c \
         JOIN pg_inherits i ON i.inhrelid = c.oid \
         JOIN pg_class p ON p.oid = i.inhparent \
         WHERE p.relname = 'events' AND c.relname LIKE 'events_p________'",
    )
    .fetch_all(db.pool())
    .await?;

    let mut dropped = 0;
    for row in partitions {
        let relname: String = row.get(0);
        let Some(suffix) = relname.strip_prefix("events_p") else {
            continue;
        };
        if suffix < cutoff_suffix.as_str() {
            let sql = format!("DROP TABLE IF EXISTS {relname}");
            sqlx::query(&sql).execute(db.pool()).await?;
            dropped += 1;
        }
    }

    if dropped > 0 {
        info!(dropped, keep_for, "dropped old event partitions");
    }

    Ok(dropped)
}

async fn prune_event_default_partition(
    db: &Db,
    keep_for: &str,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    if db.dialect() != Dialect::Postgres {
        return Ok(0);
    }

    let cutoff = timestamp_minus_expr(
        db.dialect(),
        parse_duration_spec(keep_for)
            .unwrap_or_else(|| Duration::from_secs(14 * 24 * 60 * 60))
            .as_secs(),
    );
    let mut removed = 0i64;
    let started = Instant::now();

    while removed < budget.max_rows_per_run as i64 && started.elapsed() < budget.max_run_duration {
        let remaining = (budget.max_rows_per_run as i64 - removed) as u32;
        let batch = budget.batch_size.min(remaining.max(1));
        let sql = format!(
            "DELETE FROM events_default \
             WHERE ctid IN ( \
                SELECT ctid FROM events_default \
                WHERE created_at <= {cutoff} \
                ORDER BY created_at ASC \
                LIMIT $1 \
             )"
        );
        let deleted = sqlx::query(&sql)
            .bind(batch as i64)
            .execute(db.pool())
            .await?
            .rows_affected() as i64;
        removed += deleted;
        if deleted == 0 {
            break;
        }
    }

    Ok(removed)
}

fn bool_true_sql(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Postgres => "TRUE",
        Dialect::Sqlite => "1",
    }
}

fn timestamp_plus_expr(dialect: Dialect, secs: u64) -> String {
    match dialect {
        Dialect::Postgres => format!("CURRENT_TIMESTAMP + INTERVAL '{secs} seconds'"),
        Dialect::Sqlite => format!("datetime(CURRENT_TIMESTAMP, '+{secs} seconds')"),
    }
}

fn timestamp_minus_expr(dialect: Dialect, secs: u64) -> String {
    match dialect {
        Dialect::Postgres => format!("CURRENT_TIMESTAMP - INTERVAL '{secs} seconds'"),
        Dialect::Sqlite => format!("datetime(CURRENT_TIMESTAMP, '-{secs} seconds')"),
    }
}

fn retention_days(raw: &str) -> u64 {
    let secs = parse_duration_spec(raw)
        .unwrap_or_else(|| Duration::from_secs(14 * 24 * 60 * 60))
        .as_secs();
    secs.div_ceil(24 * 60 * 60)
}

fn parse_duration_spec(raw: &str) -> Option<Duration> {
    if raw.is_empty() {
        return None;
    }
    if let Some(value) = raw.strip_suffix("ms") {
        return value.parse::<u64>().ok().map(Duration::from_millis);
    }
    if let Some(value) = raw.strip_suffix('s') {
        return value.parse::<u64>().ok().map(Duration::from_secs);
    }
    if let Some(value) = raw.strip_suffix('m') {
        return value
            .parse::<u64>()
            .ok()
            .map(|secs| Duration::from_secs(secs * 60));
    }
    if let Some(value) = raw.strip_suffix('h') {
        return value
            .parse::<u64>()
            .ok()
            .map(|secs| Duration::from_secs(secs * 60 * 60));
    }
    if let Some(value) = raw.strip_suffix('d') {
        return value
            .parse::<u64>()
            .ok()
            .map(|days| Duration::from_secs(days * 24 * 60 * 60));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_retention_duration_strings() {
        assert_eq!(parse_duration_spec("30s"), Some(Duration::from_secs(30)));
        assert_eq!(parse_duration_spec("15m"), Some(Duration::from_secs(900)));
        assert_eq!(parse_duration_spec("2h"), Some(Duration::from_secs(7200)));
        assert_eq!(parse_duration_spec("7d"), Some(Duration::from_secs(604800)));
    }
}
