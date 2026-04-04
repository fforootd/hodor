use std::{collections::HashMap, sync::Arc};

use zitadel_config::StorageConfig;
use zitadel_db::{Db, Dialect};

use crate::{
    ChannelSink, DefaultAnalyticsStorage, DefaultKvStore, DefaultSink, DefaultStatefulStorage,
    DefaultTransientStorage, MemoryKvStore, NoopAnalyticsSink, SqlAnalyticsQueryBackend,
    SqlKvStore, SqlReadStore, SqlSink, SqlStatefulStore, prepare_postgres_kv_schema,
    prepare_postgres_sink_schema,
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
        let stateful_backend = match db.dialect() {
            Dialect::Sqlite => "sqlite",
            Dialect::Postgres => "postgres",
        }
        .to_string();

        let read_backend = derive_read_backend(config, db.dialect())?;
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

        let stateful = Arc::new(DefaultStatefulStorage::new(
            SqlStatefulStore::new(db.clone()),
            read_store,
        ));

        let kv_backend = derive_kv_backend(config, db.dialect())?;
        let kv = match kv_backend.as_str() {
            "memory" => {
                DefaultKvStore::Memory(MemoryKvStore::new(db.clone(), session_max_age_secs))
            }
            "postgres_unlogged" => {
                if db.dialect() != Dialect::Postgres {
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
            "redis" => anyhow::bail!(
                "storage.kv.backend = \"redis\" is not implemented yet in this POC runtime"
            ),
            other => anyhow::bail!("unsupported storage.kv.backend: {other}"),
        };

        let sink_backend = derive_sink_backend(config, db.dialect())?;
        let flush_interval = parse_duration(&config.sink.flush_interval);
        let sink = match sink_backend.as_str() {
            "channel" => DefaultSink::Channel(ChannelSink::new(
                db.clone(),
                config.sink.buffer_size as usize,
                config.sink.batch_size as usize,
                flush_interval,
            )),
            "postgres" => {
                if db.dialect() != Dialect::Postgres {
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
        let analytics = Arc::new(DefaultAnalyticsStorage::new(
            NoopAnalyticsSink,
            SqlAnalyticsQueryBackend::new(db.clone()),
        ));

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
    if stateful_db.dialect() != Dialect::Postgres {
        return Ok(());
    }

    let mut opened_role_dbs = HashMap::new();

    let read_backend = derive_read_backend(config, Dialect::Postgres)?;
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

    let kv_backend = derive_kv_backend(config, Dialect::Postgres)?;
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

    let sink_backend = derive_sink_backend(config, Dialect::Postgres)?;
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

fn derive_read_backend(config: &StorageConfig, dialect: Dialect) -> anyhow::Result<String> {
    if config.read.backend.is_empty() {
        return Ok(match dialect {
            Dialect::Sqlite => "same_connection",
            Dialect::Postgres => "same_primary",
        }
        .to_string());
    }

    match config.read.backend.as_str() {
        "same_connection" if dialect == Dialect::Sqlite => Ok("same_connection".into()),
        "same_primary" if dialect == Dialect::Postgres => Ok("same_primary".into()),
        "postgres_replica" if dialect == Dialect::Postgres => Ok("postgres_replica".into()),
        other => anyhow::bail!("unsupported storage.read.backend for this stateful store: {other}"),
    }
}

fn derive_kv_backend(config: &StorageConfig, dialect: Dialect) -> anyhow::Result<String> {
    if config.kv.backend.is_empty() {
        return Ok(match dialect {
            Dialect::Sqlite => "memory",
            Dialect::Postgres => "postgres_unlogged",
        }
        .to_string());
    }

    match config.kv.backend.as_str() {
        "memory" => Ok("memory".into()),
        "postgres_unlogged" => Ok("postgres_unlogged".into()),
        "redis" => Ok("redis".into()),
        other => anyhow::bail!("unsupported storage.kv.backend: {other}"),
    }
}

fn derive_sink_backend(config: &StorageConfig, dialect: Dialect) -> anyhow::Result<String> {
    if config.sink.backend.is_empty() {
        return Ok(match dialect {
            Dialect::Sqlite => "channel",
            Dialect::Postgres => "postgres",
        }
        .to_string());
    }

    match config.sink.backend.as_str() {
        "channel" => Ok("channel".into()),
        "postgres" => Ok("postgres".into()),
        "redis" => Ok("redis".into()),
        "noop" => Ok("noop".into()),
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
                url: "postgres://user:pass@localhost/zitadel".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        let roles = StorageRoleSummary {
            stateful: "postgres".into(),
            read: derive_read_backend(&config, Dialect::Postgres).unwrap(),
            kv: derive_kv_backend(&config, Dialect::Postgres).unwrap(),
            sink: derive_sink_backend(&config, Dialect::Postgres).unwrap(),
            process_cache: "memory".into(),
            analytics: derive_analytics_backend(&config).unwrap(),
        };

        assert_eq!(roles.stateful, "postgres");
        assert_eq!(roles.read, "same_primary");
        assert_eq!(roles.kv, "postgres_unlogged");
        assert_eq!(roles.sink, "postgres");
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
