use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Password hashing algorithm selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HashAlgorithm {
    Argon2id,
    Bcrypt,
    // Pbkdf2 will be added under #[cfg(feature = "fips")] in a future PR.
}

/// Configuration for the primary password hasher.
///
/// These are system-level (TOML) settings, not per-org policy.
/// Password *policy* (min length, complexity) lives in the settings table.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PasswordHasherConfig {
    pub algorithm: HashAlgorithm,
    /// Argon2id: memory cost in KiB (default: 65536 = 64 MB).
    pub memory_cost_kb: u32,
    /// Argon2id: number of iterations (default: 3).
    pub time_cost: u32,
    /// Argon2id: degree of parallelism (default: 4).
    pub parallelism: u32,
    /// Bcrypt: cost factor (default: 14).
    pub bcrypt_cost: u32,
}

impl Default for PasswordHasherConfig {
    fn default() -> Self {
        Self {
            algorithm: HashAlgorithm::Argon2id,
            memory_cost_kb: 64 * 1024,
            time_cost: 3,
            parallelism: 4,
            bcrypt_cost: 14,
        }
    }
}

impl PasswordHasherConfig {
    /// Fast defaults for development / testing.
    pub fn dev_defaults() -> Self {
        Self {
            algorithm: HashAlgorithm::Argon2id,
            memory_cost_kb: 4 * 1024,
            time_cost: 1,
            parallelism: 1,
            bcrypt_cost: 4,
        }
    }
}

/// Configuration for API / machine secret hashing (lower cost than user passwords).
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SecretHasherConfig {
    pub algorithm: HashAlgorithm,
    pub bcrypt_cost: u32,
}

impl Default for SecretHasherConfig {
    fn default() -> Self {
        Self {
            algorithm: HashAlgorithm::Bcrypt,
            bcrypt_cost: 4,
        }
    }
}
