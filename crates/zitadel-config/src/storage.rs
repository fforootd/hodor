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
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct StatefulStorageConfig {
    pub url: String,
    pub migrate: String,
    pub bootstrap: String,
    pub max_open_conns: u32,
    pub max_idle_conns: u32,
    pub conn_max_lifetime: String,
}

impl Default for StatefulStorageConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://./data/zitadel.db".into(),
            migrate: String::new(),
            bootstrap: String::new(),
            max_open_conns: 25,
            max_idle_conns: 5,
            conn_max_lifetime: "1h".into(),
        }
    }
}

impl StatefulStorageConfig {
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
