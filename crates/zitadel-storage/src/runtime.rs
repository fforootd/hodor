use std::{collections::HashMap, sync::Arc};

use zitadel_config::StorageConfig;
use zitadel_db::{BackendKind, Db};

use crate::{
    ChannelSink, DefaultAnalyticsStorage, DefaultKvStore, DefaultSink, DefaultStatefulStorage,
    DefaultTransientStorage, MemoryKvStore, NoopAnalyticsSink, SpannerAnalyticsQueryBackend,
    SpannerKvStore, SpannerReadStore, SpannerStatefulStore, SqlAnalyticsQueryBackend, SqlKvStore,
    SqlReadStore, SqlSink, SqlStatefulStore, prepare_postgres_kv_schema,
    prepare_postgres_sink_schema, storage_backend_capabilities,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageRoleSummary {
    pub stateful: String,
    pub read: String,
    pub kv: String,
    pub sink: String,
    pub process_cache: String,
    pub analytics: String,
}

pub struct StorageRuntime {
    pub stateful: Arc<DefaultStatefulStorage>,
    pub transient: Arc<DefaultTransientStorage>,
    pub analytics: Arc<DefaultAnalyticsStorage>,
    pub roles: StorageRoleSummary,
}

impl StorageRuntime {
    pub async fn from_config(
        config: &StorageConfig,
        db: Db,
        session_max_age_secs: u64,
    ) -> anyhow::Result<Self> {
        let mut opened_role_dbs = HashMap::new();
        let stateful_backend = db.backend().to_string();

        let read_backend = derive_read_backend(config, db.backend())?;
        let read_store = if read_backend == "postgres_replica" {
            if config.read.url.is_empty() {
                anyhow::bail!(
                    "storage.read.url is required when storage.read.backend = \"postgres_replica\""
                );
            }
            let read_db = resolve_role_db(
                &config.read.url,
                &config.stateful.url,
                &db,
                &mut opened_role_dbs,
            )
            .await?;
            SqlReadStore::new(read_db)
        } else {
            SqlReadStore::new(db.clone())
        };

        let stateful = Arc::new(match db.backend() {
            BackendKind::Spanner => DefaultStatefulStorage::new_spanner(
                SpannerStatefulStore::new(db.clone()),
                SpannerReadStore::new(db.clone()),
            ),
            BackendKind::Sqlite | BackendKind::Postgres => {
                DefaultStatefulStorage::new_sql(SqlStatefulStore::new(db.clone()), read_store)
            }
        });

        let kv_backend = derive_kv_backend(config, db.backend())?;
        let kv = match kv_backend.as_str() {
            "memory" => {
                DefaultKvStore::Memory(MemoryKvStore::new(db.clone(), session_max_age_secs))
            }
            "postgres_unlogged" => {
                if db.backend() != BackendKind::Postgres {
                    anyhow::bail!(
                        "storage.kv.backend = \"postgres_unlogged\" requires a Postgres stateful store"
                    );
                }
                let kv_db = resolve_role_db(
                    &config.kv.url,
                    &config.stateful.url,
                    &db,
                    &mut opened_role_dbs,
                )
                .await?;
                let authoritative_db =
                    if role_uses_stateful_db(&config.kv.url, &config.stateful.url) {
                        None
                    } else {
                        Some(db.clone())
                    };
                DefaultKvStore::Sql(SqlKvStore::new(
                    kv_db,
                    authoritative_db,
                    session_max_age_secs,
                ))
            }
            "shared_sql" => match db.backend() {
                BackendKind::Spanner => {
                    DefaultKvStore::Spanner(SpannerKvStore::new(db.clone(), session_max_age_secs))
                }
                BackendKind::Sqlite | BackendKind::Postgres => {
                    let kv_db = resolve_role_db(
                        &config.kv.url,
                        &config.stateful.url,
                        &db,
                        &mut opened_role_dbs,
                    )
                    .await?;
                    let authoritative_db =
                        if role_uses_stateful_db(&config.kv.url, &config.stateful.url) {
                            None
                        } else {
                            Some(db.clone())
                        };
                    DefaultKvStore::Sql(SqlKvStore::new(
                        kv_db,
                        authoritative_db,
                        session_max_age_secs,
                    ))
                }
            },
            "redis" => anyhow::bail!(
                "storage.kv.backend = \"redis\" is not implemented yet in this POC runtime"
            ),
            other => anyhow::bail!("unsupported storage.kv.backend: {other}"),
        };

        let sink_backend = derive_sink_backend(config, db.backend())?;
        let flush_interval = parse_duration(&config.sink.flush_interval);
        let sink = match sink_backend.as_str() {
            "channel" => DefaultSink::Channel(ChannelSink::new(
                db.clone(),
                config.sink.buffer_size as usize,
                config.sink.batch_size as usize,
                flush_interval,
            )),
            "postgres" => {
                if db.backend() != BackendKind::Postgres {
                    anyhow::bail!(
                        "storage.sink.backend = \"postgres\" requires a Postgres stateful store"
                    );
                }
                let buffer_db = resolve_role_db(
                    &config.sink.url,
                    &config.stateful.url,
                    &db,
                    &mut opened_role_dbs,
                )
                .await?;
                DefaultSink::Sql(
                    SqlSink::new(
                        buffer_db,
                        db.clone(),
                        config.sink.batch_size as usize,
                        flush_interval,
                    )
                    .await?,
                )
            }
            "redis" => anyhow::bail!(
                "storage.sink.backend = \"redis\" is not implemented yet in this POC runtime"
            ),
            "noop" => DefaultSink::Noop(crate::NoopSink),
            other => anyhow::bail!("unsupported storage.sink.backend: {other}"),
        };

        let process_cache_backend = if config.process_cache.backend.is_empty() {
            "memory".to_string()
        } else {
            config.process_cache.backend.clone()
        };
        if process_cache_backend != "memory" {
            anyhow::bail!(
                "storage.process_cache.backend = \"{process_cache_backend}\" is not implemented yet"
            );
        }

        let analytics_backend = derive_analytics_backend(config)?;
        let analytics = Arc::new(match db.backend() {
            BackendKind::Spanner => DefaultAnalyticsStorage::new_spanner(
                NoopAnalyticsSink,
                SpannerAnalyticsQueryBackend::new(db.clone()),
            ),
            BackendKind::Sqlite | BackendKind::Postgres => DefaultAnalyticsStorage::new_sql(
                NoopAnalyticsSink,
                SqlAnalyticsQueryBackend::new(db.clone()),
            ),
        });

        Ok(Self {
            stateful,
            transient: Arc::new(DefaultTransientStorage::new(kv, sink)),
            analytics,
            roles: StorageRoleSummary {
                stateful: stateful_backend,
                read: read_backend,
                kv: kv_backend,
                sink: sink_backend,
                process_cache: process_cache_backend,
                analytics: analytics_backend,
            },
        })
    }
}

pub async fn prepare_postgres_role_databases(
    config: &StorageConfig,
    stateful_db: &Db,
) -> anyhow::Result<()> {
    if stateful_db.backend() != BackendKind::Postgres {
        return Ok(());
    }

    let mut opened_role_dbs = HashMap::new();

    let read_backend = derive_read_backend(config, BackendKind::Postgres)?;
    if read_backend == "postgres_replica" {
        if config.read.url.is_empty() {
            anyhow::bail!(
                "storage.read.url is required when storage.read.backend = \"postgres_replica\""
            );
        }
        if !role_uses_stateful_db(&config.read.url, &config.stateful.url) {
            tracing::info!(role = "read", url = %config.read.url, "skipping schema preparation for read replica");
        }
    }

    let kv_backend = derive_kv_backend(config, BackendKind::Postgres)?;
    if kv_backend == "postgres_unlogged"
        && !role_uses_stateful_db(&config.kv.url, &config.stateful.url)
    {
        let kv_db = resolve_role_db(
            &config.kv.url,
            &config.stateful.url,
            stateful_db,
            &mut opened_role_dbs,
        )
        .await?;
        prepare_postgres_kv_schema(&kv_db, true).await?;
    }

    let sink_backend = derive_sink_backend(config, BackendKind::Postgres)?;
    if sink_backend == "postgres" && !role_uses_stateful_db(&config.sink.url, &config.stateful.url)
    {
        let sink_db = resolve_role_db(
            &config.sink.url,
            &config.stateful.url,
            stateful_db,
            &mut opened_role_dbs,
        )
        .await?;
        prepare_postgres_sink_schema(&sink_db).await?;
    }

    Ok(())
}

fn derive_read_backend(config: &StorageConfig, backend: BackendKind) -> anyhow::Result<String> {
    let capabilities = storage_backend_capabilities(backend);
    if config.read.backend.is_empty() {
        return Ok(capabilities.default_read_backend.to_string());
    }

    match config.read.backend.as_str() {
        "same_connection" if capabilities.default_read_backend == "same_connection" => {
            Ok("same_connection".into())
        }
        "same_primary" if capabilities.default_read_backend == "same_primary" => {
            Ok("same_primary".into())
        }
        "postgres_replica" if backend == BackendKind::Postgres => Ok("postgres_replica".into()),
        other => anyhow::bail!("unsupported storage.read.backend for this stateful store: {other}"),
    }
}

fn derive_kv_backend(config: &StorageConfig, backend: BackendKind) -> anyhow::Result<String> {
    let capabilities = storage_backend_capabilities(backend);
    if config.kv.backend.is_empty() {
        return Ok(capabilities.default_kv_backend.to_string());
    }

    match config.kv.backend.as_str() {
        "memory" if capabilities.supports_memory_kv => Ok("memory".into()),
        "postgres_unlogged" if capabilities.supports_postgres_unlogged_kv => {
            Ok("postgres_unlogged".into())
        }
        "shared_sql" if capabilities.supports_shared_sql_kv => Ok("shared_sql".into()),
        "redis" => Ok("redis".into()),
        "memory" => anyhow::bail!(
            "storage.kv.backend = \"memory\" is not supported for native Spanner; use \"shared_sql\""
        ),
        other => anyhow::bail!("unsupported storage.kv.backend: {other}"),
    }
}

fn derive_sink_backend(config: &StorageConfig, backend: BackendKind) -> anyhow::Result<String> {
    let capabilities = storage_backend_capabilities(backend);
    if config.sink.backend.is_empty() {
        return Ok(capabilities.default_sink_backend.to_string());
    }

    match config.sink.backend.as_str() {
        "channel" if capabilities.supports_channel_sink => Ok("channel".into()),
        "postgres" if capabilities.supports_postgres_sink => Ok("postgres".into()),
        "redis" => Ok("redis".into()),
        "noop" if capabilities.supports_noop_sink => Ok("noop".into()),
        "channel" => anyhow::bail!(
            "storage.sink.backend = \"channel\" is not supported for native Spanner; use \"noop\""
        ),
        other => anyhow::bail!("unsupported storage.sink.backend: {other}"),
    }
}

fn derive_analytics_backend(config: &StorageConfig) -> anyhow::Result<String> {
    if config.analytics.backend.is_empty() {
        return Ok("same_stateful".into());
    }

    match config.analytics.backend.as_str() {
        "same_stateful" | "same_db" | "inherit" => Ok(config.analytics.backend.clone()),
        other => anyhow::bail!(
            "storage.analytics.backend = \"{other}\" is not implemented yet in this POC runtime"
        ),
    }
}

fn parse_duration(raw: &str) -> std::time::Duration {
    if let Some(value) = raw.strip_suffix("ms") {
        return std::time::Duration::from_millis(value.parse().unwrap_or(100));
    }
    if let Some(value) = raw.strip_suffix('s') {
        return std::time::Duration::from_secs(value.parse().unwrap_or(1));
    }
    if let Some(value) = raw.strip_suffix('m') {
        return std::time::Duration::from_secs(value.parse::<u64>().unwrap_or(1) * 60);
    }
    std::time::Duration::from_millis(100)
}

fn role_uses_stateful_db(role_url: &str, stateful_url: &str) -> bool {
    role_url.is_empty() || role_url == stateful_url
}

async fn resolve_role_db(
    role_url: &str,
    stateful_url: &str,
    stateful_db: &Db,
    opened_role_dbs: &mut HashMap<String, Db>,
) -> anyhow::Result<Db> {
    if role_uses_stateful_db(role_url, stateful_url) {
        return Ok(stateful_db.clone());
    }

    if let Some(db) = opened_role_dbs.get(role_url) {
        return Ok(db.clone());
    }

    let db = Db::open(role_url).await?;
    opened_role_dbs.insert(role_url.to_string(), db.clone());
    Ok(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sqlite_defaults_derive_memory_kv_and_channel_sink() {
        let db = Db::open("").await.unwrap();
        let runtime = StorageRuntime::from_config(&StorageConfig::default(), db, 86_400)
            .await
            .unwrap();

        assert_eq!(runtime.roles.stateful, "sqlite");
        assert_eq!(runtime.roles.read, "same_connection");
        assert_eq!(runtime.roles.kv, "memory");
        assert_eq!(runtime.roles.sink, "channel");
        assert_eq!(runtime.roles.process_cache, "memory");
        assert_eq!(runtime.roles.analytics, "same_stateful");
    }

    #[tokio::test]
    async fn postgres_like_config_selects_postgres_defaults() {
        let config = StorageConfig {
            stateful: zitadel_config::StatefulStorageConfig {
                backend: "postgres".into(),
                url: "postgres://user:pass@localhost/zitadel".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let roles = StorageRoleSummary {
            stateful: "postgres".into(),
            read: derive_read_backend(&config, BackendKind::Postgres).unwrap(),
            kv: derive_kv_backend(&config, BackendKind::Postgres).unwrap(),
            sink: derive_sink_backend(&config, BackendKind::Postgres).unwrap(),
            process_cache: "memory".into(),
            analytics: derive_analytics_backend(&config).unwrap(),
        };

        assert_eq!(roles.stateful, "postgres");
        assert_eq!(roles.read, "same_primary");
        assert_eq!(roles.kv, "postgres_unlogged");
        assert_eq!(roles.sink, "postgres");
    }

    #[test]
    fn spanner_like_config_selects_shared_sql_and_noop_sink() {
        let config = StorageConfig {
            stateful: zitadel_config::StatefulStorageConfig {
                backend: "spanner".into(),
                url: "postgres://localhost/spanner".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let roles = StorageRoleSummary {
            stateful: "spanner".into(),
            read: derive_read_backend(&config, BackendKind::Spanner).unwrap(),
            kv: derive_kv_backend(&config, BackendKind::Spanner).unwrap(),
            sink: derive_sink_backend(&config, BackendKind::Spanner).unwrap(),
            process_cache: "memory".into(),
            analytics: derive_analytics_backend(&config).unwrap(),
        };

        assert_eq!(roles.read, "same_primary");
        assert_eq!(roles.kv, "shared_sql");
        assert_eq!(roles.sink, "noop");
    }

    #[test]
    fn spanner_rejects_memory_kv_override() {
        let config = StorageConfig {
            kv: zitadel_config::KvStoreConfig {
                backend: "memory".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let error = derive_kv_backend(&config, BackendKind::Spanner)
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains("storage.kv.backend = \"memory\" is not supported for native Spanner")
        );
    }

    #[test]
    fn spanner_rejects_channel_sink_override() {
        let config = StorageConfig {
            sink: zitadel_config::SinkConfig {
                backend: "channel".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let error = derive_sink_backend(&config, BackendKind::Spanner)
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains("storage.sink.backend = \"channel\" is not supported for native Spanner")
        );
    }

    #[tokio::test]
    async fn redis_overrides_fail_clearly() {
        let db = Db::open("").await.unwrap();
        let config = StorageConfig {
            kv: zitadel_config::KvStoreConfig {
                backend: "redis".into(),
                url: "redis://localhost:6379".into(),
            },
            ..Default::default()
        };

        let error = StorageRuntime::from_config(&config, db, 86_400)
            .await
            .err()
            .unwrap();
        assert!(
            error
                .to_string()
                .contains("storage.kv.backend = \"redis\" is not implemented yet")
        );
    }

    #[test]
    fn empty_or_matching_role_urls_reuse_stateful_db() {
        assert!(role_uses_stateful_db("", "postgres://primary"));
        assert!(role_uses_stateful_db(
            "postgres://primary",
            "postgres://primary"
        ));
        assert!(!role_uses_stateful_db(
            "postgres://kv",
            "postgres://primary"
        ));
    }
}
