use crate::adapters::RuntimeKeyStore;
use crate::oidc::SigningKeys;
use crate::op::{KeyStore, NewStoredToken, StoredToken, TokenStore};
use std::sync::Arc;
use zitadel_app::repo::{
    OidcKeyRepository, OidcNewSigningKey, OidcNewToken, OidcSigningKeyRecord, OidcTokenRepository,
};
use zitadel_config::oidc::OidcConfig;
use zitadel_crypto::SecretBox;

#[derive(Clone, Default)]
pub struct NoopTokenStore;

impl TokenStore for NoopTokenStore {
    fn enforces_storage(&self) -> bool {
        false
    }

    async fn store_token(&self, _instance_id: &str, _token: &NewStoredToken) -> anyhow::Result<()> {
        Ok(())
    }

    async fn lookup_active_token(
        &self,
        _instance_id: &str,
        _raw_token: &str,
    ) -> anyhow::Result<Option<StoredToken>> {
        Ok(None)
    }

    async fn revoke_token_by_id(&self, _instance_id: &str, _token_id: &str) -> anyhow::Result<()> {
        Ok(())
    }

    async fn revoke_refresh_family(
        &self,
        _instance_id: &str,
        _refresh_family_id: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn revoke_session_tokens(
        &self,
        _instance_id: &str,
        _session_id: &str,
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Clone)]
pub struct PersistentTokenStore {
    repo: Option<Arc<dyn OidcTokenRepository>>,
}

impl PersistentTokenStore {
    pub fn new(repo: Arc<dyn OidcTokenRepository>) -> Self {
        Self { repo: Some(repo) }
    }

    pub fn disabled() -> Self {
        Self { repo: None }
    }
}

impl TokenStore for PersistentTokenStore {
    fn enforces_storage(&self) -> bool {
        self.repo.is_some()
    }

    async fn store_token(&self, instance_id: &str, token: &NewStoredToken) -> anyhow::Result<()> {
        let Some(repo) = self.repo.as_ref() else {
            return Ok(());
        };
        let scopes_json = scopes_to_json(&token.scope)?;
        repo.store_token(
            instance_id,
            &OidcNewToken {
                token_id: token.token_id.clone(),
                token_type: token.token_type.clone(),
                token_hash: zitadel_crypto::token_hash(&token.raw_token),
                user_id: token.user_id.clone(),
                session_id: token.session_id.clone(),
                client_id: token.client_id.clone(),
                application_id: token.application_id.clone(),
                scope_json: scopes_json,
                auth_method: token.auth_method.clone(),
                refresh_family_id: token.refresh_family_id.clone(),
                expires_in_secs: token.expires_in_secs,
            },
        )
        .await
    }

    async fn lookup_active_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<StoredToken>> {
        let Some(repo) = self.repo.as_ref() else {
            return Ok(None);
        };
        let hashed = zitadel_crypto::token_hash(raw_token);
        let result = repo.lookup_active_token(instance_id, &hashed).await?;
        Ok(result.map(|r| StoredToken {
            token_id: r.token_id,
            token_type: r.token_type,
            user_id: r.user_id,
            session_id: r.session_id,
            client_id: r.client_id,
            application_id: r.application_id,
            scope: r.scope,
            refresh_family_id: r.refresh_family_id,
        }))
    }

    async fn revoke_token_by_id(&self, instance_id: &str, token_id: &str) -> anyhow::Result<()> {
        let Some(repo) = self.repo.as_ref() else {
            return Ok(());
        };
        repo.revoke_token_by_id(instance_id, token_id).await
    }

    async fn revoke_refresh_family(
        &self,
        instance_id: &str,
        refresh_family_id: &str,
    ) -> anyhow::Result<()> {
        let Some(repo) = self.repo.as_ref() else {
            return Ok(());
        };
        repo.revoke_refresh_family(instance_id, refresh_family_id)
            .await
    }

    async fn revoke_session_tokens(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<()> {
        let Some(repo) = self.repo.as_ref() else {
            return Ok(());
        };
        repo.revoke_session_tokens(instance_id, session_id).await
    }
}

#[derive(Clone)]
pub struct PersistentKeyStore {
    repo: Option<Arc<dyn OidcKeyRepository>>,
    secret_box: Option<Arc<SecretBox>>,
    oidc_config: OidcConfig,
    runtime: RuntimeKeyStore,
}

impl PersistentKeyStore {
    pub fn new(
        repo: Arc<dyn OidcKeyRepository>,
        secret_box: Arc<SecretBox>,
        oidc_config: OidcConfig,
    ) -> Self {
        Self {
            repo: Some(repo),
            secret_box: Some(secret_box),
            oidc_config,
            runtime: RuntimeKeyStore::new(),
        }
    }

    pub fn ephemeral(oidc_config: OidcConfig) -> Self {
        Self {
            repo: None,
            secret_box: None,
            oidc_config,
            runtime: RuntimeKeyStore::new(),
        }
    }

    async fn list_active_records(
        &self,
        instance_id: &str,
    ) -> anyhow::Result<Vec<OidcSigningKeyRecord>> {
        let Some(repo) = self.repo.as_ref() else {
            return Ok(Vec::new());
        };
        repo.list_active_keys(instance_id).await
    }

    async fn create_signing_key(&self, instance_id: &str) -> anyhow::Result<Arc<SigningKeys>> {
        let Some(repo) = self.repo.as_ref() else {
            return self.runtime.active_signing_key(instance_id).await;
        };
        let Some(secret_box) = self.secret_box.as_ref() else {
            return self.runtime.active_signing_key(instance_id).await;
        };
        let key = SigningKeys::generate_with_rsa_bits(self.oidc_config.key_size.max(2048))?;
        let sealed = secret_box.seal(&key.private_pem)?;
        let overlap_secs = self
            .oidc_config
            .private_key_lifetime_secs
            .saturating_add(self.oidc_config.public_key_lifetime_secs)
            .max(1);

        repo.create_signing_key(
            instance_id,
            &OidcNewSigningKey {
                kid: key.kid.clone(),
                algorithm: key.alg.clone(),
                encryption_key_id: sealed.key_id,
                ciphertext: sealed.ciphertext,
                nonce: sealed.nonce,
                public_key: key.public_pem.clone(),
                expires_in_secs: overlap_secs,
            },
        )
        .await?;

        Ok(key.shared())
    }

    fn key_rotation_window_secs(&self) -> u64 {
        self.oidc_config.private_key_lifetime_secs.max(1)
    }

    fn can_use_persistent_store(&self) -> bool {
        self.repo.is_some() && self.secret_box.is_some()
    }

    fn load_key(&self, record: &OidcSigningKeyRecord) -> anyhow::Result<Arc<SigningKeys>> {
        let secret_box = self
            .secret_box
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing secret box"))?;
        let private_pem =
            secret_box.open(&record.ciphertext, &record.nonce, &record.encryption_key_id)?;
        if record.public_key.is_empty() {
            anyhow::bail!("missing public key for {}", record.kid);
        }
        Ok(SigningKeys::from_pems(
            record.kid.clone(),
            record.algorithm.clone(),
            private_pem,
            record.public_key.clone(),
        )?
        .shared())
    }
}

impl KeyStore for PersistentKeyStore {
    async fn active_signing_key(&self, instance_id: &str) -> anyhow::Result<Arc<SigningKeys>> {
        if !self.can_use_persistent_store() {
            return self.runtime.active_signing_key(instance_id).await;
        }

        let records = self.list_active_records(instance_id).await?;
        let now = crate::oidc::now_epoch_seconds();
        if let Some(record) = records.first()
            && now.saturating_sub(record.created_at_epoch) < self.key_rotation_window_secs()
        {
            return self.load_key(record);
        }

        self.create_signing_key(instance_id).await
    }

    async fn signing_keys(&self, instance_id: &str) -> anyhow::Result<Vec<Arc<SigningKeys>>> {
        if !self.can_use_persistent_store() {
            return Ok(vec![self.runtime.active_signing_key(instance_id).await?]);
        }

        let _ = self.active_signing_key(instance_id).await?;
        let records = self.list_active_records(instance_id).await?;
        let mut keys = Vec::with_capacity(records.len());
        for record in &records {
            keys.push(self.load_key(record)?);
        }
        if keys.is_empty() {
            keys.push(self.create_signing_key(instance_id).await?);
        }
        Ok(keys)
    }
}

fn scopes_to_json(scope: &str) -> anyhow::Result<String> {
    let scopes = scope
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    Ok(serde_json::to_string(&scopes)?)
}
