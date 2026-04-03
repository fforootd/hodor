use base64::Engine;
use jsonwebtoken::{DecodingKey, EncodingKey};
use rand::rngs::OsRng;
use rsa::traits::PublicKeyParts;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenIdConfiguration {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    #[serde(default)]
    pub userinfo_endpoint: String,
    pub jwks_uri: String,
    #[serde(default)]
    pub revocation_endpoint: String,
    #[serde(default)]
    pub end_session_endpoint: String,
    #[serde(default)]
    pub response_types_supported: Vec<String>,
    #[serde(default)]
    pub grant_types_supported: Vec<String>,
    #[serde(default)]
    pub subject_types_supported: Vec<String>,
    #[serde(default)]
    pub id_token_signing_alg_values_supported: Vec<String>,
    #[serde(default)]
    pub scopes_supported: Vec<String>,
    #[serde(default)]
    pub token_endpoint_auth_methods_supported: Vec<String>,
    #[serde(default)]
    pub code_challenge_methods_supported: Vec<String>,
    #[serde(default)]
    pub claims_supported: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonWebKeySet {
    pub keys: Vec<JsonWebKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonWebKey {
    pub kty: String,
    #[serde(rename = "use")]
    pub use_: String,
    pub kid: String,
    pub alg: String,
    pub n: String,
    pub e: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientMetadata {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewAuthRequest {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub scope: String,
    pub state: String,
    pub nonce: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub prompt: Vec<String>,
    pub login_hint: String,
    pub max_age: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumedAuthRequest {
    pub auth_request_id: String,
    pub user_id: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub nonce: String,
    pub code_challenge: String,
    pub auth_time: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserClaims {
    pub subject: String,
    pub name: String,
    pub email: String,
    pub email_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    pub scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserInfoResponse {
    pub sub: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub email: String,
    pub email_verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolError {
    pub status_code: u16,
    pub body: ErrorResponse,
}

impl ProtocolError {
    pub fn new(status_code: u16, error: impl Into<String>, description: impl Into<String>) -> Self {
        let description = description.into();
        Self {
            status_code,
            body: ErrorResponse {
                error: error.into(),
                error_description: if description.is_empty() {
                    None
                } else {
                    Some(description)
                },
            },
        }
    }

    pub fn invalid_request(description: impl Into<String>) -> Self {
        Self::new(400, "invalid_request", description)
    }

    pub fn invalid_client(description: impl Into<String>) -> Self {
        Self::new(401, "invalid_client", description)
    }

    pub fn invalid_grant(description: impl Into<String>) -> Self {
        Self::new(400, "invalid_grant", description)
    }

    pub fn unsupported_grant_type(description: impl Into<String>) -> Self {
        Self::new(400, "unsupported_grant_type", description)
    }

    pub fn unsupported_response_type(description: impl Into<String>) -> Self {
        Self::new(400, "unsupported_response_type", description)
    }

    pub fn unauthorized_client(description: impl Into<String>) -> Self {
        Self::new(400, "unauthorized_client", description)
    }

    pub fn server_error(description: impl Into<String>) -> Self {
        Self::new(500, "server_error", description)
    }

    pub fn temporarily_unavailable(description: impl Into<String>) -> Self {
        Self::new(503, "temporarily_unavailable", description)
    }
}

pub struct SigningKeys {
    pub kid: String,
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
    pub n: String,
    pub e: String,
}

impl SigningKeys {
    pub fn generate() -> anyhow::Result<Self> {
        let bits = 2048;
        let private_key = rsa::RsaPrivateKey::new(&mut OsRng, bits)
            .map_err(|error| anyhow::anyhow!("generate RSA key: {error}"))?;
        let public_key = private_key.to_public_key();

        let private_pem =
            rsa::pkcs8::EncodePrivateKey::to_pkcs8_pem(&private_key, rsa::pkcs8::LineEnding::LF)
                .map_err(|error| anyhow::anyhow!("encode private key: {error}"))?;
        let public_pem =
            rsa::pkcs8::EncodePublicKey::to_public_key_pem(&public_key, rsa::pkcs8::LineEnding::LF)
                .map_err(|error| anyhow::anyhow!("encode public key: {error}"))?;

        let encoding = EncodingKey::from_rsa_pem(private_pem.as_bytes())
            .map_err(|error| anyhow::anyhow!("create encoding key: {error}"))?;
        let decoding = DecodingKey::from_rsa_pem(public_pem.as_bytes())
            .map_err(|error| anyhow::anyhow!("create decoding key: {error}"))?;

        let n =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be());
        let e =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be());

        Ok(Self {
            kid: uuid::Uuid::new_v4().to_string(),
            encoding,
            decoding,
            n,
            e,
        })
    }

    pub fn jwk(&self) -> JsonWebKey {
        JsonWebKey {
            kty: "RSA".to_string(),
            use_: "sig".to_string(),
            kid: self.kid.clone(),
            alg: "RS256".to_string(),
            n: self.n.clone(),
            e: self.e.clone(),
        }
    }

    pub fn shared(self) -> Arc<Self> {
        Arc::new(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: u64,
    pub iat: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_time: Option<u64>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub nonce: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: u64,
    pub iat: u64,
    pub scope: String,
    pub client_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefreshTokenClaims {
    pub iss: String,
    pub sub: String,
    pub aud: String,
    pub exp: u64,
    pub iat: u64,
    pub scope: String,
    pub client_id: String,
}

pub fn now_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs()
}

pub fn s256_challenge(verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_s256_matches_known_value() {
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let expected = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert_eq!(s256_challenge(verifier), expected);
    }

    #[test]
    fn protocol_error_omits_empty_description() {
        let err = ProtocolError::invalid_request("");
        assert!(err.body.error_description.is_none());
    }
}
