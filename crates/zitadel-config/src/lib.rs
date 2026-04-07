mod cache;
mod cloud;
mod crypto;
mod env;
mod generators;
mod observability;
pub mod oidc;
pub mod password;
mod server;
pub mod session;
mod storage;

pub use cache::*;
pub use cloud::*;
pub use crypto::*;
pub use generators::*;
pub use observability::*;
pub use server::*;
pub use storage::*;

use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::env::flat_env_overrides;

/// Runtime configuration for the Zitadel server.
/// Covers infrastructure-level settings only.
/// Domain-specific configuration (policies, whitelabeling) lives in seed YAML.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub tls: TlsConfig,
    pub encryption: EncryptionConfig,
    pub cache: CacheConfig,
    pub storage: StorageConfig,
    pub observability: ObservabilityConfig,
    pub workers: WorkersConfig,
    pub rate_limit: RateLimitConfig,
    pub catalog: CatalogConfig,
    pub dev: DevConfig,
    pub password_hasher: PasswordHasherConfig,
    pub secret_hasher: SecretHasherConfig,
    pub oidc: OidcConfig,
    pub session: SessionConfig,
    pub generators: GeneratorsConfig,
    pub cloud: CloudConfig,
}

impl Config {
    /// Load config from an optional TOML file path, with env var overlays.
    /// If path is None, only defaults + env vars are used.
    ///
    /// Environment variables use ZITADEL_ prefix with __ for nesting:
    ///   ZITADEL_SERVER__PORT=9090
    ///   ZITADEL_STORAGE__STATEFUL__URL=postgres://...
    ///
    /// Flat env vars from the Go version are also supported:
    ///   ZITADEL_PORT, ZITADEL_STORAGE_STATEFUL_URL, etc.
    #[allow(clippy::result_large_err)]
    pub fn load(path: Option<&Path>) -> Result<Self, figment::Error> {
        validate_no_legacy_storage_config(path).map_err(figment::Error::from)?;
        let mut figment = Figment::from(Serialized::defaults(Config::default()));

        if let Some(p) = path {
            figment = figment.merge(Toml::file(p));
        }

        // Nested env vars: ZITADEL_SERVER__PORT etc.
        figment = figment.merge(Env::prefixed("ZITADEL_").split("__"));

        // Flat env var overrides matching Go behavior.
        figment = figment.merge(flat_env_overrides());

        figment.extract()
    }

    /// Generate the JSON Schema for the config.
    pub fn json_schema() -> schemars::schema::RootSchema {
        schemars::schema_for!(Config)
    }

    /// Generate the JSON Schema as a pretty-printed string.
    pub fn json_schema_string() -> String {
        serde_json::to_string_pretty(&Self::json_schema()).expect("schema serialization")
    }

    /// Whether this looks like a dev environment.
    pub fn is_dev(&self) -> bool {
        !self.dev.seed_file.is_empty() || self.server.external_domain == "localhost"
    }
}

fn validate_no_legacy_storage_config(path: Option<&Path>) -> Result<(), String> {
    if let Some(path) = path {
        let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let parsed: toml::Value = raw.parse().map_err(|e: toml::de::Error| e.to_string())?;
        if parsed.get("database").is_some() {
            return Err(
                "legacy [database] config is no longer supported; move it to [storage.primary]"
                    .into(),
            );
        }
        if let Some(storage) = parsed.get("storage").and_then(toml::Value::as_table) {
            for legacy_key in ["stateful", "read", "kv", "sink", "process_cache"] {
                if storage.contains_key(legacy_key) {
                    return Err(format!(
                        "legacy [storage.{legacy_key}] config is no longer supported; use [storage.primary], [storage.transient], [storage.analytics], or [cache.shared]"
                    ));
                }
            }
        }
    }

    for key in [
        "ZITADEL_DATABASE__URL",
        "ZITADEL_DATABASE__MIGRATE",
        "ZITADEL_DATABASE__BOOTSTRAP",
        "ZITADEL_DATABASE_URL",
        "ZITADEL_DATABASE_MIGRATE",
        "ZITADEL_DATABASE_BOOTSTRAP",
    ] {
        if std::env::var_os(key).is_some() {
            return Err(format!(
                "legacy {key} is no longer supported; use ZITADEL_STORAGE__PRIMARY__URL, ZITADEL_STORAGE__PRIMARY__MIGRATE, or ZITADEL_STORAGE__PRIMARY__BOOTSTRAP"
            ));
        }
    }

    for key in [
        "ZITADEL_STORAGE__STATEFUL__URL",
        "ZITADEL_STORAGE__STATEFUL__DATABASE",
        "ZITADEL_STORAGE__STATEFUL__EMULATOR_HOST",
        "ZITADEL_STORAGE__STATEFUL__CREDENTIALS_FILE",
        "ZITADEL_STORAGE__STATEFUL__CREDENTIALS_JSON",
        "ZITADEL_STORAGE__STATEFUL__BACKEND",
        "ZITADEL_STORAGE__STATEFUL__MIGRATE",
        "ZITADEL_STORAGE__STATEFUL__BOOTSTRAP",
        "ZITADEL_STORAGE_STATEFUL_URL",
        "ZITADEL_STORAGE_STATEFUL_DATABASE",
        "ZITADEL_STORAGE_STATEFUL_EMULATOR_HOST",
        "ZITADEL_STORAGE_STATEFUL_CREDENTIALS_FILE",
        "ZITADEL_STORAGE_STATEFUL_CREDENTIALS_JSON",
        "ZITADEL_STORAGE_STATEFUL_BACKEND",
        "ZITADEL_STORAGE_STATEFUL_MIGRATE",
        "ZITADEL_STORAGE_STATEFUL_BOOTSTRAP",
        "ZITADEL_STORAGE__READ__BACKEND",
        "ZITADEL_STORAGE__READ__URL",
        "ZITADEL_STORAGE__KV__BACKEND",
        "ZITADEL_STORAGE__KV__URL",
        "ZITADEL_STORAGE__SINK__BACKEND",
        "ZITADEL_STORAGE__SINK__URL",
        "ZITADEL_STORAGE__PROCESS_CACHE__BACKEND",
    ] {
        if std::env::var_os(key).is_some() {
            return Err(format!(
                "legacy {key} is no longer supported; use ZITADEL_STORAGE__PRIMARY__*, ZITADEL_STORAGE__TRANSIENT__*, ZITADEL_STORAGE__ANALYTICS__*, or ZITADEL_CACHE__SHARED__*"
            ));
        }
    }

    Ok(())
}

pub fn reference_toml() -> &'static str {
    include_str!("../../../zitadel.reference.toml")
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct WorkersConfig {
    pub notification_workers: u32,
    pub event_workers: u32,
    pub lake_batch_size: u32,
    pub lake_batch_window_secs: u32,
    pub scheduler_enabled: bool,
    pub scheduler_poll_interval: String,
    pub scheduler_lease_ttl: String,
    pub cleanup_batch_size: u32,
    pub cleanup_max_rows_per_run: u32,
    pub cleanup_max_run_duration: String,
    pub event_partition_premake_days: u32,
    pub event_consumer_poll_interval: String,
}

impl Default for WorkersConfig {
    fn default() -> Self {
        Self {
            notification_workers: 1,
            event_workers: 1,
            lake_batch_size: 1000,
            lake_batch_window_secs: 5,
            scheduler_enabled: true,
            scheduler_poll_interval: "30s".into(),
            scheduler_lease_ttl: "90s".into(),
            cleanup_batch_size: 500,
            cleanup_max_rows_per_run: 2000,
            cleanup_max_run_duration: "2s".into(),
            event_partition_premake_days: 3,
            event_consumer_poll_interval: "5s".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct RateLimitConfig {
    pub backend: String,
    pub redis_url: String,
    pub gc_interval: u32,
    pub batch_write: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            backend: "memory".into(),
            redis_url: String::new(),
            gc_interval: 60,
            batch_write: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CatalogConfig {
    pub url: String,
    pub local_path: String,
    pub refresh_interval: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DevConfig {
    pub seed_file: String,
    pub conformance_login_html: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_values() {
        let cfg = Config::default();
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(cfg.server.external_domain, "localhost");
        assert_eq!(cfg.storage.primary.url, "sqlite://./data/zitadel.db");
        assert_eq!(cfg.observability.log_level, "info");
        assert_eq!(cfg.observability.streams.request.mode, "sampled");
        assert_eq!(cfg.observability.streams.request.sample_rate, 0.01);
        assert_eq!(cfg.storage.primary.resolve_migrate_mode(), "auto");
        assert_eq!(cfg.storage.primary.resolve_bootstrap_mode(), "auto");
    }

    #[test]
    fn tls_resolve_mode_dev() {
        let tls = TlsConfig::default();
        let server = ServerConfig::default();
        assert_eq!(tls.resolve_mode(&server, true), "off");
    }

    #[test]
    fn json_schema_generates() {
        let schema = Config::json_schema_string();
        assert!(schema.contains("\"Config\""));
        assert!(schema.contains("\"ServerConfig\""));
        assert!(schema.contains("\"StorageConfig\""));
        assert!(schema.contains("\"PasswordHasherConfig\""));
        assert!(schema.contains("\"OidcConfig\""));
        assert!(schema.contains("\"SessionConfig\""));
        assert!(schema.contains("\"EncryptionConfig\""));
        // Verify it's valid JSON.
        let _: serde_json::Value = serde_json::from_str(&schema).unwrap();

        // Write schema to file if SCHEMA_OUT env var is set (used by `just config-schema`).
        if let Ok(path) = std::env::var("SCHEMA_OUT") {
            std::fs::write(&path, &schema).unwrap();
        }
    }

    #[test]
    fn tls_resolve_mode_explicit() {
        let tls = TlsConfig {
            mode: "auto".into(),
            ..Default::default()
        };
        let server = ServerConfig::default();
        assert_eq!(tls.resolve_mode(&server, false), "auto");
    }

    #[test]
    fn reference_toml_loads() {
        let cfg: Config = toml::from_str(reference_toml()).expect("reference TOML should parse");
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(cfg.storage.primary.url, "sqlite://./data/zitadel.db");
        assert_eq!(cfg.observability.log_level, "info");
        assert_eq!(cfg.observability.cache_path, "./data/zitadel-cache.db");
        assert_eq!(cfg.observability.streams.request.mode, "sampled");
        assert_eq!(cfg.observability.sinks.analytics.drain_batch, 500);
        assert_eq!(cfg.session.max_age_secs, 86400);
        assert_eq!(cfg.oidc.access_token_lifetime_secs, 12 * 3600);
    }
}
