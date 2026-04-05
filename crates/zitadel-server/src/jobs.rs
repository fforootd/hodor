use std::sync::Arc;
use std::time::Duration;

use google_cloud_spanner::client::Error as SpannerError;
use google_cloud_spanner::statement::Statement;
use serde_json::json;
use tracing::{Instrument, warn};
use uuid::Uuid;
use zitadel_app::hook::{HookContext, HookPhase, HookPipeline};
use zitadel_app::usecase::run_effects;
use zitadel_config::{Config, RetentionConfig, WorkersConfig};
use zitadel_db::{
    BackendKind, DEFAULT_INSTANCE_ID, Db, Dialect, JobBudget, JobReconcileSpec, bool_true_sql,
    complete_job_run, current_timestamp_sql, delete_sink_inbox_records,
    delete_terminal_sessions_records, delete_terminal_tokens_records,
    delete_transient_state_records, due_job_names, ensure_event_partitions, fetch_unshipped_events,
    mark_events_shipped, timestamp_plus_expr, try_acquire_job_lease,
};

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

#[derive(Default)]
struct JobRunResult {
    removed: i64,
}

pub async fn start(
    config: &Config,
    db: Db,
    hooks: Option<Arc<HookPipeline>>,
) -> anyhow::Result<()> {
    let jobs = builtins(config, db.backend());
    zitadel_db::reconcile_jobs(
        &db,
        DEFAULT_INSTANCE_ID,
        &jobs
            .iter()
            .map(|job| JobReconcileSpec {
                name: job.name.into(),
                display_name: job.display_name.into(),
                description: job.description.into(),
                cron: job.cron.into(),
                cadence_secs: parse_duration_spec(job.cadence)
                    .unwrap_or_else(|| Duration::from_secs(60))
                    .as_secs(),
                strategy: job.strategy.into(),
                targets: job
                    .targets
                    .iter()
                    .map(|target| (*target).to_string())
                    .collect(),
                retention: job.retention.clone(),
            })
            .collect::<Vec<_>>(),
    )
    .await?;
    if db.backend() == BackendKind::Postgres {
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

    // Start the event consumer worker if hooks are provided.
    if let Some(hooks) = hooks {
        let db_for_consumer = db.clone();
        let config_for_consumer = config.clone();
        tokio::spawn(async move {
            let poll_interval =
                parse_duration_spec(&config_for_consumer.workers.event_consumer_poll_interval)
                    .unwrap_or_else(|| Duration::from_secs(5));
            loop {
                if let Err(error) =
                    consume_events(&db_for_consumer, &hooks, DEFAULT_INSTANCE_ID).await
                {
                    warn!(%error, "event consumer tick failed");
                }
                tokio::time::sleep(poll_interval).await;
            }
        });
    }

    Ok(())
}

fn builtins(config: &Config, backend: BackendKind) -> Vec<JobSpec> {
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
            strategy: match backend {
                BackendKind::Postgres => "partition_drop",
                BackendKind::Sqlite | BackendKind::Spanner => "chunked_delete",
            },
            targets: &["events"],
            retention: config.storage.retention.events.keep_for.clone(),
        },
    ]
}

async fn reconcile_jobs(db: &Db, jobs: &[JobSpec]) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped_default();
            for job in jobs {
                let cadence =
                    parse_duration_spec(job.cadence).unwrap_or_else(|| Duration::from_secs(60));
                let next_run_expr = timestamp_plus_expr(db.dialect(), cadence.as_secs());
                let current_timestamp = current_timestamp_sql(db.dialect());
                let config_json = json!({
                    "strategy": job.strategy,
                    "targets": job.targets,
                    "retention": job.retention,
                    "cadence": job.cadence,
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
        Db::Spanner(spanner) => {
            for job in jobs {
                let cadence =
                    parse_duration_spec(job.cadence).unwrap_or_else(|| Duration::from_secs(60));
                let config_json = json!({
                    "strategy": job.strategy,
                    "targets": job.targets,
                    "retention": job.retention,
                    "cadence": job.cadence,
                })
                .to_string();

                let mut exists_stmt = Statement::new(
                    "SELECT name FROM jobs WHERE instance_id = @instance_id AND name = @name LIMIT 1",
                );
                exists_stmt.add_param("instance_id", &DEFAULT_INSTANCE_ID);
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
                        timestamp_plus_expr(Dialect::Spanner, cadence.as_secs()),
                    ));
                    stmt.add_param("display_name", &job.display_name);
                    stmt.add_param("description", &job.description);
                    stmt.add_param("cron", &job.cron);
                    stmt.add_param("config_json", &config_json);
                    stmt.add_param("instance_id", &DEFAULT_INSTANCE_ID);
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
                        timestamp_plus_expr(Dialect::Spanner, cadence.as_secs()),
                    ));
                    stmt.add_param("instance_id", &DEFAULT_INSTANCE_ID);
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

async fn run_due_jobs(
    db: &Db,
    config: &Config,
    jobs: &[JobSpec],
    owner: &str,
) -> anyhow::Result<()> {
    let allowed = jobs.iter().map(|job| job.name).collect::<Vec<_>>();
    let due = due_job_names(db, DEFAULT_INSTANCE_ID, &allowed).await?;
    for name in due {
        let Some(spec) = jobs.iter().find(|job| job.name == name) else {
            continue;
        };
        if !try_acquire_job(db, spec, owner, &config.workers).await? {
            continue;
        }

        let result = execute_job(db, config, spec).await;
        match result {
            Ok(run) => {
                complete_job_run(
                    db,
                    DEFAULT_INSTANCE_ID,
                    spec.name,
                    owner,
                    parse_duration_spec(spec.cadence)
                        .unwrap_or_else(|| Duration::from_secs(60))
                        .as_secs(),
                    "ok",
                    "",
                    run.removed,
                )
                .await?
            }
            Err(error) => {
                complete_job_run(
                    db,
                    DEFAULT_INSTANCE_ID,
                    spec.name,
                    owner,
                    parse_duration_spec(spec.cadence)
                        .unwrap_or_else(|| Duration::from_secs(60))
                        .as_secs(),
                    "error",
                    &error.to_string(),
                    0,
                )
                .await?
            }
        }
    }

    Ok(())
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
    try_acquire_job_lease(db, DEFAULT_INSTANCE_ID, job.name, owner, lease_ttl).await
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
    delete_terminal_tokens_records(
        db,
        DEFAULT_INSTANCE_ID,
        parse_duration_spec(&retention.tokens.retain_terminal_for)
            .unwrap_or_else(|| Duration::from_secs(7 * 24 * 60 * 60)),
        budget,
    )
    .await
}

async fn delete_terminal_sessions(
    db: &Db,
    retention: &RetentionConfig,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    delete_terminal_sessions_records(
        db,
        DEFAULT_INSTANCE_ID,
        parse_duration_spec(&retention.sessions.retain_terminal_for)
            .unwrap_or_else(|| Duration::from_secs(7 * 24 * 60 * 60)),
        budget,
    )
    .await
}

async fn delete_transient_state(
    db: &Db,
    retention: &RetentionConfig,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    delete_transient_state_records(
        db,
        DEFAULT_INSTANCE_ID,
        parse_duration_spec(&retention.transient_auth_state.retain_after_expiry)
            .unwrap_or_else(|| Duration::from_secs(60 * 60)),
        budget,
    )
    .await
}

async fn delete_sink_inbox(
    db: &Db,
    retention: &RetentionConfig,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    delete_sink_inbox_records(
        db,
        parse_duration_spec(&retention.sink_inbox.retain_failed_for)
            .unwrap_or_else(|| Duration::from_secs(24 * 60 * 60)),
        budget,
    )
    .await
}

async fn maintain_event_storage(
    db: &Db,
    config: &Config,
    budget: &JobBudget,
) -> anyhow::Result<i64> {
    zitadel_db::maintain_event_storage(
        db,
        DEFAULT_INSTANCE_ID,
        parse_duration_spec(&config.storage.retention.events.keep_for)
            .unwrap_or_else(|| Duration::from_secs(14 * 24 * 60 * 60)),
        budget,
        config.workers.event_partition_premake_days,
    )
    .await
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

// ─── Event consumer ──────────────────────────────────────────

/// Consume unshipped events and run PostEvent effect hooks.
///
/// 1. Fetch a batch of unshipped events from the events table
/// 2. For each event, deserialize the payload and run PostEvent effect hooks
/// 3. Mark the events as shipped (set shipped_at)
async fn consume_events(db: &Db, hooks: &HookPipeline, instance_id: &str) -> anyhow::Result<()> {
    const BATCH_SIZE: u32 = 100;

    let events = fetch_unshipped_events(db, instance_id, BATCH_SIZE).await?;
    if events.is_empty() {
        return Ok(());
    }

    let mut shipped_ids = Vec::with_capacity(events.len());

    for event_record in &events {
        let span = tracing::info_span!(
            "event_consumer",
            event.id = %event_record.id,
            event.type_ = %event_record.event_type,
        );

        // Try to deserialize as a DomainEvent for richer hook context
        let domain_event: Option<zitadel_app::DomainEvent> =
            serde_json::from_str(&event_record.payload).ok();

        let hook_ctx = HookContext {
            instance_id: event_record.instance_id.clone(),
            actor_id: String::new(), // Not available from raw event record
            org_id: String::new(),
            operation: event_record.event_type.clone(),
            metadata: serde_json::from_str(&event_record.metadata).unwrap_or_default(),
        };

        // Run PostEvent effects
        run_effects(
            &hooks.post_event_effects,
            HookPhase::PostEvent,
            &hook_ctx,
            domain_event.as_ref(),
        )
        .instrument(span)
        .await;

        shipped_ids.push(event_record.id.clone());
    }

    if !shipped_ids.is_empty() {
        let marked = mark_events_shipped(db, instance_id, &shipped_ids).await?;
        tracing::debug!(count = marked, "marked events as shipped");
    }

    Ok(())
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
