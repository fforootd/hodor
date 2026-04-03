use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::oidc;
use crate::password;
use crate::session;

// Re-export for convenience from the top-level config.
pub use oidc::OidcConfig;
pub use password::{PasswordHasherConfig, SecretHasherConfig};
pub use session::SessionConfig;

/// Per-purpose secret generator profile override in TOML.
///
/// Only fields explicitly set override the compiled-in defaults.
/// This mirrors the profile shape from `zitadel-crypto::generator::GeneratorProfile`
/// but lives in the config crate to avoid a circular dependency.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct GeneratorProfileOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub charset: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_chars: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiry_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dash_interval: Option<usize>,
}

/// Generator configuration section in TOML.
///
/// Keys are purpose names (e.g., `client_secret`, `otp_sms`).
/// Values are partial overrides of the compiled-in defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct GeneratorsConfig {
    #[serde(flatten)]
    pub profiles: HashMap<String, GeneratorProfileOverride>,
}
