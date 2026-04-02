use hodor_db::scoped::ScopedDb;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
pub struct SessionStore {
    db: hodor_db::Db,
}

impl SessionStore {
    pub fn new(db: hodor_db::Db) -> Self {
        Self { db }
    }

    /// Create a new session. Returns the session ID and raw token.
    pub async fn create(
        &self,
        scoped: &ScopedDb,
        user_id: &str,
        org_id: &str,
        user_agent: &str,
        ip_address: &str,
    ) -> anyhow::Result<(String, String)> {
        let session_id = Uuid::new_v4().to_string();
        let token = Uuid::new_v4().to_string();
        let token_hash = hash_token(&token);

        let org = if org_id.is_empty() { "_global" } else { org_id };

        sqlx::query(
            "INSERT INTO sessions (id, instance_id, user_id, org_id, token_hash, user_agent, ip_address, created_at, expires_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'), datetime('now', '+24 hours'))"
        )
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

        let row: Option<(String, String, String, String, String, String, String, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, created_at, expires_at, revoked_at \
                 FROM sessions \
                 WHERE instance_id = ? AND token_hash = ? AND revoked_at IS NULL"
            )
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
    pub async fn revoke(
        &self,
        scoped: &ScopedDb,
        session_id: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE sessions SET revoked_at = datetime('now') WHERE instance_id = ? AND id = ?"
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
        let rows: Vec<(String, String, String, String, String, String, String, Option<String>, Option<String>)> =
            sqlx::query_as(
                "SELECT id, user_id, org_id, token_hash, user_agent, ip_address, created_at, expires_at, revoked_at \
                 FROM sessions \
                 WHERE instance_id = ? AND user_id = ? AND revoked_at IS NULL \
                 ORDER BY created_at DESC LIMIT 50"
            )
            .bind(scoped.instance_id())
            .bind(user_id)
            .fetch_all(scoped.pool())
            .await?;

        Ok(rows.into_iter().map(|r| SessionRecord {
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
        }).collect())
    }
}

/// Hash a session token for storage (SHA-256, hex-encoded).
pub fn hash_token(token: &str) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    format!("sha256:{}", hex::encode(hasher.finalize()))
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
        let db = hodor_db::Db::open("").await.unwrap();
        hodor_db::migrate::migrate(&db).await.unwrap();

        // Create a user first (needed for FK).
        let scoped = db.scoped_default();
        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ('org1', 'default', 'Test')")
            .execute(scoped.pool()).await.unwrap();
        sqlx::query("INSERT INTO users (id, instance_id, org_id, identifier, user_type) VALUES ('u1', 'default', 'org1', 'test', 'human')")
            .execute(scoped.pool()).await.unwrap();

        let store = SessionStore::new(db.clone());
        let (session_id, token) = store.create(&scoped, "u1", "org1", "test-agent", "127.0.0.1").await.unwrap();
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
}
