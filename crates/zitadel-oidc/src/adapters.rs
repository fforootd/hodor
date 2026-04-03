use crate::oidc::{ClientMetadata, ConsumedAuthRequest, NewAuthRequest, SigningKeys, UserClaims};
use crate::op::{AuthRequestStore, ClaimSource, ClientStore, KeyStore};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;
use zitadel_db::Db;

#[derive(Clone)]
pub struct ZitadelOpStore {
    db: Db,
}

impl ZitadelOpStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    fn scoped(&self, instance_id: &str) -> zitadel_db::scoped::ScopedDb {
        self.db.scoped(instance_id.to_string())
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
        let scoped = self.scoped(instance_id);
        let sql = format!(
            "SELECT COALESCE(client_secret, ''), COALESCE({}, '[]'), COALESCE({}, '[]'), COALESCE({}, '[]'), COALESCE(state, 'active') \
             FROM apps WHERE instance_id = $1 AND client_id = $2",
            scoped.as_text("redirect_uris"),
            scoped.as_text("grant_types"),
            scoped.as_text("response_types"),
        );
        let row: Option<(String, String, String, String, String)> = sqlx::query_as(&sql)
            .bind(scoped.instance_id())
            .bind(client_id)
            .fetch_optional(scoped.pool())
            .await?;

        Ok(row.map(
            |(client_secret, redirect_uris, grant_types, response_types, state)| ClientMetadata {
                client_id: client_id.to_string(),
                client_secret,
                redirect_uris: Self::parse_string_list(&redirect_uris),
                grant_types: Self::parse_string_list(&grant_types),
                response_types: Self::parse_string_list(&response_types),
                state,
            },
        ))
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
        let scoped = self.scoped(instance_id);
        let auth_request_id = Uuid::new_v4().to_string();
        let sql = format!(
            "INSERT INTO oidc_auth_requests (id, instance_id, client_id, redirect_uri, scope, state, nonce, response_type, code_challenge, code_challenge_method, prompt, login_hint) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, {}, $12)",
            scoped.json_bind(11),
        );
        sqlx::query(&sql)
            .bind(&auth_request_id)
            .bind(scoped.instance_id())
            .bind(&request.client_id)
            .bind(&request.redirect_uri)
            .bind(&request.scope)
            .bind(&request.state)
            .bind(&request.nonce)
            .bind(&request.response_type)
            .bind(&request.code_challenge)
            .bind(&request.code_challenge_method)
            .bind(serde_json::to_string(&request.prompt).unwrap_or_else(|_| "[]".to_string()))
            .bind(&request.login_hint)
            .execute(scoped.pool())
            .await?;

        Ok(auth_request_id)
    }

    async fn consume_auth_code(
        &self,
        instance_id: &str,
        code: &str,
    ) -> anyhow::Result<Option<ConsumedAuthRequest>> {
        let scoped = self.scoped(instance_id);
        let mut tx = scoped.pool().begin().await?;
        let row: Option<(String, String, String, String, String, String, String)> = sqlx::query_as(
            "SELECT id, user_id, client_id, redirect_uri, scope, nonce, code_challenge \
             FROM oidc_auth_requests WHERE instance_id = $1 AND code = $2 AND done = 1",
        )
        .bind(scoped.instance_id())
        .bind(code)
        .fetch_optional(&mut *tx)
        .await?;

        let Some((auth_request_id, user_id, client_id, redirect_uri, scope, nonce, code_challenge)) =
            row
        else {
            tx.rollback().await?;
            return Ok(None);
        };

        sqlx::query("DELETE FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2")
            .bind(scoped.instance_id())
            .bind(&auth_request_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(Some(ConsumedAuthRequest {
            auth_request_id,
            user_id,
            client_id,
            redirect_uri,
            scope,
            nonce,
            code_challenge,
        }))
    }
}

impl ClaimSource for ZitadelOpStore {
    async fn load_user_claims(
        &self,
        instance_id: &str,
        subject: &str,
    ) -> anyhow::Result<Option<UserClaims>> {
        let scoped = self.scoped(instance_id);
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT identifier, display_name FROM users WHERE instance_id = $1 AND id = $2",
        )
        .bind(scoped.instance_id())
        .bind(subject)
        .fetch_optional(scoped.pool())
        .await?;

        Ok(row.map(|(email, name)| UserClaims {
            subject: subject.to_string(),
            name,
            email: email.clone(),
            email_verified: !email.is_empty(),
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
            "INSERT INTO oidc_auth_requests (id, instance_id, user_id, client_id, redirect_uri, scope, nonce, code_challenge, code, done) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 1)",
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
}
