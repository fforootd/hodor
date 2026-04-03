use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

const SECURE_COOKIE_NAME: &str = "__Host-zitadel_session";
const DEV_COOKIE_NAME: &str = "__zitadel_session";
/// Default max age for session cookies (24h). Can be overridden via `SessionConfig.max_age_secs`.
pub const DEFAULT_MAX_AGE: i64 = 86400;

/// Cookie configuration with HMAC key rotation support.
#[derive(Clone)]
pub struct CookieConfig {
    /// Ordered list of HMAC keys. First signs, all verify.
    pub secrets: Vec<String>,
    /// Whether to use Secure flag (derived from external_domain).
    pub secure: bool,
    /// Maximum cookie/session age in seconds.
    pub max_age: i64,
}

impl CookieConfig {
    pub fn new(secrets: Vec<String>, external_domain: &str, force_insecure_cookies: bool) -> Self {
        Self::new_with_max_age(
            secrets,
            external_domain,
            force_insecure_cookies,
            DEFAULT_MAX_AGE,
        )
    }

    pub fn new_with_max_age(
        secrets: Vec<String>,
        external_domain: &str,
        force_insecure_cookies: bool,
        max_age: i64,
    ) -> Self {
        let secure = !force_insecure_cookies
            && !external_domain.is_empty()
            && external_domain != "localhost"
            && external_domain != "127.0.0.1";

        let secrets = if secrets.is_empty() {
            vec![zitadel_crypto::random_hex(32)]
        } else {
            secrets
        };

        Self {
            secrets,
            secure,
            max_age,
        }
    }

    pub fn cookie_name(&self) -> &str {
        if self.secure {
            SECURE_COOKIE_NAME
        } else {
            DEV_COOKIE_NAME
        }
    }

    /// All accepted cookie names (for reading during transition).
    pub fn all_cookie_names(&self) -> &[&str] {
        &[SECURE_COOKIE_NAME, DEV_COOKIE_NAME]
    }
}

/// Sign a token for cookie storage: base64url(token.hex(hmac)).
pub fn sign(token: &str, secret: &str) -> String {
    let mut mac =
        HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
    mac.update(token.as_bytes());
    let sig = hex::encode(mac.finalize().into_bytes());
    let payload = format!("{token}.{sig}");
    URL_SAFE_NO_PAD.encode(payload.as_bytes())
}

/// Verify a signed cookie value against all secrets (key rotation).
/// Returns the raw token if any key validates.
pub fn verify(cookie_value: &str, secrets: &[String]) -> Option<String> {
    let decoded = URL_SAFE_NO_PAD.decode(cookie_value).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let (token, provided_sig) = decoded.split_once('.')?;

    for secret in secrets {
        let mut mac =
            HmacSha256::new_from_slice(secret.as_bytes()).expect("HMAC accepts any key length");
        mac.update(token.as_bytes());
        let expected_sig = hex::encode(mac.finalize().into_bytes());

        // Constant-time comparison via hmac crate isn't needed here since
        // we're comparing hex strings, but let's be safe.
        if constant_time_eq(provided_sig.as_bytes(), expected_sig.as_bytes()) {
            return Some(token.to_string());
        }
    }

    None
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_roundtrip() {
        let secret = "test-secret-key-for-hmac";
        let token = "session-token-123";
        let signed = sign(token, secret);
        let verified = verify(&signed, &[secret.to_string()]);
        assert_eq!(verified, Some(token.to_string()));
    }

    #[test]
    fn verify_wrong_secret_fails() {
        let signed = sign("token", "secret1");
        let result = verify(&signed, &["secret2".to_string()]);
        assert_eq!(result, None);
    }

    #[test]
    fn verify_key_rotation() {
        let signed = sign("token", "old-key");
        // Verify against both old and new keys
        let result = verify(&signed, &["new-key".to_string(), "old-key".to_string()]);
        assert_eq!(result, Some("token".to_string()));
    }

    #[test]
    fn verify_garbage_returns_none() {
        assert_eq!(verify("not-base64!!!", &["secret".to_string()]), None);
        assert_eq!(verify("", &["secret".to_string()]), None);
    }

    #[test]
    fn cookie_config_dev() {
        let cfg = CookieConfig::new(vec![], "localhost", false);
        assert!(!cfg.secure);
        assert_eq!(cfg.cookie_name(), DEV_COOKIE_NAME);
        assert!(!cfg.secrets.is_empty()); // Random key generated
    }

    #[test]
    fn cookie_config_production() {
        let cfg = CookieConfig::new(vec!["my-secret".into()], "auth.example.com", false);
        assert!(cfg.secure);
        assert_eq!(cfg.cookie_name(), SECURE_COOKIE_NAME);
    }

    #[test]
    fn cookie_config_force_insecure() {
        let cfg = CookieConfig::new(vec!["my-secret".into()], "auth.example.com", true);
        assert!(!cfg.secure);
        assert_eq!(cfg.cookie_name(), DEV_COOKIE_NAME);
    }
}
