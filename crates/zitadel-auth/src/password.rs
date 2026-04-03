use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use password_hash::{PasswordHash, SaltString, rand_core::OsRng};

/// Password hashing with argon2id. Supports production and fast-dev params.
pub struct Passwords {
    params: Argon2<'static>,
}

impl Passwords {
    /// Production params: 3 iterations, 64 MB, 4 threads.
    pub fn new() -> Self {
        Self {
            params: Argon2::new(
                argon2::Algorithm::Argon2id,
                argon2::Version::V0x13,
                argon2::Params::new(64 * 1024, 3, 4, Some(32)).unwrap(),
            ),
        }
    }

    /// Dev params: 1 iteration, 4 MB, 1 thread (fast login ~100ms).
    pub fn new_dev() -> Self {
        Self {
            params: Argon2::new(
                argon2::Algorithm::Argon2id,
                argon2::Version::V0x13,
                argon2::Params::new(4 * 1024, 1, 1, Some(32)).unwrap(),
            ),
        }
    }

    /// Hash a plaintext password. Returns PHC-format encoded string.
    pub fn hash(&self, plain: &str) -> anyhow::Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = self
            .params
            .hash_password(plain.as_bytes(), &salt)
            .map_err(|e| anyhow::anyhow!("hash password: {e}"))?;
        Ok(hash.to_string())
    }

    /// Verify a plaintext password against a PHC-format encoded hash.
    /// Also accepts the `$plain$` prefix used by seed data (dev only).
    pub fn verify(&self, encoded: &str, plain: &str) -> bool {
        // Handle seed-data plaintext marker.
        if let Some(stored_plain) = encoded.strip_prefix("$plain$") {
            return stored_plain == plain;
        }

        let parsed = match PasswordHash::new(encoded) {
            Ok(h) => h,
            Err(_) => return false,
        };
        self.params
            .verify_password(plain.as_bytes(), &parsed)
            .is_ok()
    }
}

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

    #[test]
    fn hash_and_verify() {
        let pw = Passwords::new_dev();
        let hash = pw.hash("test123").unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(pw.verify(&hash, "test123"));
        assert!(!pw.verify(&hash, "wrong"));
    }

    #[test]
    fn verify_plain_seed_password() {
        let pw = Passwords::new_dev();
        assert!(pw.verify("$plain$admin123", "admin123"));
        assert!(!pw.verify("$plain$admin123", "wrong"));
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
