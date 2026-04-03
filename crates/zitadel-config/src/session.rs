use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Session configuration.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SessionConfig {
    /// Maximum session age in seconds. Default: 86400 (24h).
    pub max_age_secs: u64,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            max_age_secs: 86400,
        }
    }
}
