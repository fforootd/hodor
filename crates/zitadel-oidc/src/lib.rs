pub mod discovery;
pub mod jwks;
pub mod authorize;
pub mod token;
pub mod userinfo;

use axum::Router;
use zitadel_db::Db;
use jsonwebtoken::{EncodingKey, DecodingKey};
use std::sync::Arc;
use tokio::sync::RwLock;

/// OIDC provider state.
#[derive(Clone)]
pub struct OidcState {
    pub db: Db,
    pub issuer: String,
    pub keys: Arc<RwLock<Option<SigningKeys>>>,
}

/// RSA signing key pair for JWT tokens.
pub struct SigningKeys {
    pub kid: String,
    pub encoding: EncodingKey,
    pub decoding: DecodingKey,
    pub n: String,  // RSA modulus (base64url)
    pub e: String,  // RSA exponent (base64url)
}

impl SigningKeys {
    /// Generate a new RSA-2048 key pair.
    pub fn generate() -> anyhow::Result<Self> {
        use rand::rngs::OsRng;
        use base64::Engine;
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;

        // Use jsonwebtoken's built-in RSA key generation via the rsa crate.
        let bits = 2048;
        let private_key = rsa::RsaPrivateKey::new(&mut OsRng, bits)
            .map_err(|e| anyhow::anyhow!("generate RSA key: {e}"))?;
        let public_key = private_key.to_public_key();

        let private_der = rsa::pkcs8::EncodePrivateKey::to_pkcs8_der(&private_key)
            .map_err(|e| anyhow::anyhow!("encode private key: {e}"))?;
        let public_der = rsa::pkcs8::EncodePublicKey::to_public_key_der(&public_key)
            .map_err(|e| anyhow::anyhow!("encode public key: {e}"))?;

        let encoding = EncodingKey::from_rsa_der(private_der.as_bytes());
        let decoding = DecodingKey::from_rsa_der(public_der.as_ref());

        // Extract n and e for JWKS.
        use rsa::traits::PublicKeyParts;
        let n_bytes = public_key.n().to_bytes_be();
        let e_bytes = public_key.e().to_bytes_be();
        let n = URL_SAFE_NO_PAD.encode(&n_bytes);
        let e = URL_SAFE_NO_PAD.encode(&e_bytes);

        let kid = uuid::Uuid::new_v4().to_string();

        Ok(Self { kid, encoding, decoding, n, e })
    }
}

impl OidcState {
    /// Create OIDC state with lazy key generation (non-blocking).
    /// Keys are generated in a background task so startup isn't delayed.
    pub fn new(db: Db, issuer: String) -> Self {
        let keys = Arc::new(RwLock::new(None));
        let keys_clone = keys.clone();

        // Generate RSA key in background to avoid blocking startup (~3s).
        tokio::spawn(async move {
            match SigningKeys::generate() {
                Ok(k) => {
                    tracing::info!(kid = %k.kid, "OIDC signing key generated");
                    *keys_clone.write().await = Some(k);
                }
                Err(e) => tracing::error!(error = %e, "failed to generate OIDC signing key"),
            }
        });

        Self { db, issuer, keys }
    }

    /// Get signing keys, waiting if still generating.
    pub async fn signing_keys(&self) -> anyhow::Result<tokio::sync::RwLockReadGuard<'_, Option<SigningKeys>>> {
        // Spin briefly if keys aren't ready yet.
        for _ in 0..100 {
            let guard = self.keys.read().await;
            if guard.is_some() {
                return Ok(guard);
            }
            drop(guard);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        anyhow::bail!("OIDC signing keys not ready after 5s")
    }
}

/// Build OIDC provider routes.
pub fn routes(state: OidcState) -> Router {
    Router::new()
        .merge(discovery::routes(state.clone()))
        .merge(jwks::routes(state.clone()))
        .merge(authorize::routes(state.clone()))
        .merge(token::routes(state.clone()))
        .merge(userinfo::routes(state.clone()))
}
