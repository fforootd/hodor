use crate::oidc::{ClientMetadata, ConsumedAuthRequest, NewAuthRequest, SigningKeys, UserClaims};
use crate::op::{AuthRequestStore, ClaimSource, ClientStore, KeyStore};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use zitadel_app::repo::{OidcAuthRequest, OidcRepository};

#[derive(Clone)]
pub struct ZitadelOpStore {
    repo: Arc<dyn OidcRepository>,
}

impl ZitadelOpStore {
    pub fn new(repo: Arc<dyn OidcRepository>) -> Self {
        Self { repo }
    }
}

impl ClientStore for ZitadelOpStore {
    async fn find_client(
        &self,
        instance_id: &str,
        client_id: &str,
    ) -> anyhow::Result<Option<ClientMetadata>> {
        Ok(self
            .repo
            .find_client(instance_id, client_id)
            .await?
            .map(|info| ClientMetadata {
                app_id: info.app_id,
                client_id: info.client_id,
                client_secret: info.client_secret.unwrap_or_default(),
                redirect_uris: info.redirect_uris,
                post_logout_redirect_uris: info.post_logout_redirect_uris,
                grant_types: info.grant_types,
                response_types: info.response_types,
                state: info.state,
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
        let app_request = OidcAuthRequest {
            id: String::new(),
            user_id: None,
            session_id: None,
            client_id: request.client_id.clone(),
            redirect_uri: request.redirect_uri.clone(),
            scope: request.scope.clone(),
            state: request.state.clone(),
            nonce: Some(request.nonce.clone()).filter(|s| !s.is_empty()),
            response_type: request.response_type.clone(),
            code_challenge: Some(request.code_challenge.clone()).filter(|s| !s.is_empty()),
            code_challenge_method: Some(request.code_challenge_method.clone())
                .filter(|s| !s.is_empty()),
            prompt: request.prompt.clone(),
            login_hint: Some(request.login_hint.clone()).filter(|s| !s.is_empty()),
            max_age: request.max_age,
            auth_time: None,
        };
        self.repo
            .create_auth_request(instance_id, &app_request)
            .await
    }

    async fn consume_auth_code(
        &self,
        instance_id: &str,
        code: &str,
    ) -> anyhow::Result<Option<ConsumedAuthRequest>> {
        Ok(self
            .repo
            .consume_auth_code(instance_id, code)
            .await?
            .map(|req| ConsumedAuthRequest {
                auth_request_id: req.id,
                user_id: req.user_id.unwrap_or_default(),
                session_id: req.session_id.unwrap_or_default(),
                client_id: req.client_id,
                redirect_uri: req.redirect_uri,
                scope: req.scope,
                state: req.state,
                nonce: req.nonce.unwrap_or_default(),
                response_type: req.response_type,
                code_challenge: req.code_challenge.unwrap_or_default(),
                code_challenge_method: req.code_challenge_method.unwrap_or_default(),
                prompt: req.prompt,
                login_hint: req.login_hint.unwrap_or_default(),
                max_age: req.max_age,
                auth_time: req.auth_time.as_deref().and_then(parse_auth_time),
            }))
    }
}

impl ClaimSource for ZitadelOpStore {
    async fn load_user_claims(
        &self,
        instance_id: &str,
        subject: &str,
    ) -> anyhow::Result<Option<UserClaims>> {
        Ok(self
            .repo
            .load_user_claims(instance_id, subject)
            .await?
            .map(|claims| UserClaims {
                subject: claims.sub,
                name: claims.name.unwrap_or_default(),
                email: claims.email.unwrap_or_default(),
                email_verified: claims.email_verified.unwrap_or(false),
            }))
    }
}

/// Parse auth_time from the DB — may be a Unix epoch or a timestamp string.
fn parse_auth_time(s: &str) -> Option<u64> {
    // Try numeric epoch first.
    if let Ok(epoch) = s.parse::<u64>() {
        return Some(epoch);
    }
    // Try common timestamp formats (SQLite: "YYYY-MM-DD HH:MM:SS", Postgres ISO).
    for fmt in &[
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f%:z",
        "%Y-%m-%dT%H:%M:%S",
    ] {
        if let Some(epoch) = parse_timestamp_to_epoch(s, fmt) {
            return Some(epoch);
        }
    }
    None
}

fn parse_timestamp_to_epoch(s: &str, _fmt: &str) -> Option<u64> {
    // Simple UTC timestamp parser for "YYYY-MM-DD HH:MM:SS" format.
    let parts: Vec<&str> = s.splitn(2, ' ').collect();
    if parts.len() != 2 {
        return None;
    }
    let date_parts: Vec<u32> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
    let time_parts: Vec<u32> = parts[1].split(':').filter_map(|p| p.parse().ok()).collect();
    if date_parts.len() != 3 || time_parts.len() != 3 {
        return None;
    }
    // Simplified: calculate days from Unix epoch (1970-01-01).
    let (year, month, day) = (date_parts[0], date_parts[1], date_parts[2]);
    let (hour, min, sec) = (time_parts[0], time_parts[1], time_parts[2]);
    let mut days: i64 = 0;
    for y in 1970..year {
        days += if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
    }
    let month_days = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    for m in 1..month {
        days += month_days[m as usize] as i64;
        if m == 2 && year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            days += 1;
        }
    }
    days += (day as i64) - 1;
    let secs = days * 86400 + (hour as i64) * 3600 + (min as i64) * 60 + (sec as i64);
    u64::try_from(secs).ok()
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

    async fn signing_keys(&self, instance_id: &str) -> anyhow::Result<Vec<Arc<SigningKeys>>> {
        Ok(vec![self.active_signing_key(instance_id).await?])
    }
}

#[allow(dead_code)]
fn _assert_json_value_send_sync(_: &Value) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::op::{AuthRequestStore, ClientStore};
    use zitadel_db::repos::adapters::DbOidcRepository;
    use zitadel_db::{DEFAULT_ORG_ID, Db};

    fn make_store(db: Db) -> ZitadelOpStore {
        ZitadelOpStore::new(Arc::new(DbOidcRepository::new(db)))
    }

    #[tokio::test]
    async fn finds_client_and_parses_registered_redirect_uris() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db, None).await.unwrap();
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
            .bind(DEFAULT_ORG_ID)
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

        let store = make_store(db);
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

        let store = make_store(db);
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

        let store = make_store(db);
        let auth = store
            .consume_auth_code(zitadel_db::DEFAULT_INSTANCE_ID, "code-1")
            .await
            .unwrap()
            .unwrap();

        assert!(auth.auth_time.is_some());
    }
}
