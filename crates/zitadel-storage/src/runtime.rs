use std::{collections::HashMap, sync::Arc};

use zitadel_config::{DatabaseConnectConfig, StorageConfig};
use zitadel_db::{BackendKind, Db};

use crate::{
    DefaultAnalyticsStorage, DefaultKvStore, DefaultPrimaryStorage, DefaultSink,
    DefaultTransientStorage, NoopAnalyticsSink, NoopSink, SpannerAnalyticsQueryBackend,
    SpannerKvStore, SpannerReadStore, SpannerStatefulStore, SqlAnalyticsQueryBackend, SqlKvStore,
    SqlReadStore, SqlStatefulStore,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StorageRoleSummary {
    pub primary: String,
    pub transient: String,
    pub analytics: String,
}

pub struct StorageRuntime {
    pub primary: Arc<DefaultPrimaryStorage>,
    pub transient: Arc<DefaultTransientStorage>,
    pub analytics: Arc<DefaultAnalyticsStorage>,
    pub roles: StorageRoleSummary,
}

impl StorageRuntime {
    pub async fn from_config(
        config: &StorageConfig,
        primary_db: Db,
        session_max_age_secs: u64,
    ) -> anyhow::Result<Self> {
        let mut opened_role_dbs = HashMap::new();
        let primary_backend = primary_db.backend().to_string();

        let primary =
            Arc::new(build_primary_storage(config, &primary_db, &mut opened_role_dbs).await?);
        let transient = Arc::new(
            build_transient_storage(
                config,
                &primary_db,
                &mut opened_role_dbs,
                session_max_age_secs,
            )
            .await?,
        );
        let analytics_db = resolve_analytics_db(config, &primary_db, &mut opened_role_dbs).await?;
        let analytics_backend = analytics_db.backend().to_string();
        let analytics = Arc::new(match analytics_db.backend() {
            BackendKind::Spanner => DefaultAnalyticsStorage::new_spanner(
                NoopAnalyticsSink,
                SpannerAnalyticsQueryBackend::new(analytics_db),
            ),
            BackendKind::Sqlite | BackendKind::Postgres => DefaultAnalyticsStorage::new_sql(
                NoopAnalyticsSink,
                SqlAnalyticsQueryBackend::new(analytics_db),
            ),
        });

        Ok(Self {
            primary,
            transient,
            analytics,
            roles: StorageRoleSummary {
                primary: primary_backend,
                transient: derive_transient_role(config, primary_db.backend()),
                analytics: analytics_backend,
            },
        })
    }
}

pub async fn prepare_auxiliary_databases(
    config: &StorageConfig,
    primary_db: &Db,
) -> anyhow::Result<()> {
    let mut opened = HashMap::new();

    if let Some(transient_db) = resolve_transient_db(config, primary_db, &mut opened).await? {
        if !same_db(&transient_db, primary_db) {
            match transient_db.backend() {
                BackendKind::Sqlite | BackendKind::Postgres => {
                    zitadel_db::migrate::migrate(&transient_db).await?;
                }
                BackendKind::Spanner => {
                    anyhow::bail!(
                        "separate native Spanner transient databases are not supported in this POC"
                    );
                }
            }
        }
    }

    let analytics_db = resolve_analytics_db(config, primary_db, &mut opened).await?;
    if !same_db(&analytics_db, primary_db) {
        match analytics_db.backend() {
            BackendKind::Sqlite | BackendKind::Postgres => {
                zitadel_db::migrate::migrate(&analytics_db).await?;
            }
            BackendKind::Spanner => {
                anyhow::bail!(
                    "separate native Spanner analytics databases are not supported in this POC"
                );
            }
        }
    }

    Ok(())
}

pub async fn open_analytics_db(config: &StorageConfig, primary_db: &Db) -> anyhow::Result<Db> {
    let mut opened = HashMap::new();
    let analytics_db = resolve_analytics_db(config, primary_db, &mut opened).await?;
    if !same_db(&analytics_db, primary_db) {
        match analytics_db.backend() {
            BackendKind::Sqlite | BackendKind::Postgres => {
                zitadel_db::migrate::migrate(&analytics_db).await?;
            }
            BackendKind::Spanner => {
                anyhow::bail!(
                    "separate native Spanner analytics databases are not supported in this POC"
                );
            }
        }
    }
    Ok(analytics_db)
}

async fn build_primary_storage(
    config: &StorageConfig,
    primary_db: &Db,
    opened_role_dbs: &mut HashMap<String, Db>,
) -> anyhow::Result<DefaultPrimaryStorage> {
    Ok(match primary_db.backend() {
        BackendKind::Spanner => DefaultPrimaryStorage::new_spanner(
            SpannerStatefulStore::new(primary_db.clone()),
            SpannerReadStore::new(primary_db.clone()),
        ),
        BackendKind::Sqlite | BackendKind::Postgres => {
            let read_store = SqlReadStore::new(primary_db.clone());
            let replica = if primary_db.backend() == BackendKind::Postgres
                && config.primary.replica.is_enabled()
                && config.primary.replica.resolve_mode() == "explicit"
            {
                Some(SqlReadStore::new(
                    resolve_role_db(
                        &config.primary.replica.url,
                        &config.primary.url,
                        primary_db,
                        opened_role_dbs,
                    )
                    .await?,
                ))
            } else {
                None
            };

            match replica {
                Some(replica) => DefaultPrimaryStorage::new_sql_with_replica(
                    SqlStatefulStore::new(primary_db.clone()),
                    read_store,
                    replica,
                ),
                None => DefaultPrimaryStorage::new_sql(
                    SqlStatefulStore::new(primary_db.clone()),
                    read_store,
                ),
            }
        }
    })
}

async fn build_transient_storage(
    config: &StorageConfig,
    primary_db: &Db,
    opened_role_dbs: &mut HashMap<String, Db>,
    session_max_age_secs: u64,
) -> anyhow::Result<DefaultTransientStorage> {
    if let Some(transient_db) = resolve_transient_db(config, primary_db, opened_role_dbs).await? {
        return Ok(match transient_db.backend() {
            BackendKind::Spanner => {
                if primary_db.backend() != BackendKind::Spanner {
                    anyhow::bail!(
                        "storage.transient.backend = \"spanner\" requires storage.primary.backend = \"spanner\""
                    );
                }
                DefaultTransientStorage::new(
                    DefaultKvStore::Spanner(SpannerKvStore::new(
                        transient_db,
                        session_max_age_secs,
                    )),
                    DefaultSink::Noop(NoopSink),
                )
            }
            BackendKind::Sqlite | BackendKind::Postgres => DefaultTransientStorage::new(
                DefaultKvStore::Sql(SqlKvStore::new(
                    transient_db,
                    Some(primary_db.clone()),
                    session_max_age_secs,
                )),
                DefaultSink::Noop(NoopSink),
            ),
        });
    }

    Ok(match primary_db.backend() {
        BackendKind::Spanner => DefaultTransientStorage::new(
            DefaultKvStore::Spanner(SpannerKvStore::new(
                primary_db.clone(),
                session_max_age_secs,
            )),
            DefaultSink::Noop(NoopSink),
        ),
        BackendKind::Sqlite | BackendKind::Postgres => DefaultTransientStorage::new(
            DefaultKvStore::Sql(SqlKvStore::local_only(
                primary_db.clone(),
                session_max_age_secs,
            )),
            DefaultSink::Noop(NoopSink),
        ),
    })
}

async fn resolve_transient_db(
    config: &StorageConfig,
    primary_db: &Db,
    opened_role_dbs: &mut HashMap<String, Db>,
) -> anyhow::Result<Option<Db>> {
    if config.transient.inherits_primary() {
        return Ok(None);
    }

    let transient_db = resolve_storage_db(
        &config.transient,
        &config.primary.url,
        primary_db,
        opened_role_dbs,
    )
    .await?;
    Ok(Some(transient_db))
}

async fn resolve_analytics_db(
    config: &StorageConfig,
    primary_db: &Db,
    opened_role_dbs: &mut HashMap<String, Db>,
) -> anyhow::Result<Db> {
    if config.analytics.inherits_primary() {
        return Ok(primary_db.clone());
    }

    resolve_storage_db(
        &config.analytics,
        &config.primary.url,
        primary_db,
        opened_role_dbs,
    )
    .await
}

async fn resolve_storage_db<T: DatabaseConnectConfig>(
    config: &T,
    primary_url: &str,
    primary_db: &Db,
    opened_role_dbs: &mut HashMap<String, Db>,
) -> anyhow::Result<Db> {
    let role_url = config.url();
    if role_uses_primary_db(role_url, primary_url) {
        return Ok(primary_db.clone());
    }

    if let Some(db) = opened_role_dbs.get(role_url) {
        return Ok(db.clone());
    }

    let db = Db::open_with_config::<T>(role_url, config).await?;
    opened_role_dbs.insert(role_url.to_string(), db.clone());
    Ok(db)
}

async fn resolve_role_db(
    role_url: &str,
    primary_url: &str,
    primary_db: &Db,
    opened_role_dbs: &mut HashMap<String, Db>,
) -> anyhow::Result<Db> {
    if role_uses_primary_db(role_url, primary_url) {
        return Ok(primary_db.clone());
    }

    if let Some(db) = opened_role_dbs.get(role_url) {
        return Ok(db.clone());
    }

    let db = Db::open(role_url).await?;
    opened_role_dbs.insert(role_url.to_string(), db.clone());
    Ok(db)
}

fn role_uses_primary_db(role_url: &str, primary_url: &str) -> bool {
    role_url.is_empty() || role_url == primary_url
}

fn same_db(left: &Db, right: &Db) -> bool {
    match (left, right) {
        (Db::Sql(left), Db::Sql(right)) => std::ptr::eq(left.pool(), right.pool()),
        (Db::Spanner(left), Db::Spanner(right)) => left.database_name() == right.database_name(),
        _ => false,
    }
}

fn derive_transient_role(config: &StorageConfig, primary_backend: BackendKind) -> String {
    if config.transient.inherits_primary() {
        return format!("inherit({primary_backend})");
    }

    config.transient.resolve_backend().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sqlite_defaults_inherit_primary() {
        let db = Db::open("").await.unwrap();
        let runtime = StorageRuntime::from_config(&StorageConfig::default(), db, 86_400)
            .await
            .unwrap();

        assert_eq!(runtime.roles.primary, "sqlite");
        assert_eq!(runtime.roles.transient, "inherit(sqlite)");
        assert_eq!(runtime.roles.analytics, "sqlite");
    }

    #[test]
    fn transient_role_reports_explicit_backend() {
        let config = StorageConfig {
            transient: zitadel_config::TransientStorageConfig {
                backend: "postgres".into(),
                url: "postgres://transient/zitadel".into(),
                ..Default::default()
            },
            ..Default::default()
        };

        assert_eq!(
            derive_transient_role(&config, BackendKind::Postgres),
            "postgres"
        );
    }

    #[test]
    fn empty_or_matching_role_urls_reuse_primary_db() {
        assert!(role_uses_primary_db("", "postgres://primary"));
        assert!(role_uses_primary_db(
            "postgres://primary",
            "postgres://primary"
        ));
        assert!(!role_uses_primary_db(
            "postgres://transient",
            "postgres://primary"
        ));
    }
}
