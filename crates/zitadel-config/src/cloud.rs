use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CloudConfig {
    pub enabled: bool,
    pub resolver_cache_capacity: u32,
    pub positive_cache_ttl_secs: u64,
    pub negative_cache_ttl_secs: u64,
    pub control_plane: CloudControlPlaneConfig,
}

impl CloudConfig {
    pub fn resolve_cache_capacity(&self) -> usize {
        self.resolver_cache_capacity.max(1) as usize
    }

    pub fn resolve_positive_cache_ttl_secs(&self) -> u64 {
        self.positive_cache_ttl_secs.max(1)
    }

    pub fn resolve_negative_cache_ttl_secs(&self) -> u64 {
        self.negative_cache_ttl_secs.max(1)
    }

    pub fn resolve_control_plane_url<'a>(&'a self, fallback: &'a str) -> &'a str {
        if self.control_plane.url.is_empty() {
            fallback
        } else {
            self.control_plane.url.as_str()
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CloudControlPlaneConfig {
    pub url: String,
    pub secret_ref: String,
}

impl Default for CloudConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            resolver_cache_capacity: 50_000,
            positive_cache_ttl_secs: 60,
            negative_cache_ttl_secs: 10,
            control_plane: CloudControlPlaneConfig::default(),
        }
    }
}
