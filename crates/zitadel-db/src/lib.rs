pub mod bootstrap;
pub mod context;
pub mod features;
pub mod job_runtime;
pub mod migrate;
pub mod provider;
pub mod repo_impls;
pub mod repos;
pub mod retained;
pub mod scoped;
pub mod seed;
pub mod spanner;

use anyhow::Context;
use sqlx::{AnyPool, any::AnyPoolOptions};
use std::fmt;
use std::time::Duration;

pub const DEFAULT_INSTANCE_ID: &str = "default";
pub const DEFAULT_ORG_ID: &str = "1";
pub use context::{
    InstanceContext, current_instance_context, current_instance_id, current_instance_id_or,
    current_request_origin, current_request_origin_or, with_instance_context,
};
pub use features::{
    FeatureMap, feature_enabled, merge_feature_overrides, validate_feature_overrides,
};
pub use job_runtime::{
    JobBudget, JobReconcileSpec, bool_true_sql, complete_job_run, current_timestamp_sql,
    delete_sink_inbox_records, delete_terminal_sessions_records, delete_terminal_tokens_records,
    delete_transient_state_records, due_job_names, ensure_event_partitions,
    event_table_is_partitioned, maintain_event_storage, reconcile_jobs, timestamp_plus_expr,
    try_acquire_job_lease,
};
pub use retained::{
    ActionRecord, ActiveRoleBindingRecord, ChildInstanceOwnershipRecord, ConsoleBootstrapData, CreateManagedInstanceInput,
    DomainDeleteOutcome, DomainRecord, FingerprintRecord, IdentityMetadata, InstanceMetadata,
    JobRecord, LinkedIdentityRecord, LoginFlowRecord, ManagedInstancePatch, ManagedInstanceRecord,
    MembershipRow, NamedResourceRecord, OidcAuthRequestRecord, OidcClientRecord, OrgRecord,
    OrgRoleMembershipRecord, OrgSummary, OrgUserLinkRecord, PatRecord, RouteResolutionRecord,
    SavedQueryRecord, SchemaRegistryRecord, SearchRecord, SettingsRecord,
    UnshippedEventRecord, UserClaimsRecord, UserRecord, add_instance_domain, add_membership,
    append_event, consume_oidc_auth_code_record, count_users_for_schema,
    create_linked_identity_record, create_login_flow,
    create_managed_instance, create_named_resource, create_oidc_auth_request_record, create_org,
    create_pat, create_role_assignment, create_saved_query, create_schema_record, create_user,
    delete_instance_domain, delete_instance_row, delete_provider, delete_saved_query,
    delete_settings_record, deprovision_managed_instance, fetch_unshipped_events,
    find_active_user_by_identifier, find_linked_identity, first_org_id, get_action,
    get_instance_trust_link, get_login_flow_record, get_managed_instance, get_named_resource,
    get_oidc_client_record, get_org, get_role_assignment, get_schema_record,
    get_settings_record, get_user, instance_visible, list_actions,
    list_active_child_instance_ownerships, list_active_org_role_memberships,
    list_active_org_users, list_admin_instances, list_fingerprints, list_instance_domains,
    list_active_role_bindings_for_scope, list_jobs_for_instance, list_login_flow_records,
    list_managed_instances, list_memberships, list_named_resources, list_org_records,
    list_pats_for_instance, list_role_assignments, list_role_definitions, list_saved_queries, list_schema_registry, list_users,
    load_console_bootstrap_data, load_entity_counts, load_identity_metadata,
    load_instance_metadata, load_session_user_profile, load_user_claims_record,
    mark_events_shipped, promote_schema_record, put_instance_settings, remove_membership,
    replace_password_credential, resolve_domain_route, resolve_instance_route,
    resolve_login_flow, revoke_pat, revoke_role_assignment, search_records,
    seed_builtin_role_definitions, set_login_flow_state, touch_linked_identity,
    update_login_flow, update_managed_instance, update_named_resource_name, update_org,
    update_password_hash, update_schema_record, update_session_metadata, update_user,
    upsert_catalog_action, upsert_fingerprint, user_has_capability,
};
pub use spanner::{ParsedDatabaseName, SpannerDb};

/// Supported SQL dialects and native backend syntaxes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Sqlite,
    Postgres,
    Spanner,
}

impl Dialect {
    /// Whether this dialect uses SQLite-compatible SQL syntax.
    pub fn is_sqlite_compat(self) -> bool {
        matches!(self, Dialect::Sqlite)
    }
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dialect::Sqlite => write!(f, "sqlite"),
            Dialect::Postgres => write!(f, "postgres"),
            Dialect::Spanner => write!(f, "spanner"),
        }
    }
}

/// Backend semantics layered on top of the SQL dialect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Sqlite,
    Postgres,
    Spanner,
}

impl BackendKind {
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        match raw {
            "sqlite" => Ok(Self::Sqlite),
            "postgres" => Ok(Self::Postgres),
            "spanner" => Ok(Self::Spanner),
            other => anyhow::bail!("unsupported storage.stateful.backend: {other}"),
        }
    }

    pub fn dialect(self) -> Dialect {
        match self {
            Self::Sqlite => Dialect::Sqlite,
            Self::Postgres => Dialect::Postgres,
            Self::Spanner => Dialect::Spanner,
        }
    }
}

impl fmt::Display for BackendKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BackendKind::Sqlite => write!(f, "sqlite"),
            BackendKind::Postgres => write!(f, "postgres"),
            BackendKind::Spanner => write!(f, "spanner"),
        }
    }
}

#[derive(Clone)]
pub struct SqlDb {
    pool: AnyPool,
    dialect: Dialect,
    backend: BackendKind,
    max_connections: u32,
    max_idle_connections: u32,
    conn_max_lifetime: Option<Duration>,
}

/// Database handle for SQL or native Spanner backends.
#[derive(Clone)]
pub enum Db {
    Sql(SqlDb),
    Spanner(SpannerDb),
}

impl Db {
    /// Open a SQL database connection based on the connection string.
    pub async fn open(conn_str: &str) -> anyhow::Result<Self> {
        let (dialect, url) = parse_connection_string(conn_str)?;
        let backend = infer_backend(dialect);
        let sql = open_sql(url, dialect, backend, None).await?;
        Ok(Self::Sql(sql))
    }

    /// Open with explicit backend settings from config.
    pub async fn open_with_config(
        conn_str: &str,
        config: &zitadel_config::StatefulStorageConfig,
    ) -> anyhow::Result<Self> {
        let backend = BackendKind::parse(config.resolve_backend())?;
        match backend {
            BackendKind::Spanner => {
                if config.database.is_empty() {
                    anyhow::bail!(
                        "storage.stateful.database is required when storage.stateful.backend = \"spanner\""
                    );
                }
                Ok(Self::Spanner(SpannerDb::open(config).await?))
            }
            BackendKind::Sqlite | BackendKind::Postgres => {
                let (dialect, url) = parse_connection_string(conn_str)?;
                validate_backend_dialect(backend, dialect)?;
                let sql = open_sql(url, dialect, backend, Some(config)).await?;
                Ok(Self::Sql(sql))
            }
        }
    }

    pub fn pool(&self) -> &AnyPool {
        match self {
            Db::Sql(sql) => sql.pool(),
            Db::Spanner(_) => panic!("native Spanner backend does not expose sqlx::AnyPool"),
        }
    }

    pub fn sql(&self) -> Option<&SqlDb> {
        match self {
            Db::Sql(sql) => Some(sql),
            Db::Spanner(_) => None,
        }
    }

    pub fn sql_pool(&self) -> Option<&AnyPool> {
        match self {
            Db::Sql(sql) => Some(sql.pool()),
            Db::Spanner(_) => None,
        }
    }

    pub fn spanner(&self) -> Option<&SpannerDb> {
        match self {
            Db::Spanner(spanner) => Some(spanner),
            Db::Sql(_) => None,
        }
    }

    pub fn dialect(&self) -> Dialect {
        match self {
            Db::Sql(sql) => sql.dialect,
            Db::Spanner(_) => Dialect::Spanner,
        }
    }

    pub fn backend(&self) -> BackendKind {
        match self {
            Db::Sql(sql) => sql.backend,
            Db::Spanner(_) => BackendKind::Spanner,
        }
    }

    /// ScopedDb bound to the default instance ID (for startup operations).
    pub fn scoped_default(&self) -> scoped::ScopedDb {
        match self {
            Db::Sql(sql) => sql.scoped(current_instance_id_or(DEFAULT_INSTANCE_ID).into_owned()),
            Db::Spanner(_) => {
                panic!("ScopedDb is SQL-only; use native Spanner store paths instead")
            }
        }
    }

    /// ScopedDb bound to a specific instance ID.
    pub fn scoped(&self, instance_id: String) -> scoped::ScopedDb {
        match self {
            Db::Sql(sql) => sql.scoped(instance_id),
            Db::Spanner(_) => {
                panic!("ScopedDb is SQL-only; use native Spanner store paths instead")
            }
        }
    }

    pub fn into_sql(self) -> anyhow::Result<SqlDb> {
        match self {
            Db::Sql(sql) => Ok(sql),
            Db::Spanner(_) => anyhow::bail!("expected SQL backend, got native Spanner"),
        }
    }

    pub fn into_spanner(self) -> anyhow::Result<SpannerDb> {
        match self {
            Db::Spanner(spanner) => Ok(spanner),
            Db::Sql(_) => anyhow::bail!("expected native Spanner backend, got SQL"),
        }
    }

    pub async fn close(&self) {
        match self.clone() {
            Db::Sql(sql) => sql.pool.close().await,
            Db::Spanner(spanner) => spanner.close().await,
        }
    }
}

async fn open_sql(
    url: String,
    dialect: Dialect,
    backend: BackendKind,
    config: Option<&zitadel_config::StatefulStorageConfig>,
) -> anyhow::Result<SqlDb> {
    sqlx::any::install_default_drivers();

    let is_memory = url == "sqlite::memory:";
    let max_conns = if is_memory {
        1
    } else if dialect == Dialect::Sqlite {
        16
    } else {
        config.map(|cfg| cfg.max_open_conns).unwrap_or(25)
    };
    let max_idle_conns = if is_memory {
        1
    } else {
        config
            .map(|cfg| cfg.max_idle_conns.max(1).min(max_conns))
            .unwrap_or(5)
            .min(max_conns)
    };
    let conn_max_lifetime = config
        .map(|cfg| parse_duration_setting(&cfg.conn_max_lifetime))
        .transpose()
        .context("parse storage.stateful.conn_max_lifetime")?
        .or(Some(Duration::from_secs(60 * 60)));

    let idle_connections = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
    let idle_release_counter = idle_connections.clone();
    let idle_acquire_counter = idle_connections.clone();

    let pool = AnyPoolOptions::new()
        .max_connections(max_conns)
        .max_lifetime(conn_max_lifetime)
        .after_release(move |_conn, _meta| {
            let idle_release_counter = idle_release_counter.clone();
            Box::pin(async move {
                let previous =
                    idle_release_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if previous >= max_idle_conns {
                    idle_release_counter.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                    return Ok(false);
                }
                Ok(true)
            })
        })
        .before_acquire(move |_conn, _meta| {
            let idle_acquire_counter = idle_acquire_counter.clone();
            Box::pin(async move {
                let _ = idle_acquire_counter.fetch_update(
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                    |value| value.checked_sub(1),
                );
                Ok(true)
            })
        })
        .connect(&url)
        .await?;

    if dialect == Dialect::Sqlite && !is_memory {
        sqlx::query("PRAGMA journal_mode = WAL")
            .execute(&pool)
            .await?;
    }
    if dialect == Dialect::Sqlite {
        sqlx::query("PRAGMA foreign_keys = ON")
            .execute(&pool)
            .await?;
        sqlx::query("PRAGMA busy_timeout = 5000")
            .execute(&pool)
            .await?;
    }

    Ok(SqlDb {
        pool,
        dialect,
        backend,
        max_connections: max_conns,
        max_idle_connections: max_idle_conns,
        conn_max_lifetime,
    })
}

impl SqlDb {
    pub fn pool(&self) -> &AnyPool {
        &self.pool
    }

    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    pub fn backend(&self) -> BackendKind {
        self.backend
    }

    pub fn scoped(&self, instance_id: String) -> scoped::ScopedDb {
        scoped::ScopedDb::new(self.pool.clone(), self.dialect, instance_id)
    }

    pub fn max_connections(&self) -> u32 {
        self.max_connections
    }

    pub fn max_idle_connections(&self) -> u32 {
        self.max_idle_connections
    }

    pub fn conn_max_lifetime(&self) -> Option<Duration> {
        self.conn_max_lifetime
    }
}

/// Parse connection string into (dialect, sqlx-compatible URL).
fn parse_connection_string(conn_str: &str) -> anyhow::Result<(Dialect, String)> {
    if conn_str.is_empty() || conn_str.starts_with("sqlite://") {
        let path = conn_str.strip_prefix("sqlite://").unwrap_or("").to_string();

        if !path.is_empty()
            && path != ":memory:"
            && let Some(parent) = std::path::Path::new(&path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        let url = if path.is_empty() || path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            let p = std::path::Path::new(&path);
            let normalized = if p.exists() {
                p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
            } else if let Some(parent) = p.parent() {
                if parent.exists() {
                    let canon_parent = parent
                        .canonicalize()
                        .unwrap_or_else(|_| parent.to_path_buf());
                    canon_parent.join(p.file_name().unwrap_or_default())
                } else {
                    p.to_path_buf()
                }
            } else {
                p.to_path_buf()
            };
            format!("sqlite:{}?mode=rwc", normalized.display())
        };
        Ok((Dialect::Sqlite, url))
    } else if conn_str.starts_with("postgres://") || conn_str.starts_with("postgresql://") {
        Ok((Dialect::Postgres, conn_str.to_string()))
    } else {
        anyhow::bail!("unsupported database URL scheme: {conn_str}")
    }
}

fn infer_backend(dialect: Dialect) -> BackendKind {
    match dialect {
        Dialect::Sqlite => BackendKind::Sqlite,
        Dialect::Postgres => BackendKind::Postgres,
        Dialect::Spanner => BackendKind::Spanner,
    }
}

fn validate_backend_dialect(backend: BackendKind, dialect: Dialect) -> anyhow::Result<()> {
    if backend.dialect() != dialect {
        anyhow::bail!(
            "storage.stateful.backend = \"{backend}\" requires a {} connection URL",
            backend.dialect()
        );
    }
    Ok(())
}

fn parse_duration_setting(raw: &str) -> anyhow::Result<Duration> {
    if raw.is_empty() {
        anyhow::bail!("duration must not be empty");
    }
    if let Some(value) = raw.strip_suffix("ms") {
        return value
            .parse::<u64>()
            .map(Duration::from_millis)
            .context("invalid millisecond duration");
    }
    if let Some(value) = raw.strip_suffix('s') {
        return value
            .parse::<u64>()
            .map(Duration::from_secs)
            .context("invalid second duration");
    }
    if let Some(value) = raw.strip_suffix('m') {
        return value
            .parse::<u64>()
            .map(|minutes| Duration::from_secs(minutes * 60))
            .context("invalid minute duration");
    }
    if let Some(value) = raw.strip_suffix('h') {
        return value
            .parse::<u64>()
            .map(|hours| Duration::from_secs(hours * 60 * 60))
            .context("invalid hour duration");
    }
    if let Some(value) = raw.strip_suffix('d') {
        return value
            .parse::<u64>()
            .map(|days| Duration::from_secs(days * 24 * 60 * 60))
            .context("invalid day duration");
    }
    anyhow::bail!("unsupported duration format `{raw}`")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sqlite_empty() {
        let (d, url) = parse_connection_string("").unwrap();
        assert_eq!(d, Dialect::Sqlite);
        assert_eq!(url, "sqlite::memory:");
    }

    #[test]
    fn parse_sqlite_path() {
        let (d, url) = parse_connection_string("sqlite://./data/test.db").unwrap();
        assert_eq!(d, Dialect::Sqlite);
        assert!(url.starts_with("sqlite:"), "url={url}");
        assert!(url.contains("data/test.db"), "url={url}");
    }

    #[test]
    fn parse_postgres() {
        let (d, url) = parse_connection_string("postgres://user:pass@localhost/db").unwrap();
        assert_eq!(d, Dialect::Postgres);
        assert_eq!(url, "postgres://user:pass@localhost/db");
    }

    #[test]
    fn parse_unsupported() {
        assert!(parse_connection_string("mysql://localhost/db").is_err());
    }

    #[test]
    fn validates_backend_dialect_pairs() {
        assert!(validate_backend_dialect(BackendKind::Postgres, Dialect::Postgres).is_ok());
        assert!(validate_backend_dialect(BackendKind::Sqlite, Dialect::Sqlite).is_ok());
        assert!(validate_backend_dialect(BackendKind::Spanner, Dialect::Postgres).is_err());
    }

    #[tokio::test]
    async fn open_in_memory_sqlite() {
        let db = Db::open("").await.unwrap();
        assert_eq!(db.dialect(), Dialect::Sqlite);
        assert_eq!(db.backend(), BackendKind::Sqlite);
        sqlx::query("SELECT 1").execute(db.pool()).await.unwrap();
    }

    #[test]
    fn parses_duration_settings_strictly() {
        assert_eq!(
            parse_duration_setting("250ms").unwrap(),
            Duration::from_millis(250)
        );
        assert_eq!(
            parse_duration_setting("15m").unwrap(),
            Duration::from_secs(900)
        );
        assert!(parse_duration_setting("bogus").is_err());
    }
}
