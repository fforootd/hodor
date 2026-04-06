use anyhow::{Context, Result, bail};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};

/// Well-known dev license key. Accepted only when the server is in dev mode.
/// This avoids requiring a real signed JWT for local development.
pub const DEV_LICENSE_KEY: &str = "zitadel-dev-cloud-license-do-not-use-in-production";

/// Claims embedded in a Zitadel Cloud license key.
///
/// Production license keys are Ed25519-signed JWTs issued by Zitadel.
/// The public key used to verify signatures is embedded in this binary.
/// The key encodes what the licensee is entitled to and when the license
/// expires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseClaims {
    /// Licensee identifier (customer or org ID).
    pub sub: String,
    /// Issued-at (Unix timestamp).
    pub iat: u64,
    /// Expiration (Unix timestamp). 0 = no expiry.
    pub exp: u64,
    /// Licensed features (e.g. ["billing", "support"] or ["*"] for all).
    pub features: Vec<String>,
    /// Maximum number of cloud-managed instances. 0 = unlimited.
    #[serde(default)]
    pub max_instances: u64,
}

// ── Embedded verification key ──────────────────────────────────────────
//
// Production license JWTs are signed with Ed25519. The signing private key
// lives in Zitadel's license-issuing service (never in this repo).
// Only the public key is embedded here for verification.
//
// To regenerate the keypair:
//   openssl genpkey -algorithm Ed25519 -out license-signing.pem
//   openssl pkey -in license-signing.pem -pubout -out keys/license-verify.pem
// Or with Python:
//   python3 -c "from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey; ..."
//
// The private key MUST NOT be committed to the repository.

/// Embedded Ed25519 public key (SPKI PEM) for verifying production license JWTs.
const VERIFY_KEY_PEM: &str = include_str!("../keys/license-verify.pem");

/// Validate a license key. In dev mode, accepts the well-known dev key.
/// In production, verifies the Ed25519 JWT signature.
pub fn validate(key: &str, is_dev: bool) -> Result<LicenseClaims> {
    if key.is_empty() {
        bail!("cloud.license_key is required when cloud.enabled = true");
    }

    if is_dev {
        return validate_dev(key);
    }

    validate_production(key)
}

/// Dev mode: accept the well-known dev key (or any key) with wildcard claims.
fn validate_dev(key: &str) -> Result<LicenseClaims> {
    if key != DEV_LICENSE_KEY {
        tracing::warn!(
            "dev mode accepts any cloud.license_key, but the canonical dev key is: {DEV_LICENSE_KEY}"
        );
    }

    Ok(LicenseClaims {
        sub: "dev".into(),
        iat: 0,
        exp: 0,
        features: vec!["*".into()],
        max_instances: 0,
    })
}

/// Production: verify JWT signature against the embedded Ed25519 public key.
fn validate_production(key: &str) -> Result<LicenseClaims> {
    let decoding_key = DecodingKey::from_ed_pem(VERIFY_KEY_PEM.as_bytes())
        .context("failed to parse embedded license verification key")?;

    let mut validation = Validation::new(Algorithm::EdDSA);
    // We validate exp ourselves (0 = no expiry).
    validation.validate_exp = false;
    validation.required_spec_claims.clear();

    let token_data = decode::<LicenseClaims>(key, &decoding_key, &validation)
        .context("invalid license key — signature verification failed")?;

    let claims = token_data.claims;

    // Check expiry if set (0 = no expiry).
    if claims.exp > 0 {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        if now > claims.exp {
            bail!(
                "license key expired at {} (current time: {now})",
                claims.exp
            );
        }
    }

    if claims.features.is_empty() {
        bail!("license key has no features — contact Zitadel for a valid key");
    }

    Ok(claims)
}

/// Check whether a specific feature is included in the license.
pub fn has_feature(claims: &LicenseClaims, feature: &str) -> bool {
    claims.features.iter().any(|f| f == "*" || f == feature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_key_rejected() {
        assert!(validate("", true).is_err());
        assert!(validate("", false).is_err());
    }

    #[test]
    fn dev_key_accepted_in_dev_mode() {
        let claims = validate(DEV_LICENSE_KEY, true).unwrap();
        assert_eq!(claims.sub, "dev");
        assert!(has_feature(&claims, "billing"));
        assert!(has_feature(&claims, "anything"));
    }

    #[test]
    fn any_key_accepted_in_dev_mode() {
        let claims = validate("some-random-key", true).unwrap();
        assert_eq!(claims.sub, "dev");
    }

    #[test]
    fn production_rejects_bad_jwt() {
        let err = validate("not-a-valid-jwt", false).unwrap_err();
        assert!(
            err.to_string().contains("signature verification failed"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn embedded_key_parses() {
        // Verify the embedded PEM is valid.
        DecodingKey::from_ed_pem(VERIFY_KEY_PEM.as_bytes())
            .expect("embedded license-verify.pem should be a valid Ed25519 public key");
    }

    #[test]
    fn wildcard_feature_matches_all() {
        let claims = LicenseClaims {
            sub: "test".into(),
            iat: 0,
            exp: 0,
            features: vec!["*".into()],
            max_instances: 0,
        };
        assert!(has_feature(&claims, "billing"));
        assert!(has_feature(&claims, "infra"));
    }

    #[test]
    fn specific_feature_matching() {
        let claims = LicenseClaims {
            sub: "test".into(),
            iat: 0,
            exp: 0,
            features: vec!["billing".into(), "support".into()],
            max_instances: 10,
        };
        assert!(has_feature(&claims, "billing"));
        assert!(has_feature(&claims, "support"));
        assert!(!has_feature(&claims, "infra"));
    }
}
