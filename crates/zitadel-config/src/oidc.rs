use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// OIDC signing algorithm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum SigningAlgorithm {
    RS256,
    ES256,
    ES384,
}

/// OIDC provider configuration: signing keys, key lifecycle, and token lifetimes.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct OidcConfig {
    /// JWT signing algorithm (default: RS256).
    pub signing_algorithm: SigningAlgorithm,
    /// RSA key size in bits (ignored for EC). Default: 2048.
    pub key_size: u32,
    /// How long a private signing key is used before rotation (seconds). Default: 6h.
    pub private_key_lifetime_secs: u64,
    /// How long a public key remains available for verification after rotation (seconds). Default: 30h.
    pub public_key_lifetime_secs: u64,
    /// Access token lifetime (seconds). Default: 43200 (12h).
    pub access_token_lifetime_secs: u64,
    /// ID token lifetime (seconds). Default: 43200 (12h).
    pub id_token_lifetime_secs: u64,
    /// Refresh token idle expiration (seconds). Default: 2592000 (720h).
    pub refresh_token_idle_secs: u64,
    /// Refresh token absolute max lifetime (seconds). Default: 7776000 (2160h).
    pub refresh_token_max_secs: u64,
}

impl Default for OidcConfig {
    fn default() -> Self {
        Self {
            signing_algorithm: SigningAlgorithm::RS256,
            key_size: 2048,
            private_key_lifetime_secs: 6 * 3600,
            public_key_lifetime_secs: 30 * 3600,
            access_token_lifetime_secs: 12 * 3600,
            id_token_lifetime_secs: 12 * 3600,
            refresh_token_idle_secs: 720 * 3600,
            refresh_token_max_secs: 2160 * 3600,
        }
    }
}
