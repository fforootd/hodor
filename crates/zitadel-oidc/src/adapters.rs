use crate::oidc::{ClientMetadata, ConsumedAuthRequest, NewAuthRequest, SigningKeys, UserClaims};
use crate::op::{AuthRequestStore, ClaimSource, ClientStore, KeyStore};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use zitadel_db::{
    Db, consume_oidc_auth_code_record, create_oidc_auth_request_record, get_oidc_client_record,
    load_user_claims_record,
};

#[derive(Clone)]
pub struct ZitadelOpStore {
    db: Db,
}

impl ZitadelOpStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    fn parse_string_list(raw: &str) -> Vec<String> {
        serde_json::from_str::<Vec<String>>(raw).unwrap_or_default()
    }
}

impl ClientStore for ZitadelOpStore {
    async fn find_client(
        &self,
        instance_id: &str,
        client_id: &str,
    ) -> anyhow::Result<Option<ClientMetadata>> {
        Ok(get_oidc_client_record(&self.db, instance_id, client_id)
            .await?
            .map(|record| ClientMetadata {
                client_id: client_id.to_string(),
                client_secret: record.client_secret,
                redirect_uris: Self::parse_string_list(&record.redirect_uris_json),
                grant_types: Self::parse_string_list(&record.grant_types_json),
                response_types: Self::parse_string_list(&record.response_types_json),
                state: record.state,
            }))
    }

    async fn authenticate_client_secret(
        &self,
        instance_id: &str,
        client_id: &str,
        client_secret: &str,
    ) -> anyhow::Result<bool> {
        Ok(self
            .find_client(instance_id, client_id)
            .await?
            .map(|client| client.client_secret == client_secret)
            .unwrap_or(false))
    }
}

impl AuthRequestStore for ZitadelOpStore {
    async fn create_auth_request(
        &self,
        instance_id: &str,
        request: &NewAuthRequest,
    ) -> anyhow::Result<String> {
        let auth_request_id = Uuid::new_v4().to_string();
        create_oidc_auth_request_record(
            &self.db,
            instance_id,
            &auth_request_id,
            &request.client_id,
            &request.redirect_uri,
            &request.scope,
            &request.state,
            &request.nonce,
            &request.response_type,
            &request.code_challenge,
            &request.code_challenge_method,
            &serde_json::to_string(&request.prompt).unwrap_or_else(|_| "[]".to_string()),
            &request.login_hint,
            request.max_age.map(|value| value as i64),
        )
        .await?;

        Ok(auth_request_id)
    }

    async fn consume_auth_code(
        &self,
        instance_id: &str,
        code: &str,
    ) -> anyhow::Result<Option<ConsumedAuthRequest>> {
        Ok(consume_oidc_auth_code_record(&self.db, instance_id, code)
            .await?
            .map(|record| ConsumedAuthRequest {
                auth_request_id: record.auth_request_id,
                user_id: record.user_id,
                client_id: record.client_id,
                redirect_uri: record.redirect_uri,
                scope: record.scope,
                nonce: record.nonce,
                code_challenge: record.code_challenge,
                auth_time: record.auth_time.and_then(|value| u64::try_from(value).ok()),
            }))
    }
}

impl ClaimSource for ZitadelOpStore {
    async fn load_user_claims(
        &self,
        instance_id: &str,
        subject: &str,
    ) -> anyhow::Result<Option<UserClaims>> {
        Ok(load_user_claims_record(&self.db, instance_id, subject)
            .await?
            .map(|record| UserClaims {
                subject: subject.to_string(),
                name: record.display_name,
                email_verified: !record.identifier.is_empty(),
                email: record.identifier,
            }))
    }
}

#[derive(Clone)]
pub struct RuntimeKeyStore {
    keys: Arc<RwLock<Option<Arc<SigningKeys>>>>,
}

impl RuntimeKeyStore {
    pub fn new() -> Self {
        let keys = Arc::new(RwLock::new(None));
        let background = keys.clone();
        tokio::spawn(async move {
            match SigningKeys::generate() {
                Ok(key) => {
                    tracing::info!(kid = %key.kid, "OIDC signing key generated");
                    *background.write().await = Some(key.shared());
                }
                Err(error) => tracing::error!(%error, "failed to generate OIDC signing key"),
            }
        });
        Self { keys }
    }
}

impl Default for RuntimeKeyStore {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyStore for RuntimeKeyStore {
    async fn active_signing_key(&self, _instance_id: &str) -> anyhow::Result<Arc<SigningKeys>> {
        for _ in 0..100 {
            let guard = self.keys.read().await;
            if let Some(key) = guard.clone() {
                return Ok(key);
            }
            drop(guard);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        anyhow::bail!("OIDC signing keys not ready after 5s")
    }
}

#[allow(dead_code)]
fn _assert_json_value_send_sync(_: &Value) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::{AuthRequestStore, ClientStore};

    #[tokio::test]
    async fn finds_client_and_parses_registered_redirect_uris() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();

        // Create the org that the app references.
        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, 'Test Org')")
            .bind("org-1")
            .bind(scoped.instance_id())
            .execute(scoped.pool())
            .await
            .unwrap();

        let redirect_uris = r#"["https://app.example/callback","https://app.example/alt"]"#;
        let grant_types = r#"["authorization_code","client_credentials"]"#;
        let response_types = r#"["code"]"#;

        let sql = format!(
            "INSERT INTO apps (id, instance_id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, {}, {}, $11)",
            scoped.json_bind(8),
            scoped.json_bind(9),
            scoped.json_bind(10),
        );
        sqlx::query(&sql)
            .bind("app-1")
            .bind(scoped.instance_id())
            .bind("org-1")
            .bind("Example App")
            .bind("web")
            .bind("client-1")
            .bind("secret-1")
            .bind(redirect_uris)
            .bind(grant_types)
            .bind(response_types)
            .bind("active")
            .execute(scoped.pool())
            .await
            .unwrap();

        let store = ZitadelOpStore::new(db);
        let client = store
            .find_client(zitadel_db::DEFAULT_INSTANCE_ID, "client-1")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(client.client_secret, "secret-1");
        assert_eq!(
            client.redirect_uris,
            vec![
                "https://app.example/callback".to_string(),
                "https://app.example/alt".to_string()
            ]
        );
        assert!(
            client
                .grant_types
                .iter()
                .any(|grant| grant == "client_credentials")
        );
    }

    #[tokio::test]
    async fn consume_auth_code_is_one_time() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();

        sqlx::query(
            "INSERT INTO oidc_auth_requests (id, instance_id, user_id, client_id, redirect_uri, scope, nonce, code_challenge, code, done, auth_time) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1, CURRENT_TIMESTAMP)",
        )
        .bind("auth-1")
        .bind(scoped.instance_id())
        .bind("user-1")
        .bind("client-1")
        .bind("https://app.example/callback")
        .bind("openid profile")
        .bind("nonce-1")
        .bind("challenge-1")
        .bind("code-1")
        .execute(scoped.pool())
        .await
        .unwrap();

        let store = ZitadelOpStore::new(db);
        let first = store
            .consume_auth_code(zitadel_db::DEFAULT_INSTANCE_ID, "code-1")
            .await
            .unwrap();
        let second = store
            .consume_auth_code(zitadel_db::DEFAULT_INSTANCE_ID, "code-1")
            .await
            .unwrap();

        assert!(first.is_some());
        assert!(second.is_none());
    }

    #[tokio::test]
    async fn consume_auth_code_returns_stored_auth_time() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();

        sqlx::query(
            "INSERT INTO oidc_auth_requests (id, instance_id, user_id, client_id, redirect_uri, scope, nonce, code_challenge, code, done, auth_time) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1, CURRENT_TIMESTAMP)",
        )
        .bind("auth-1")
        .bind(scoped.instance_id())
        .bind("user-1")
        .bind("client-1")
        .bind("https://app.example/callback")
        .bind("openid")
        .bind("nonce-1")
        .bind("challenge-1")
        .bind("code-1")
        .execute(scoped.pool())
        .await
        .unwrap();

        let store = ZitadelOpStore::new(db);
        let auth = store
            .consume_auth_code(zitadel_db::DEFAULT_INSTANCE_ID, "code-1")
            .await
            .unwrap()
            .unwrap();

        assert!(auth.auth_time.is_some());
    }
}
