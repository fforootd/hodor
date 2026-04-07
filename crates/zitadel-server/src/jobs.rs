use std::sync::Arc;
use std::time::Duration;

use google_cloud_spanner::client::Error as SpannerError;
use google_cloud_spanner::statement::Statement;
use serde_json::json;
use tracing::{Instrument, warn};
use uuid::Uuid;
use zitadel_app::effect::{Effect, EffectDispatcher, EffectType};
use zitadel_app::hook::{HookContext, HookPhase, HookPipeline};
use zitadel_app::repo::EffectRepository;
use zitadel_app::usecase::{plan_durable_effects, run_effects};
use zitadel_config::{Config, RetentionConfig, WorkersConfig};
use zitadel_db::repos::adapters::DbEffectRepository;
use zitadel_db::{
    BackendKind, DEFAULT_INSTANCE_ID, Db, Dialect, JobBudget, JobReconcileSpec, bool_true_sql,
    complete_job_run, current_timestamp_sql, delete_terminal_sessions_records,
    delete_terminal_tokens_records, delete_transient_state_records, due_job_names,
    ensure_event_partitions, fetch_unshipped_events, mark_events_shipped, timestamp_plus_expr,
    try_acquire_job_lease,
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

    // Start the effects worker (durable side-effect delivery with retry).
    {
        let effects_repo = DbEffectRepository::new(db.clone());
        let worker_id = format!("effects:{}:{}", std::process::id(), Uuid::new_v4());
        let domain_provisioning_dispatcher = build_domain_provisioning_dispatcher(config, &db);
        let domain_deprovisioning_dispatcher = build_domain_deprovisioning_dispatcher(config, &db);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()?;
        tokio::spawn(async move {
            let poll_interval = Duration::from_secs(2);
            loop {
                if let Err(error) = process_pending_effects(
                    &effects_repo,
                    &client,
                    domain_provisioning_dispatcher.as_deref(),
                    domain_deprovisioning_dispatcher.as_deref(),
                    DEFAULT_INSTANCE_ID,
                    &worker_id,
                )
                .await
                {
                    warn!(%error, "effects worker tick failed");
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
        JobSpec {
            name: "effects_gc",
            display_name: "Effects Cleanup",
            description: "Deletes completed and dead effects older than the retention period.",
            cron: "0 */6 * * *",
            cadence: "6h",
            strategy: "chunked_delete",
            targets: &["effects"],
            retention: "7d".to_string(),
        },
    ]
}

#[allow(dead_code, clippy::needless_borrows_for_generic_args)]
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
        "event_partition_maint" => maintain_event_storage(db, config, &budget).await?,
        "effects_gc" => delete_effects(db, job, &budget).await?,
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

async fn delete_effects(db: &Db, job: &JobSpec, budget: &JobBudget) -> anyhow::Result<i64> {
    let repo = DbEffectRepository::new(db.clone());
    let keep_for = parse_duration_spec(&job.retention)
        .unwrap_or_else(|| Duration::from_secs(7 * 24 * 60 * 60));
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let cutoff = time_from_unix_secs(now_secs.saturating_sub(keep_for.as_secs()));

    let mut removed = 0i64;
    let started = std::time::Instant::now();
    while removed < budget.max_rows_per_run as i64 && started.elapsed() < budget.max_run_duration {
        let remaining = (budget.max_rows_per_run as i64 - removed) as u32;
        let batch = budget.batch_size.min(remaining.max(1));
        let deleted = repo.cleanup(DEFAULT_INSTANCE_ID, &cutoff, batch).await? as i64;
        removed += deleted;
        if deleted == 0 {
            break;
        }
    }

    Ok(removed)
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

// ─── Effects worker ─────────────────────────────────────────

/// Process pending effects with retry and exponential backoff.
///
/// Fetches a batch of effects ready for dispatch, attempts delivery,
/// and records success/failure with appropriate retry scheduling.
async fn process_pending_effects(
    repo: &DbEffectRepository,
    client: &reqwest::Client,
    domain_provisioning_dispatcher: Option<&dyn EffectDispatcher>,
    domain_deprovisioning_dispatcher: Option<&dyn EffectDispatcher>,
    instance_id: &str,
    worker_id: &str,
) -> anyhow::Result<()> {
    const BATCH_SIZE: u32 = 50;
    const LEASE_TTL_SECS: u64 = 60;
    const BASE_BACKOFF_SECS: u64 = 5;
    const MAX_BACKOFF_SECS: u64 = 480;

    let claim_token = format!("{worker_id}:{}", Uuid::new_v4());
    let effects = repo
        .claim_due(instance_id, &claim_token, LEASE_TTL_SECS, BATCH_SIZE)
        .await?;
    if effects.is_empty() {
        return Ok(());
    }

    for effect in &effects {
        let span = tracing::info_span!(
            "effect_dispatch",
            effect.id = %effect.id,
            effect.type_ = %effect.effect_type.as_str(),
            effect.attempt = effect.attempt,
        );

        let result = dispatch_effect(
            client,
            effect,
            domain_provisioning_dispatcher,
            domain_deprovisioning_dispatcher,
        )
        .instrument(span)
        .await;

        match result {
            Ok(()) => {
                repo.mark_completed(instance_id, &effect.id).await?;
                tracing::debug!(
                    effect_id = %effect.id,
                    effect_type = %effect.effect_type.as_str(),
                    "effect delivered"
                );
            }
            Err(error) => {
                let next_attempt = effect.attempt + 1;
                if next_attempt >= effect.max_attempts {
                    repo.mark_dead(instance_id, &effect.id, &format!("{error:#}"))
                        .await?;
                    tracing::warn!(
                        effect_id = %effect.id,
                        effect_type = %effect.effect_type.as_str(),
                        attempts = next_attempt,
                        %error,
                        "effect dead — max attempts exhausted"
                    );
                } else {
                    let backoff_secs =
                        (BASE_BACKOFF_SECS * 2u64.pow(next_attempt as u32)).min(MAX_BACKOFF_SECS);
                    let next_retry_at = chrono_next_retry(backoff_secs);
                    repo.record_failure(
                        instance_id,
                        &effect.id,
                        &format!("{error:#}"),
                        &next_retry_at,
                    )
                    .await?;
                    tracing::debug!(
                        effect_id = %effect.id,
                        attempt = next_attempt,
                        retry_in_secs = backoff_secs,
                        %error,
                        "effect failed — scheduled retry"
                    );
                }
            }
        }
    }
    Ok(())
}

/// Dispatch a single effect based on its type.
async fn dispatch_effect(
    client: &reqwest::Client,
    effect: &Effect,
    domain_provisioning_dispatcher: Option<&dyn EffectDispatcher>,
    domain_deprovisioning_dispatcher: Option<&dyn EffectDispatcher>,
) -> anyhow::Result<()> {
    match effect.effect_type {
        EffectType::Log => {
            tracing::info!(
                effect_id = %effect.id,
                payload = %effect.payload,
                "log effect"
            );
            Ok(())
        }
        EffectType::Webhook => {
            let url = effect
                .config
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("webhook effect missing 'url' in config"))?;

            let mut req = client.post(url).json(&effect.payload);

            // Apply custom headers if present.
            if let Some(headers) = effect.config.get("headers").and_then(|v| v.as_object()) {
                for (key, value) in headers {
                    if let Some(v) = value.as_str() {
                        req = req.header(key, v);
                    }
                }
            }

            // Inject trace context for distributed tracing (Tier 3 OTEL export).
            for (key, value) in zitadel_observability::propagation::trace_context_headers() {
                req = req.header(key, value);
            }

            let resp = req.send().await?;
            if !resp.status().is_success() {
                anyhow::bail!(
                    "webhook returned HTTP {}: {}",
                    resp.status(),
                    resp.text().await.unwrap_or_default()
                );
            }
            Ok(())
        }
        EffectType::Email => {
            // Stub: log intent, real SMTP integration is a future workstream.
            let to = effect
                .config
                .get("to")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>");
            let template = effect
                .config
                .get("template")
                .and_then(|v| v.as_str())
                .unwrap_or("<none>");
            tracing::info!(
                effect_id = %effect.id,
                to = %to,
                template = %template,
                "email effect (stub — not yet delivered)"
            );
            Ok(())
        }
        EffectType::Sms => {
            // Stub: log intent.
            let to = effect
                .config
                .get("to")
                .and_then(|v| v.as_str())
                .unwrap_or("<unknown>");
            tracing::info!(
                effect_id = %effect.id,
                to = %to,
                "sms effect (stub — not yet delivered)"
            );
            Ok(())
        }
        EffectType::DomainProvisioning => {
            let dispatcher = domain_provisioning_dispatcher.ok_or_else(|| {
                anyhow::anyhow!("domain provisioning dispatcher unavailable for this runtime")
            })?;
            dispatcher.dispatch(effect).await
        }
        EffectType::DomainDeprovisioning => {
            let dispatcher = domain_deprovisioning_dispatcher.ok_or_else(|| {
                anyhow::anyhow!("domain deprovisioning dispatcher unavailable for this runtime")
            })?;
            dispatcher.dispatch(effect).await
        }
    }
}

fn build_domain_provisioning_dispatcher(
    config: &Config,
    db: &Db,
) -> Option<Arc<dyn EffectDispatcher>> {
    if !zitadel_cloud::is_enabled(&config.cloud, config.is_dev()) {
        return None;
    }
    let gcp = &config.cloud.gcp;
    if gcp.project_id.is_empty()
        || gcp.certificate_map.is_empty()
        || gcp.url_map.is_empty()
        || gcp.backend_service.is_empty()
    {
        warn!("cloud is enabled but GCP domain provisioning config is incomplete");
        return None;
    }
    Some(Arc::new(
        zitadel_cloud::infra::DomainProvisioningDispatcher::new(
            Arc::new(zitadel_cloud::gcp::GcpClient::new(gcp.clone())),
            db.clone(),
        ),
    ))
}

fn build_domain_deprovisioning_dispatcher(
    config: &Config,
    db: &Db,
) -> Option<Arc<dyn EffectDispatcher>> {
    if !zitadel_cloud::is_enabled(&config.cloud, config.is_dev()) {
        return None;
    }
    let gcp = &config.cloud.gcp;
    if gcp.project_id.is_empty()
        || gcp.certificate_map.is_empty()
        || gcp.url_map.is_empty()
        || gcp.backend_service.is_empty()
    {
        warn!("cloud is enabled but GCP domain deprovisioning config is incomplete");
        return None;
    }
    Some(Arc::new(
        zitadel_cloud::infra::DomainDeprovisioningDispatcher::new(
            Arc::new(zitadel_cloud::gcp::GcpClient::new(gcp.clone())),
            db.clone(),
        ),
    ))
}

/// Compute a concrete UTC timestamp for retry scheduling.
fn chrono_next_retry(backoff_secs: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let future = now + Duration::from_secs(backoff_secs);
    // Format as ISO 8601 compatible with SQLite datetime()
    let secs = future.as_secs();
    time_from_unix_secs(secs)
}

/// Simple UTC datetime string from unix seconds (no external crate needed).
fn time_from_unix_secs(secs: u64) -> String {
    // Days since epoch, hours, minutes, seconds
    const SECS_PER_DAY: u64 = 86400;
    const SECS_PER_HOUR: u64 = 3600;
    const SECS_PER_MIN: u64 = 60;

    let days = secs / SECS_PER_DAY;
    let remaining = secs % SECS_PER_DAY;
    let hour = remaining / SECS_PER_HOUR;
    let min = (remaining % SECS_PER_HOUR) / SECS_PER_MIN;
    let sec = remaining % SECS_PER_MIN;

    // Convert days since 1970-01-01 to y/m/d
    let (year, month, day) = days_to_ymd(days);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02}")
}

fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
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

    let effects_repo = DbEffectRepository::new(db.clone());
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
            actor_id: domain_event
                .as_ref()
                .map(|event| event.actor_id().to_string())
                .unwrap_or_default(),
            org_id: String::new(),
            operation: event_record.event_type.clone(),
            event_id: Some(event_record.id.clone()),
            metadata: serde_json::from_str(&event_record.metadata).unwrap_or_default(),
        };

        let planned = plan_durable_effects(
            &hooks.post_event_effects,
            HookPhase::PostEvent,
            &hook_ctx,
            domain_event.as_ref(),
        )
        .instrument(span.clone())
        .await?;
        if !planned.is_empty() {
            effects_repo.enqueue_batch(instance_id, &planned).await?;
        }

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
