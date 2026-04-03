use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_db::scoped::ScopedDb;

/// Session record matching the sessions table schema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub id: String,
    pub user_id: String,
    pub org_id: String,
    pub token_hash: String,
    pub user_agent: String,
    pub ip_address: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
}

/// Session store backed by the sessions table.
pub struct SessionStore;

impl SessionStore {
    pub fn new(_db: zitadel_db::Db) -> Self {
        Self
    }

    /// Create a new session. Returns the session ID and raw token.
    /// `max_age_secs` controls session expiry (default: 86400 = 24h).
    pub async fn create(
        &self,
        scoped: &ScopedDb,
        user_id: &str,
        org_id: &str,
        user_agent: &str,
        ip_address: &str,
    ) -> anyhow::Result<(String, String)> {
        self.create_with_max_age(scoped, user_id, org_id, user_agent, ip_address, 86400)
            .await
    }

    /// Create a new session with a specific max age. Returns the session ID and raw token.
    pub async fn create_with_max_age(
        &self,
        scoped: &ScopedDb,
        user_id: &str,
        org_id: &str,
        user_agent: &str,
        ip_address: &str,
        max_age_secs: u64,
    ) -> anyhow::Result<(String, String)> {
        let session_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let token_hash = hash_token(&token);

        let org = if org_id.is_empty() { "_global" } else { org_id };
        let expires_expr = match scoped.dialect() {
            zitadel_db::Dialect::Postgres => {
                format!("CURRENT_TIMESTAMP + INTERVAL '{max_age_secs} seconds'")
            }
            zitadel_db::Dialect::Sqlite => {
                format!("datetime(CURRENT_TIMESTAMP, '+{max_age_secs} seconds')")
            }
        };
        let sql = format!(
            "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, created_at, expires_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, {}, {})",
            scoped.timestamp_now(),
            expires_expr,
        );

        sqlx::query(&sql)
            .bind(&session_id)
            .bind(scoped.instance_id())
            .bind(user_id)
            .bind(org)
            .bind(&token_hash)
            .bind(user_agent)
            .bind(ip_address)
            .execute(scoped.pool())
            .await?;

        Ok((session_id, token))
    }

    /// Look up a session by token hash. Returns None if not found or revoked/expired.
    pub async fn find_by_token(
        &self,
        scoped: &ScopedDb,
        token: &str,
    ) -> anyhow::Result<Option<SessionRecord>> {
        let token_hash = hash_token(token);
        let created_at = scoped.as_text("created_at");
        let expires_at = scoped.as_text("expires_at");
        let revoked_at = scoped.as_text("revoked_at");
        let sql = format!(
            "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, {created_at}, {expires_at}, {revoked_at} \
             FROM sessions \
             WHERE instance_id = $1 AND token_hash = $2 AND revoked_at IS NULL \
             AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP)"
        );

        let row: Option<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(&sql)
            .bind(scoped.instance_id())
            .bind(&token_hash)
            .fetch_optional(scoped.pool())
            .await?;

        Ok(row.map(|r| SessionRecord {
            id: r.0,
            user_id: r.1,
            org_id: r.2,
            token_hash: r.3,
            user_agent: r.4,
            ip_address: r.5,
            metadata: serde_json::Value::Object(Default::default()),
            created_at: r.6,
            expires_at: r.7,
            revoked_at: r.8,
        }))
    }

    /// Revoke a session by ID.
    pub async fn revoke(&self, scoped: &ScopedDb, session_id: &str) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE sessions SET revoked_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND id = $2",
        )
        .bind(scoped.instance_id())
        .bind(session_id)
        .execute(scoped.pool())
        .await?;
        Ok(())
    }

    /// List active sessions for a user.
    pub async fn list_by_user(
        &self,
        scoped: &ScopedDb,
        user_id: &str,
    ) -> anyhow::Result<Vec<SessionRecord>> {
        let created_at = scoped.as_text("created_at");
        let expires_at = scoped.as_text("expires_at");
        let revoked_at = scoped.as_text("revoked_at");
        let sql = format!(
            "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, {created_at}, {expires_at}, {revoked_at} \
             FROM sessions \
             WHERE instance_id = $1 AND user_id = $2 AND revoked_at IS NULL \
             AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP) \
             ORDER BY created_at DESC LIMIT 50"
        );
        let rows: Vec<(
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
        )> = sqlx::query_as(&sql)
            .bind(scoped.instance_id())
            .bind(user_id)
            .fetch_all(scoped.pool())
            .await?;

        Ok(rows
            .into_iter()
            .map(|r| SessionRecord {
                id: r.0,
                user_id: r.1,
                org_id: r.2,
                token_hash: r.3,
                user_agent: r.4,
                ip_address: r.5,
                metadata: serde_json::Value::Object(Default::default()),
                created_at: r.6,
                expires_at: r.7,
                revoked_at: r.8,
            })
            .collect())
    }
}

/// Hash a session token for storage (SHA-256, hex-encoded).
pub fn hash_token(token: &str) -> String {
    zitadel_crypto::token_hash(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_hash_deterministic() {
        let h1 = hash_token("test-token");
        let h2 = hash_token("test-token");
        assert_eq!(h1, h2);
        assert!(h1.starts_with("sha256:"));
    }

    #[test]
    fn different_tokens_different_hashes() {
        assert_ne!(hash_token("a"), hash_token("b"));
    }

    #[tokio::test]
    async fn session_create_and_find() {
        let db = zitadel_db::Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();

        // Create a user first (needed for FK).
        let scoped = db.scoped_default();
        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ('org1', 'default', 'Test')")
            .execute(scoped.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id, instance_id, org_id, identifier, user_type) VALUES ('u1', 'default', 'org1', 'test', 'human')")
            .execute(scoped.pool()).await.unwrap();

        let store = SessionStore::new(db.clone());
        let (session_id, token) = store
            .create(&scoped, "u1", "org1", "test-agent", "127.0.0.1")
            .await
            .unwrap();
        assert!(!session_id.is_empty());
        assert!(!token.is_empty());

        // Find by token.
        let found = store.find_by_token(&scoped, &token).await.unwrap();
        assert!(found.is_some());
        let session = found.unwrap();
        assert_eq!(session.user_id, "u1");

        // Revoke.
        store.revoke(&scoped, &session_id).await.unwrap();
        let found = store.find_by_token(&scoped, &token).await.unwrap();
        assert!(found.is_none()); // Revoked sessions not returned.
    }

    #[tokio::test]
    async fn session_lookup_is_scoped_by_instance() {
        let db = zitadel_db::Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();

        let default_scoped = db.scoped_default();
        let other_scoped = db.scoped("other".to_string());

        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, $3)")
            .bind("org-default")
            .bind(default_scoped.instance_id())
            .bind("Default Org")
            .execute(default_scoped.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id, instance_id, org_id, identifier, user_type) VALUES ($1, $2, $3, $4, $5)")
            .bind("user-default")
            .bind(default_scoped.instance_id())
            .bind("org-default")
            .bind("default-user")
            .bind("human")
            .execute(default_scoped.pool())
            .await
            .unwrap();

        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, $3)")
            .bind("org-other")
            .bind(other_scoped.instance_id())
            .bind("Other Org")
            .execute(other_scoped.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id, instance_id, org_id, identifier, user_type) VALUES ($1, $2, $3, $4, $5)")
            .bind("user-other")
            .bind(other_scoped.instance_id())
            .bind("org-other")
            .bind("other-user")
            .bind("human")
            .execute(other_scoped.pool())
            .await
            .unwrap();

        let store = SessionStore::new(db.clone());
        let (_session_id, token) = store
            .create(
                &other_scoped,
                "user-other",
                "org-other",
                "test-agent",
                "127.0.0.1",
            )
            .await
            .unwrap();

        assert!(
            store
                .find_by_token(&default_scoped, &token)
                .await
                .unwrap()
                .is_none()
        );

        let found = store.find_by_token(&other_scoped, &token).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().user_id, "user-other");

        let default_sessions = store
            .list_by_user(&default_scoped, "user-other")
            .await
            .unwrap();
        assert!(default_sessions.is_empty());
    }
}
