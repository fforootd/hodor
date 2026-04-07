use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct StorageConfig {
    pub primary: PrimaryStorageConfig,
    pub transient: TransientStorageConfig,
    pub analytics: AnalyticsStorageConfig,
    pub retention: RetentionConfig,
}

pub trait DatabaseConnectConfig {
    fn backend(&self) -> &str;
    fn url(&self) -> &str;
    fn database(&self) -> &str;
    fn emulator_host(&self) -> &str;
    fn credentials_file(&self) -> &str;
    fn credentials_json(&self) -> &str;
    fn resolve_backend(&self) -> &str;
    fn max_open_conns(&self) -> u32 {
        25
    }
    fn max_idle_conns(&self) -> u32 {
        5
    }
    fn conn_max_lifetime(&self) -> &str {
        "1h"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ReplicaReadConfig {
    pub enabled: bool,
    pub url: String,
    pub mode: String,
}

impl Default for ReplicaReadConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            mode: "explicit".into(),
        }
    }
}

impl ReplicaReadConfig {
    pub fn is_enabled(&self) -> bool {
        self.enabled && !self.url.is_empty()
    }

    pub fn resolve_mode(&self) -> &str {
        match self.mode.as_str() {
            "explicit" => "explicit",
            _ => "explicit",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PrimaryStorageConfig {
    pub backend: String,
    pub url: String,
    pub database: String,
    pub emulator_host: String,
    pub credentials_file: String,
    pub credentials_json: String,
    pub migrate: String,
    pub bootstrap: String,
    pub max_open_conns: u32,
    pub max_idle_conns: u32,
    pub conn_max_lifetime: String,
    pub replica: ReplicaReadConfig,
}

impl Default for PrimaryStorageConfig {
    fn default() -> Self {
        Self {
            backend: String::new(),
            url: "sqlite://./data/zitadel.db".into(),
            database: String::new(),
            emulator_host: String::new(),
            credentials_file: String::new(),
            credentials_json: String::new(),
            migrate: String::new(),
            bootstrap: String::new(),
            max_open_conns: 25,
            max_idle_conns: 5,
            conn_max_lifetime: "1h".into(),
            replica: ReplicaReadConfig::default(),
        }
    }
}

impl PrimaryStorageConfig {
    pub fn resolve_backend(&self) -> &str {
        match self.backend.as_str() {
            "sqlite" | "postgres" | "spanner" => self.backend.as_str(),
            _ if self.url.starts_with("postgres://") || self.url.starts_with("postgresql://") => {
                "postgres"
            }
            _ if !self.database.is_empty() => "spanner",
            _ => "sqlite",
        }
    }

    pub fn resolve_migrate_mode(&self) -> &str {
        match self.migrate.as_str() {
            "auto" | "check" | "skip" => self.migrate.as_str(),
            _ => "auto",
        }
    }

    pub fn resolve_bootstrap_mode(&self) -> &str {
        match self.bootstrap.as_str() {
            "auto" | "skip" => self.bootstrap.as_str(),
            _ => "auto",
        }
    }
}

impl DatabaseConnectConfig for PrimaryStorageConfig {
    fn backend(&self) -> &str {
        &self.backend
    }

    fn url(&self) -> &str {
        &self.url
    }

    fn database(&self) -> &str {
        &self.database
    }

    fn emulator_host(&self) -> &str {
        &self.emulator_host
    }

    fn credentials_file(&self) -> &str {
        &self.credentials_file
    }

    fn credentials_json(&self) -> &str {
        &self.credentials_json
    }

    fn resolve_backend(&self) -> &str {
        Self::resolve_backend(self)
    }

    fn max_open_conns(&self) -> u32 {
        self.max_open_conns
    }

    fn max_idle_conns(&self) -> u32 {
        self.max_idle_conns
    }

    fn conn_max_lifetime(&self) -> &str {
        &self.conn_max_lifetime
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TransientStorageConfig {
    pub backend: String,
    pub url: String,
    pub database: String,
    pub emulator_host: String,
    pub credentials_file: String,
    pub credentials_json: String,
}

impl Default for TransientStorageConfig {
    fn default() -> Self {
        Self {
            backend: "inherit".into(),
            url: String::new(),
            database: String::new(),
            emulator_host: String::new(),
            credentials_file: String::new(),
            credentials_json: String::new(),
        }
    }
}

impl TransientStorageConfig {
    pub fn inherits_primary(&self) -> bool {
        matches!(
            self.backend.as_str(),
            "" | "inherit" | "same_primary" | "same_db"
        ) && self.url.is_empty()
            && self.database.is_empty()
            && self.emulator_host.is_empty()
            && self.credentials_file.is_empty()
            && self.credentials_json.is_empty()
    }

    pub fn resolve_backend(&self) -> &str {
        match self.backend.as_str() {
            "inherit" | "same_primary" | "same_db" => "inherit",
            "sqlite" | "postgres" | "spanner" => self.backend.as_str(),
            _ if self.url.starts_with("postgres://") || self.url.starts_with("postgresql://") => {
                "postgres"
            }
            _ if !self.database.is_empty() => "spanner",
            _ => "sqlite",
        }
    }
}

impl DatabaseConnectConfig for TransientStorageConfig {
    fn backend(&self) -> &str {
        &self.backend
    }

    fn url(&self) -> &str {
        &self.url
    }

    fn database(&self) -> &str {
        &self.database
    }

    fn emulator_host(&self) -> &str {
        &self.emulator_host
    }

    fn credentials_file(&self) -> &str {
        &self.credentials_file
    }

    fn credentials_json(&self) -> &str {
        &self.credentials_json
    }

    fn resolve_backend(&self) -> &str {
        Self::resolve_backend(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AnalyticsStorageConfig {
    pub backend: String,
    pub url: String,
    pub database: String,
    pub emulator_host: String,
    pub credentials_file: String,
    pub credentials_json: String,
}

impl Default for AnalyticsStorageConfig {
    fn default() -> Self {
        Self {
            backend: "inherit".into(),
            url: String::new(),
            database: String::new(),
            emulator_host: String::new(),
            credentials_file: String::new(),
            credentials_json: String::new(),
        }
    }
}

impl AnalyticsStorageConfig {
    pub fn inherits_primary(&self) -> bool {
        matches!(
            self.backend.as_str(),
            "" | "inherit" | "same_primary" | "same_db" | "same_stateful"
        ) && self.url.is_empty()
            && self.database.is_empty()
            && self.emulator_host.is_empty()
            && self.credentials_file.is_empty()
            && self.credentials_json.is_empty()
    }

    pub fn resolve_backend(&self) -> &str {
        match self.backend.as_str() {
            "inherit" | "same_primary" | "same_db" | "same_stateful" => "inherit",
            "sqlite" | "postgres" | "spanner" => self.backend.as_str(),
            _ if self.url.starts_with("postgres://") || self.url.starts_with("postgresql://") => {
                "postgres"
            }
            _ if !self.database.is_empty() => "spanner",
            _ => "sqlite",
        }
    }
}

impl DatabaseConnectConfig for AnalyticsStorageConfig {
    fn backend(&self) -> &str {
        &self.backend
    }

    fn url(&self) -> &str {
        &self.url
    }

    fn database(&self) -> &str {
        &self.database
    }

    fn emulator_host(&self) -> &str {
        &self.emulator_host
    }

    fn credentials_file(&self) -> &str {
        &self.credentials_file
    }

    fn credentials_json(&self) -> &str {
        &self.credentials_json
    }

    fn resolve_backend(&self) -> &str {
        Self::resolve_backend(self)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct RetentionConfig {
    pub events: EventRetentionConfig,
    pub sessions: TerminalDataRetentionConfig,
    pub tokens: TerminalDataRetentionConfig,
    pub transient_auth_state: ExpiredDataRetentionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct EventRetentionConfig {
    pub keep_for: String,
}

impl Default for EventRetentionConfig {
    fn default() -> Self {
        Self {
            keep_for: "14d".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TerminalDataRetentionConfig {
    pub retain_terminal_for: String,
}

impl Default for TerminalDataRetentionConfig {
    fn default() -> Self {
        Self {
            retain_terminal_for: "7d".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ExpiredDataRetentionConfig {
    pub retain_after_expiry: String,
}

impl Default for ExpiredDataRetentionConfig {
    fn default() -> Self {
        Self {
            retain_after_expiry: "1h".into(),
        }
    }
}

pub type StatefulStorageConfig = PrimaryStorageConfig;

#[cfg(test)]
mod tests {
    use super::{PrimaryStorageConfig, ReplicaReadConfig, TransientStorageConfig};

    #[test]
    fn resolves_spanner_backend_from_explicit_backend() {
        let config = PrimaryStorageConfig {
            backend: "spanner".into(),
            ..Default::default()
        };

        assert_eq!(config.resolve_backend(), "spanner");
    }

    #[test]
    fn resolves_spanner_backend_from_database_name() {
        let config = PrimaryStorageConfig {
            database: "projects/test/instances/dev/databases/zitadel".into(),
            ..Default::default()
        };

        assert_eq!(config.resolve_backend(), "spanner");
    }

    #[test]
    fn preserves_postgres_detection_from_url() {
        let config = PrimaryStorageConfig {
            url: "postgres://localhost/zitadel".into(),
            ..Default::default()
        };

        assert_eq!(config.resolve_backend(), "postgres");
    }

    #[test]
    fn transient_defaults_to_inherit() {
        let config = TransientStorageConfig::default();
        assert!(config.inherits_primary());
    }

    #[test]
    fn replica_needs_enable_and_url() {
        let disabled = ReplicaReadConfig::default();
        assert!(!disabled.is_enabled());

        let enabled = ReplicaReadConfig {
            enabled: true,
            url: "postgres://readonly/zitadel".into(),
            ..Default::default()
        };
        assert!(enabled.is_enabled());
    }
}
