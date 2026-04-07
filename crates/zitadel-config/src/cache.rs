use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CacheConfig {
    pub shared: SharedCacheConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SharedCacheConfig {
    pub backend: String,
    pub url: String,
}

impl Default for SharedCacheConfig {
    fn default() -> Self {
        Self {
            backend: "disabled".into(),
            url: String::new(),
        }
    }
}

impl SharedCacheConfig {
    pub fn resolve_backend(&self) -> &str {
        match self.backend.as_str() {
            "" | "disabled" => "disabled",
            "db" | "redis" => self.backend.as_str(),
            _ => "disabled",
        }
    }
}
