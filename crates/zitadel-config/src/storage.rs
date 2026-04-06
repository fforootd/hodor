use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct StorageConfig {
    pub stateful: StatefulStorageConfig,
    pub read: ReadStoreConfig,
    pub kv: KvStoreConfig,
    pub sink: SinkConfig,
    pub process_cache: ProcessCacheConfig,
    pub analytics: AnalyticsStorageConfig,
    pub retention: RetentionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct StatefulStorageConfig {
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
}

impl Default for StatefulStorageConfig {
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
        }
    }
}

impl StatefulStorageConfig {
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ReadStoreConfig {
    pub backend: String,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct KvStoreConfig {
    pub backend: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SinkConfig {
    pub backend: String,
    pub url: String,
    pub buffer_size: u32,
    pub batch_size: u32,
    pub flush_interval: String,
}

impl Default for SinkConfig {
    fn default() -> Self {
        Self {
            backend: String::new(),
            url: String::new(),
            buffer_size: 4096,
            batch_size: 128,
            flush_interval: "100ms".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ProcessCacheConfig {
    pub backend: String,
}

impl Default for ProcessCacheConfig {
    fn default() -> Self {
        Self {
            backend: "memory".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AnalyticsStorageConfig {
    pub backend: String,
    pub url: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct RetentionConfig {
    pub events: EventRetentionConfig,
    pub sessions: TerminalDataRetentionConfig,
    pub tokens: TerminalDataRetentionConfig,
    pub transient_auth_state: ExpiredDataRetentionConfig,
    pub sink_inbox: InboxRetentionConfig,
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

#[cfg(test)]
mod tests {
    use super::StatefulStorageConfig;

    #[test]
    fn resolves_spanner_backend_from_explicit_backend() {
        let config = StatefulStorageConfig {
            backend: "spanner".into(),
            ..Default::default()
        };

        assert_eq!(config.resolve_backend(), "spanner");
    }

    #[test]
    fn resolves_spanner_backend_from_database_name() {
        let config = StatefulStorageConfig {
            database: "projects/test/instances/dev/databases/zitadel".into(),
            ..Default::default()
        };

        assert_eq!(config.resolve_backend(), "spanner");
    }

    #[test]
    fn preserves_postgres_detection_from_url() {
        let config = StatefulStorageConfig {
            url: "postgres://localhost/zitadel".into(),
            ..Default::default()
        };

        assert_eq!(config.resolve_backend(), "postgres");
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct InboxRetentionConfig {
    pub retain_failed_for: String,
}

impl Default for InboxRetentionConfig {
    fn default() -> Self {
        Self {
            retain_failed_for: "24h".into(),
        }
    }
}
