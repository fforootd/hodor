use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use password_hash::{PasswordHash, SaltString, rand_core::OsRng};
use zitadel_config::password::{HashAlgorithm, PasswordHasherConfig};

// ─── Public types ───

/// Result of password verification.
#[derive(Debug)]
pub enum VerifyResult {
    /// Password matches and hash is current.
    Ok,
    /// Password matches but hash uses an outdated algorithm or params.
    /// Contains the re-hashed value that should be persisted.
    NeedUpdate(String),
}

/// Passwap-style password swapper: verifies against any supported algorithm
/// and transparently re-hashes to the preferred algorithm on success.
pub struct Swapper {
    preferred: Box<dyn Hasher>,
    legacy: Vec<Box<dyn Hasher>>,
}

impl Swapper {
    /// Build from runtime config. Selects the preferred hasher based on algorithm
    /// and always adds legacy verifiers for bcrypt and plain (dev/seed).
    pub fn from_config(cfg: &PasswordHasherConfig) -> Self {
        let preferred: Box<dyn Hasher> = match cfg.algorithm {
            HashAlgorithm::Argon2id => Box::new(Argon2idHasher::from_config(cfg)),
            HashAlgorithm::Bcrypt => Box::new(BcryptHasher),
        };
        Self {
            preferred,
            legacy: vec![
                Box::new(Argon2idHasher::from_config(&PasswordHasherConfig::default())),
                Box::new(BcryptHasher),
                Box::new(PlainDevHasher),
            ],
        }
    }

    /// Production swapper: argon2id (64 MB, 3 iter, 4 threads) + bcrypt + plain legacy.
    pub fn production() -> Self {
        Self::from_config(&PasswordHasherConfig::default())
    }

    /// Dev swapper: fast argon2id (4 MB, 1 iter) + plain legacy.
    pub fn dev() -> Self {
        Self::from_config(&PasswordHasherConfig::dev_defaults())
    }

    /// Hash a password with the preferred algorithm.
    pub fn hash(&self, password: &str) -> anyhow::Result<String> {
        self.preferred.hash(password)
    }

    /// Verify a password against an encoded hash.
    ///
    /// Returns `Ok(VerifyResult::Ok)` if the password matches and the hash is current.
    /// Returns `Ok(VerifyResult::NeedUpdate(new_hash))` if the password matches but
    /// the hash should be updated (different algorithm or outdated params).
    /// Returns `Err` if the password doesn't match or the hash format is unrecognized.
    pub fn verify(&self, encoded: &str, password: &str) -> anyhow::Result<VerifyResult> {
        // Try preferred hasher first.
        if self.preferred.supports(encoded) {
            if !self.preferred.verify(encoded, password) {
                anyhow::bail!("invalid password");
            }
            if self.preferred.needs_update(encoded) {
                let new_hash = self.preferred.hash(password)?;
                return Ok(VerifyResult::NeedUpdate(new_hash));
            }
            return Ok(VerifyResult::Ok);
        }

        // Try legacy verifiers.
        for legacy in &self.legacy {
            if !legacy.supports(encoded) {
                continue;
            }
            if !legacy.verify(encoded, password) {
                anyhow::bail!("invalid password");
            }
            // Legacy match → re-hash with preferred.
            let new_hash = self.preferred.hash(password)?;
            return Ok(VerifyResult::NeedUpdate(new_hash));
        }

        anyhow::bail!("unrecognized password hash format")
    }
}

// ─── Hasher trait ───

trait Hasher: Send + Sync {
    fn hash(&self, password: &str) -> anyhow::Result<String>;
    fn verify(&self, encoded: &str, password: &str) -> bool;
    fn needs_update(&self, encoded: &str) -> bool;
    fn supports(&self, encoded: &str) -> bool;
}

// ─── Argon2id ───

struct Argon2idHasher {
    params: Argon2<'static>,
    expected_m_cost: u32,
    expected_t_cost: u32,
    expected_p_cost: u32,
}

impl Argon2idHasher {
    fn from_config(cfg: &PasswordHasherConfig) -> Self {
        let (m, t, p) = (cfg.memory_cost_kb, cfg.time_cost, cfg.parallelism);
        Self {
            params: Argon2::new(
                argon2::Algorithm::Argon2id,
                argon2::Version::V0x13,
                argon2::Params::new(m, t, p, Some(32)).unwrap(),
            ),
            expected_m_cost: m,
            expected_t_cost: t,
            expected_p_cost: p,
        }
    }
}

impl Hasher for Argon2idHasher {
    fn hash(&self, password: &str) -> anyhow::Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .params
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("argon2id hash: {e}"))?;
        Ok(hash.to_string())
    }

    fn verify(&self, encoded: &str, password: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(encoded) else {
            return false;
        };
        self.params
            .verify_password(password.as_bytes(), &parsed)
            .is_ok()
    }

    fn needs_update(&self, encoded: &str) -> bool {
        let Ok(parsed) = PasswordHash::new(encoded) else {
            return true;
        };
        // If it's not argon2id, it needs update.
        if parsed.algorithm != argon2::ARGON2ID_IDENT {
            return true;
        }
        // Check params from the PHC string.
        let params = parsed.params;
        let m = params
            .get_str("m")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        let t = params
            .get_str("t")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        let p = params
            .get_str("p")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);
        m != self.expected_m_cost || t != self.expected_t_cost || p != self.expected_p_cost
    }

    fn supports(&self, encoded: &str) -> bool {
        encoded.starts_with("$argon2id$") || encoded.starts_with("$argon2i$")
    }
}

// ─── Bcrypt ───

struct BcryptHasher;

impl Hasher for BcryptHasher {
    fn hash(&self, password: &str) -> anyhow::Result<String> {
        bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| anyhow::anyhow!("bcrypt hash: {e}"))
    }

    fn verify(&self, encoded: &str, password: &str) -> bool {
        bcrypt::verify(password, encoded).unwrap_or(false)
    }

    fn needs_update(&self, _encoded: &str) -> bool {
        // Bcrypt is always "outdated" when argon2id is preferred — caller
        // decides via the Swapper whether to re-hash.
        false
    }

    fn supports(&self, encoded: &str) -> bool {
        encoded.starts_with("$2a$") || encoded.starts_with("$2b$") || encoded.starts_with("$2y$")
    }
}

// ─── Plain (dev/seed only) ───

struct PlainDevHasher;

impl Hasher for PlainDevHasher {
    fn hash(&self, password: &str) -> anyhow::Result<String> {
        Ok(format!("$plain${password}"))
    }

    fn verify(&self, encoded: &str, password: &str) -> bool {
        encoded
            .strip_prefix("$plain$")
            .is_some_and(|stored| stored == password)
    }

    fn needs_update(&self, _encoded: &str) -> bool {
        false
    }

    fn supports(&self, encoded: &str) -> bool {
        encoded.starts_with("$plain$")
    }
}

// ─── Helpers ───

/// Encode a hash into credential JSON format: {"hash":"<hash>"}.
pub fn encode_credential_json(hash: &str) -> String {
    format!(r#"{{"hash":"{hash}"}}"#)
}

/// Decode hash from credential JSON: {"hash":"<hash>"}.
pub fn decode_credential_json(json: &str) -> Option<String> {
    let json = json.trim();
    let inner = json.strip_prefix(r#"{"hash":""#)?;
    let hash = inner.strip_suffix(r#""}"#)?;
    Some(hash.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dev_swapper() -> Swapper {
        Swapper::dev()
    }

    #[test]
    fn hash_and_verify_argon2id() {
        let sw = dev_swapper();
        let hash = sw.hash("test123").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        match sw.verify(&hash, "test123").unwrap() {
            VerifyResult::Ok => {}
            VerifyResult::NeedUpdate(_) => panic!("should not need update"),
        }
    }

    #[test]
    fn wrong_password_fails() {
        let sw = dev_swapper();
        let hash = sw.hash("test123").unwrap();
        assert!(sw.verify(&hash, "wrong").is_err());
    }

    #[test]
    fn plain_seed_triggers_need_update() {
        let sw = dev_swapper();
        match sw.verify("$plain$admin123", "admin123").unwrap() {
            VerifyResult::NeedUpdate(new_hash) => {
                assert!(new_hash.starts_with("$argon2id$"));
                // Verify the new hash works.
                match sw.verify(&new_hash, "admin123").unwrap() {
                    VerifyResult::Ok => {}
                    _ => panic!("re-hashed value should verify as Ok"),
                }
            }
            VerifyResult::Ok => panic!("plain should trigger NeedUpdate"),
        }
    }

    #[test]
    fn bcrypt_triggers_need_update() {
        let bcrypt_hash = bcrypt::hash("mypassword", 4).unwrap();
        let sw = dev_swapper();
        match sw.verify(&bcrypt_hash, "mypassword").unwrap() {
            VerifyResult::NeedUpdate(new_hash) => {
                assert!(new_hash.starts_with("$argon2id$"));
            }
            VerifyResult::Ok => panic!("bcrypt should trigger NeedUpdate"),
        }
    }

    #[test]
    fn bcrypt_wrong_password_fails() {
        let bcrypt_hash = bcrypt::hash("mypassword", 4).unwrap();
        let sw = dev_swapper();
        assert!(sw.verify(&bcrypt_hash, "wrong").is_err());
    }

    #[test]
    fn unknown_format_fails() {
        let sw = dev_swapper();
        assert!(sw.verify("$unknown$something", "password").is_err());
    }

    #[test]
    fn credential_json_roundtrip() {
        let hash = "$argon2id$v=19$m=4096,t=1,p=1$abc$def";
        let json = encode_credential_json(hash);
        assert_eq!(json, r#"{"hash":"$argon2id$v=19$m=4096,t=1,p=1$abc$def"}"#);
        let decoded = decode_credential_json(&json).unwrap();
        assert_eq!(decoded, hash);
    }
}
