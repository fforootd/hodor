use std::time::{Duration, Instant};

use google_cloud_spanner::{client::Error as SpannerError, statement::Statement};
use serde_json::json;
use sqlx::Row;

use crate::{BackendKind, DEFAULT_INSTANCE_ID, Db, Dialect};

#[derive(Debug, Clone)]
pub struct JobReconcileSpec {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub cron: String,
    pub cadence_secs: u64,
    pub strategy: String,
    pub targets: Vec<String>,
    pub retention: String,
}

#[derive(Debug, Clone)]
pub struct JobBudget {
    pub batch_size: u32,
    pub max_rows_per_run: u32,
    pub max_run_duration: Duration,
}

pub async fn reconcile_jobs(
    db: &Db,
    instance_id: &str,
    jobs: &[JobReconcileSpec],
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            for job in jobs {
                let next_run_expr = timestamp_plus_expr(db.dialect(), job.cadence_secs);
                let current_timestamp = current_timestamp_sql(db.dialect());
                let config_json = json!({
                    "strategy": job.strategy,
                    "targets": job.targets,
                    "retention": job.retention,
                    "cadence": format!("{}s", job.cadence_secs),
                });
                let sql = format!(
                    "INSERT INTO jobs (instance_id, name, display_name, description, cron, enabled, next_run_at, last_status, last_error, run_count, config_json, created_at, updated_at, last_rows_removed) \
                     VALUES ($1, $2, $3, $4, $5, {}, {}, 'scheduled', '', 0, {}, {current_timestamp}, {current_timestamp}, 0) \
                     ON CONFLICT(instance_id, name) DO UPDATE SET \
                       display_name = EXCLUDED.display_name, \
                       description = EXCLUDED.description, \
                       cron = EXCLUDED.cron, \
                       config_json = EXCLUDED.config_json, \
                       next_run_at = COALESCE(jobs.next_run_at, EXCLUDED.next_run_at), \
                       updated_at = {current_timestamp}",
                    bool_true_sql(db.dialect()),
                    next_run_expr,
                    scoped.json_bind(6),
                );
                sqlx::query(&sql)
                    .bind(instance_id)
                    .bind(&job.name)
                    .bind(&job.display_name)
                    .bind(&job.description)
                    .bind(&job.cron)
                    .bind(config_json.to_string())
                    .execute(scoped.pool())
                    .await?;
            }
            Ok(())
        }
        Db::Spanner(spanner) => {
            for job in jobs {
                let config_json = json!({
                    "strategy": job.strategy,
                    "targets": job.targets,
                    "retention": job.retention,
                    "cadence": format!("{}s", job.cadence_secs),
                })
                .to_string();

                let mut exists_stmt = Statement::new(
                    "SELECT name FROM jobs WHERE instance_id = @instance_id AND name = @name LIMIT 1",
                );
                exists_stmt.add_param("instance_id", &instance_id);
                exists_stmt.add_param("name", &job.name);
                let mut tx = spanner.client().single().await?;
                let mut rows = tx.query(exists_stmt).await?;
                let exists = rows.next().await?.is_some();

                if exists {
                    let mut stmt = Statement::new(&format!(
                        "UPDATE jobs \
                         SET display_name = @display_name, \
                             description = @description, \
                             cron = @cron, \
                             config_json = @config_json, \
                             next_run_at = IFNULL(next_run_at, {}), \
                             updated_at = CURRENT_TIMESTAMP() \
                         WHERE instance_id = @instance_id AND name = @name",
                        timestamp_plus_expr(Dialect::Spanner, job.cadence_secs),
                    ));
                    stmt.add_param("display_name", &job.display_name);
                    stmt.add_param("description", &job.description);
                    stmt.add_param("cron", &job.cron);
                    stmt.add_param("config_json", &config_json);
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("name", &job.name);
                    let _ = spanner
                        .client()
                        .read_write_transaction(|tx| {
                            let stmt = stmt.clone();
                            Box::pin(async move {
                                tx.update(stmt).await?;
                                Ok::<(), SpannerError>(())
                            })
                        })
                        .await?;
                } else {
                    let mut stmt = Statement::new(&format!(
                        "INSERT INTO jobs \
                         (instance_id, name, display_name, description, cron, enabled, next_run_at, last_status, last_error, run_count, config_json, created_at, updated_at, last_rows_removed) \
                         VALUES \
                         (@instance_id, @name, @display_name, @description, @cron, TRUE, {}, 'scheduled', '', 0, @config_json, CURRENT_TIMESTAMP(), CURRENT_TIMESTAMP(), 0)",
                        timestamp_plus_expr(Dialect::Spanner, job.cadence_secs),
                    ));
                    stmt.add_param("instance_id", &instance_id);
                    stmt.add_param("name", &job.name);
                    stmt.add_param("display_name", &job.display_name);
                    stmt.add_param("description", &job.description);
                    stmt.add_param("cron", &job.cron);
                    stmt.add_param("config_json", &config_json);
                    let _ = spanner
                        .client()
                        .read_write_transaction(|tx| {
                            let stmt = stmt.clone();
                            Box::pin(async move {
                                tx.update(stmt).await?;
                                Ok::<(), SpannerError>(())
                            })
                        })
                        .await?;
                }
            }

            Ok(())
        }
    }
}

pub async fn due_job_names(
    db: &Db,
    instance_id: &str,
    allowed_names: &[&str],
) -> anyhow::Result<Vec<String>> {
    if allowed_names.is_empty() {
        return Ok(Vec::new());
    }

    match db {
        Db::Sql(_) => {
            let current_timestamp = current_timestamp_sql(db.dialect());
            let names = allowed_names
                .iter()
                .map(|job| format!("'{}'", job.replace('\'', "''")))
                .collect::<Vec<_>>()
                .join(", ");
            let sql = format!(
                "SELECT name FROM jobs \
                 WHERE instance_id = $1 AND enabled = {} \
                   AND name IN ({names}) \
                   AND (next_run_at IS NULL OR next_run_at <= {current_timestamp}) \
                 ORDER BY COALESCE(next_run_at, created_at) ASC",
                bool_true_sql(db.dialect()),
            );

            let rows = sqlx::query(&sql)
                .bind(instance_id)
                .fetch_all(db.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| row.get::<String, _>(0))
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT name FROM jobs \
                 WHERE instance_id = @instance_id AND enabled = TRUE \
                   AND (next_run_at IS NULL OR next_run_at <= CURRENT_TIMESTAMP()) \
                 ORDER BY next_run_at ASC, created_at ASC",
            );
            stmt.add_param("instance_id", &instance_id);
            let allowed = allowed_names
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>();
            let mut tx = spanner.client().single().await?;
            let mut rows = tx.query(stmt).await?;
            let mut due = Vec::new();
            while let Some(row) = rows.next().await? {
                let name = row.column_by_name::<String>("name")?;
                if allowed.contains(name.as_str()) {
                    due.push(name);
                }
            }
            Ok(due)
        }
    }
}

pub async fn try_acquire_job_lease(
    db: &Db,
    instance_id: &str,
    name: &str,
    owner: &str,
    lease_ttl_secs: u64,
) -> anyhow::Result<bool> {
    let lease_expr = timestamp_plus_expr(db.dialect(), lease_ttl_secs);
    let current_timestamp = current_timestamp_sql(db.dialect());
    let sql = format!(
        "UPDATE jobs \
         SET lease_owner = $1, \
             lease_expires_at = {lease_expr}, \
             last_status = 'running', \
             updated_at = {current_timestamp} \
         WHERE instance_id = $2 AND name = $3 AND enabled = {} \
           AND (next_run_at IS NULL OR next_run_at <= {current_timestamp}) \
           AND (lease_expires_at IS NULL OR lease_expires_at <= {current_timestamp})",
        bool_true_sql(db.dialect()),
    );

    match db {
        Db::Sql(_) => {
            let result = sqlx::query(&sql)
                .bind(owner)
                .bind(instance_id)
                .bind(name)
                .execute(db.pool())
                .await?;
            Ok(result.rows_affected() > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                &sql.replace("$1", "@owner")
                    .replace("$2", "@instance_id")
                    .replace("$3", "@name"),
            );
            stmt.add_param("owner", &owner);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("name", &name);
            let (_, affected) = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
            Ok(affected > 0)
        }
    }
}

pub async fn complete_job_run(
    db: &Db,
    instance_id: &str,
    name: &str,
    owner: &str,
    cadence_secs: u64,
    status: &str,
    error: &str,
    removed: i64,
) -> anyhow::Result<()> {
    let next_run_expr = timestamp_plus_expr(db.dialect(), cadence_secs);
    let current_timestamp = current_timestamp_sql(db.dialect());
    let sql = format!(
        "UPDATE jobs \
         SET lease_owner = '', \
             lease_expires_at = NULL, \
             last_run_at = {current_timestamp}, \
             next_run_at = {next_run_expr}, \
             last_status = $1, \
             last_error = $2, \
             run_count = run_count + 1, \
             last_rows_removed = $3, \
             updated_at = {current_timestamp} \
         WHERE instance_id = $4 AND name = $5 AND lease_owner = $6"
    );

    match db {
        Db::Sql(_) => {
            sqlx::query(&sql)
                .bind(status)
                .bind(error)
                .bind(removed)
                .bind(instance_id)
                .bind(name)
                .bind(owner)
                .execute(db.pool())
                .await?;
            Ok(())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                &sql.replace("$1", "@status")
                    .replace("$2", "@error")
                    .replace("$3", "@removed")
                    .replace("$4", "@instance_id")
                    .replace("$5", "@name")
                    .replace("$6", "@owner"),
            );
            stmt.add_param("status", &status);
            stmt.add_param("error", &error);
            stmt.add_param("removed", &removed);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("name", &name);
            stmt.add_param("owner", &owner);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move {
                        tx.update(stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
            Ok(())
        }
    }
}

pub async fn delete_terminal_tokens_records(
    db: &Db,
    instance_id: &str,
    keep_for: Duration,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    let cutoff = timestamp_minus_expr(db.dialect(), keep_for.as_secs());
    delete_scoped_batches(
        db,
        instance_id,
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

pub async fn delete_terminal_sessions_records(
    db: &Db,
    instance_id: &str,
    keep_for: Duration,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    let cutoff = timestamp_minus_expr(db.dialect(), keep_for.as_secs());
    delete_scoped_batches(
        db,
        instance_id,
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

pub async fn delete_transient_state_records(
    db: &Db,
    instance_id: &str,
    keep_for: Duration,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    let cutoff = timestamp_minus_expr(db.dialect(), keep_for.as_secs());
    let mut removed = 0;
    removed += delete_scoped_batches(
        db,
        instance_id,
        "auth_states",
        "id",
        &format!("expires_at <= {cutoff}"),
        "expires_at",
        budget,
    )
    .await?;
    removed += delete_scoped_batches(
        db,
        instance_id,
        "oidc_auth_requests",
        "id",
        &format!("expires_at <= {cutoff}"),
        "expires_at",
        budget,
    )
    .await?;
    removed += delete_scoped_batches(
        db,
        instance_id,
        "oidc_rp_auth_states",
        "id",
        &format!("expires_at <= {cutoff}"),
        "expires_at",
        budget,
    )
    .await?;
    Ok(removed)
}

pub async fn delete_sink_inbox_records(
    db: &Db,
    keep_for: Duration,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    let cutoff = timestamp_minus_expr(db.dialect(), keep_for.as_secs());
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

pub async fn maintain_event_storage(
    db: &Db,
    instance_id: &str,
    keep_for: Duration,
    budget: &JobBudget,
    premake_days: u32,
) -> anyhow::Result<i64> {
    if db.backend() == BackendKind::Postgres && event_table_is_partitioned(db).await? {
        ensure_event_partitions(db, premake_days).await?;
        let dropped = drop_old_event_partitions(db, keep_for).await?;
        let pruned = prune_event_default_partition(db, keep_for, budget).await?;
        return Ok(dropped + pruned);
    }

    let cutoff = timestamp_minus_expr(db.dialect(), keep_for.as_secs());
    delete_scoped_batches(
        db,
        instance_id,
        "events",
        "id",
        &format!("created_at <= {cutoff}"),
        "created_at",
        budget,
    )
    .await
}

pub async fn event_table_is_partitioned(db: &Db) -> anyhow::Result<bool> {
    if db.backend() != BackendKind::Postgres {
        return Ok(false);
    }

    let partitioned: Option<(i64,)> =
        sqlx::query_as("SELECT 1 FROM pg_partitioned_table WHERE partrelid = 'events'::regclass")
            .fetch_optional(db.pool())
            .await?;
    Ok(partitioned.is_some())
}

async fn delete_scoped_batches(
    db: &Db,
    instance_id: &str,
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
        let deleted = match db {
            Db::Sql(_) => sqlx::query(&sql)
                .bind(instance_id)
                .bind(batch as i64)
                .execute(db.pool())
                .await?
                .rows_affected() as i64,
            Db::Spanner(spanner) => {
                let mut stmt =
                    Statement::new(&sql.replace("$1", "@instance_id").replace("$2", "@batch"));
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("batch", &(batch as i64));
                let (_, affected) = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = stmt.clone();
                        Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                    })
                    .await?;
                affected
            }
        };
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
        let deleted = match db {
            Db::Sql(_) => sqlx::query(&sql)
                .bind(batch as i64)
                .execute(db.pool())
                .await?
                .rows_affected() as i64,
            Db::Spanner(spanner) => {
                let mut stmt = Statement::new(&sql.replace("$1", "@batch"));
                stmt.add_param("batch", &(batch as i64));
                let (_, affected) = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = stmt.clone();
                        Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                    })
                    .await?;
                affected
            }
        };
        removed += deleted;
        if deleted == 0 {
            break;
        }
    }

    Ok(removed)
}

pub async fn ensure_event_partitions(db: &Db, premake_days: u32) -> anyhow::Result<()> {
    if db.backend() != BackendKind::Postgres {
        return Ok(());
    }
    if !event_table_is_partitioned(db).await? {
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

async fn drop_old_event_partitions(db: &Db, keep_for: Duration) -> anyhow::Result<i64> {
    if db.backend() != BackendKind::Postgres {
        return Ok(0);
    }

    let (cutoff_suffix,): (String,) =
        sqlx::query_as("SELECT TO_CHAR(CURRENT_DATE - CAST($1 AS INT), 'YYYYMMDD')")
            .bind(retention_days(keep_for) as i32)
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

    Ok(dropped)
}

async fn prune_event_default_partition(
    db: &Db,
    keep_for: Duration,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    if db.backend() != BackendKind::Postgres {
        return Ok(0);
    }

    let cutoff = timestamp_minus_expr(db.dialect(), keep_for.as_secs());
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

pub fn bool_true_sql(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Postgres | Dialect::Spanner => "TRUE",
        Dialect::Sqlite => "1",
    }
}

pub fn current_timestamp_sql(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Spanner => "CURRENT_TIMESTAMP()",
        Dialect::Postgres | Dialect::Sqlite => "CURRENT_TIMESTAMP",
    }
}

pub fn timestamp_plus_expr(dialect: Dialect, secs: u64) -> String {
    match dialect {
        Dialect::Postgres => format!("CURRENT_TIMESTAMP + INTERVAL '{secs} seconds'"),
        Dialect::Spanner => format!("TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL {secs} SECOND)"),
        Dialect::Sqlite => format!("datetime(CURRENT_TIMESTAMP, '+{secs} seconds')"),
    }
}

fn timestamp_minus_expr(dialect: Dialect, secs: u64) -> String {
    match dialect {
        Dialect::Postgres => format!("CURRENT_TIMESTAMP - INTERVAL '{secs} seconds'"),
        Dialect::Spanner => format!("TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL {secs} SECOND)"),
        Dialect::Sqlite => format!("datetime(CURRENT_TIMESTAMP, '-{secs} seconds')"),
    }
}

fn retention_days(keep_for: Duration) -> u64 {
    keep_for.as_secs().div_ceil(24 * 60 * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retention_duration_rounds_up_to_whole_days() {
        assert_eq!(retention_days(Duration::from_secs(24 * 60 * 60)), 1);
        assert_eq!(retention_days(Duration::from_secs(24 * 60 * 60 + 1)), 2);
    }

    #[test]
    fn owned_job_specs_round_trip_targets() {
        let spec = JobReconcileSpec {
            name: "gc".into(),
            display_name: "GC".into(),
            description: "job".into(),
            cron: "*/5 * * * *".into(),
            cadence_secs: 300,
            strategy: "chunked_delete".into(),
            targets: vec!["sessions".into(), "tokens".into()],
            retention: "7d".into(),
        };
        assert_eq!(spec.targets.len(), 2);
    }

    #[test]
    fn default_instance_constant_stays_available_for_job_wiring() {
        assert_eq!(DEFAULT_INSTANCE_ID, "default");
    }
}
