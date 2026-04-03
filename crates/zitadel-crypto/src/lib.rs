use aes_gcm::{Aes256Gcm, KeyInit, Nonce, aead::Aead};
use std::collections::HashMap;

/// AES-256-GCM envelope encryption with key rotation support.
/// When no keys are configured, operates in plaintext passthrough mode (dev default).
pub struct SecretBox {
    active_key_id: String,
    keys: HashMap<String, [u8; 32]>,
}

pub struct SealedSecret {
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub key_id: String,
}

impl SecretBox {
    /// Create from a key ring. Pass empty ring for plaintext mode.
    pub fn new(active_key_id: &str, keys: &HashMap<String, String>) -> anyhow::Result<Self> {
        let mut decoded = HashMap::new();
        for (id, hex_key) in keys {
            let bytes =
                hex::decode(hex_key).map_err(|e| anyhow::anyhow!("key {id}: invalid hex: {e}"))?;
            if bytes.len() != 32 {
                anyhow::bail!("key {id}: must be 32 bytes (got {})", bytes.len());
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&bytes);
            decoded.insert(id.clone(), key);
        }

        if !active_key_id.is_empty() && !decoded.contains_key(active_key_id) {
            anyhow::bail!("active_key_id {active_key_id} not found in key ring");
        }

        Ok(Self {
            active_key_id: active_key_id.to_string(),
            keys: decoded,
        })
    }

    pub fn plaintext(&self) -> bool {
        self.keys.is_empty()
    }

    /// Encrypt plaintext with the active key. In plaintext mode, returns data as-is.
    pub fn seal(&self, plaintext: &[u8]) -> anyhow::Result<SealedSecret> {
        if self.plaintext() {
            return Ok(SealedSecret {
                ciphertext: plaintext.to_vec(),
                nonce: Vec::new(),
                key_id: String::new(),
            });
        }

        let key = self
            .keys
            .get(&self.active_key_id)
            .ok_or_else(|| anyhow::anyhow!("active key {} not found", self.active_key_id))?;

        let cipher =
            Aes256Gcm::new_from_slice(key).map_err(|e| anyhow::anyhow!("create cipher: {e}"))?;

        let mut nonce_bytes = [0u8; 12];
        rand::fill(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| anyhow::anyhow!("encrypt: {e}"))?;

        Ok(SealedSecret {
            ciphertext,
            nonce: nonce_bytes.to_vec(),
            key_id: self.active_key_id.clone(),
        })
    }

    /// Decrypt ciphertext using the key identified by key_id. Empty key_id = plaintext.
    pub fn open(&self, ciphertext: &[u8], nonce: &[u8], key_id: &str) -> anyhow::Result<Vec<u8>> {
        if key_id.is_empty() {
            return Ok(ciphertext.to_vec());
        }

        let key = self
            .keys
            .get(key_id)
            .ok_or_else(|| anyhow::anyhow!("key {key_id} not found in ring"))?;

        let cipher =
            Aes256Gcm::new_from_slice(key).map_err(|e| anyhow::anyhow!("create cipher: {e}"))?;

        let nonce = Nonce::from_slice(nonce);
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| anyhow::anyhow!("decrypt failed: {e}"))
    }
}

/// Generate a random hex string of the given byte length.
pub fn random_hex(byte_len: usize) -> String {
    let mut bytes = vec![0u8; byte_len];
    rand::fill(bytes.as_mut_slice());
    hex::encode(&bytes)
}

/// Hash a token for storage using SHA-256 and the shared `sha256:` prefix.
pub fn token_hash(token: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plaintext_mode() {
        let sb = SecretBox::new("", &HashMap::new()).unwrap();
        assert!(sb.plaintext());
        let sealed = sb.seal(b"hello").unwrap();
        assert_eq!(sealed.ciphertext, b"hello");
        assert!(sealed.key_id.is_empty());
        let opened = sb
            .open(&sealed.ciphertext, &sealed.nonce, &sealed.key_id)
            .unwrap();
        assert_eq!(opened, b"hello");
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let mut keys = HashMap::new();
        keys.insert("k1".to_string(), "a".repeat(64)); // 32 bytes hex = 64 chars of 'a' = 0xaa...
        let sb = SecretBox::new("k1", &keys).unwrap();
        assert!(!sb.plaintext());

        let sealed = sb.seal(b"secret data").unwrap();
        assert_ne!(sealed.ciphertext, b"secret data");
        assert_eq!(sealed.key_id, "k1");

        let opened = sb
            .open(&sealed.ciphertext, &sealed.nonce, &sealed.key_id)
            .unwrap();
        assert_eq!(opened, b"secret data");
    }

    #[test]
    fn key_rotation() {
        let mut keys = HashMap::new();
        keys.insert("k1".to_string(), "a".repeat(64));
        keys.insert("k2".to_string(), "b".repeat(64));

        // Encrypt with k1
        let sb1 = SecretBox::new("k1", &keys).unwrap();
        let sealed = sb1.seal(b"data").unwrap();
        assert_eq!(sealed.key_id, "k1");

        // New SecretBox with k2 as active can still decrypt k1
        let sb2 = SecretBox::new("k2", &keys).unwrap();
        let opened = sb2
            .open(&sealed.ciphertext, &sealed.nonce, &sealed.key_id)
            .unwrap();
        assert_eq!(opened, b"data");
    }

    #[test]
    fn wrong_key_fails() {
        let mut keys = HashMap::new();
        keys.insert("k1".to_string(), "a".repeat(64));
        let sb = SecretBox::new("k1", &keys).unwrap();
        let sealed = sb.seal(b"data").unwrap();

        let mut keys2 = HashMap::new();
        keys2.insert("k1".to_string(), "b".repeat(64)); // different key bytes
        let sb2 = SecretBox::new("k1", &keys2).unwrap();
        assert!(
            sb2.open(&sealed.ciphertext, &sealed.nonce, &sealed.key_id)
                .is_err()
        );
    }

    #[test]
    fn random_hex_length() {
        let h = random_hex(16);
        assert_eq!(h.len(), 32); // 16 bytes = 32 hex chars
    }

    #[test]
    fn token_hash_is_stable() {
        assert_eq!(token_hash("abc"), token_hash("abc"));
        assert_ne!(token_hash("abc"), token_hash("def"));
        assert!(token_hash("abc").starts_with("sha256:"));
    }
}
