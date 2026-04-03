use zitadel_crypto::token_hash;
use zitadel_db::Db;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserIdentity {
    pub user_id: String,
    pub org_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPatIdentity {
    pub user_id: String,
    pub session_id: String,
    pub token_type: String,
    pub org_id: String,
}

pub trait StatefulStore: Clone + Send + Sync + 'static {
    fn db(&self) -> &Db;
}

pub trait ReadStore: Clone + Send + Sync + 'static {
    async fn find_active_user_by_identifier(
        &self,
        instance_id: &str,
        identifier: &str,
    ) -> anyhow::Result<Option<UserIdentity>>;

    async fn load_password_hash(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<String>>;

    async fn resolve_pat_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<ResolvedPatIdentity>>;
}

#[derive(Clone)]
pub struct SqlStatefulStore {
    db: Db,
}

impl SqlStatefulStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl StatefulStore for SqlStatefulStore {
    fn db(&self) -> &Db {
        &self.db
    }
}

#[derive(Clone)]
pub struct SqlReadStore {
    db: Db,
}

impl SqlReadStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl ReadStore for SqlReadStore {
    async fn find_active_user_by_identifier(
        &self,
        instance_id: &str,
        identifier: &str,
    ) -> anyhow::Result<Option<UserIdentity>> {
        let scoped = self.db.scoped(instance_id.to_string());
        let row: Option<(String, String)> = sqlx::query_as(
            "SELECT id, org_id FROM users WHERE instance_id = $1 AND identifier = $2 AND state = 'active'",
        )
        .bind(scoped.instance_id())
        .bind(identifier)
        .fetch_optional(scoped.pool())
        .await?;

        Ok(row.map(|(user_id, org_id)| UserIdentity { user_id, org_id }))
    }

    async fn load_password_hash(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let scoped = self.db.scoped(instance_id.to_string());
        let sql = format!(
            "SELECT COALESCE({}, '{{}}') FROM credentials WHERE instance_id = $1 AND user_id = $2 AND type = 'password'",
            scoped.as_text("data"),
        );
        let row: Option<(String,)> = sqlx::query_as(&sql)
            .bind(scoped.instance_id())
            .bind(user_id)
            .fetch_optional(scoped.pool())
            .await?;

        Ok(row.and_then(|(json,)| {
            serde_json::from_str::<serde_json::Value>(&json)
                .ok()
                .and_then(|value| {
                    value
                        .get("hash")
                        .and_then(|hash| hash.as_str())
                        .map(str::to_owned)
                })
        }))
    }

    async fn resolve_pat_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<ResolvedPatIdentity>> {
        let scoped = self.db.scoped(instance_id.to_string());
        let row: Option<(String, String, Option<String>, String)> = sqlx::query_as(
            "SELECT t.user_id, t.type, t.session_id, COALESCE(u.org_id, '') \
             FROM tokens t \
             JOIN users u ON u.id = t.user_id AND u.instance_id = t.instance_id \
             WHERE t.instance_id = $1 AND t.token_hash = $2 AND t.revoked_at IS NULL",
        )
        .bind(scoped.instance_id())
        .bind(token_hash(raw_token))
        .fetch_optional(scoped.pool())
        .await?;

        Ok(row.map(
            |(user_id, token_type, session_id, org_id)| ResolvedPatIdentity {
                user_id,
                session_id: session_id.unwrap_or_default(),
                token_type,
                org_id,
            },
        ))
    }
}

#[derive(Clone)]
pub struct StatefulStorage<S, R> {
    stateful: S,
    read: R,
}

impl<S, R> StatefulStorage<S, R> {
    pub fn new(stateful: S, read: R) -> Self {
        Self { stateful, read }
    }

    pub fn stateful(&self) -> &S {
        &self.stateful
    }

    pub fn read(&self) -> &R {
        &self.read
    }
}

impl<S, R> StatefulStorage<S, R>
where
    S: StatefulStore,
    R: ReadStore,
{
    pub fn db(&self) -> &Db {
        self.stateful.db()
    }

    pub async fn find_active_user_by_identifier(
        &self,
        instance_id: &str,
        identifier: &str,
    ) -> anyhow::Result<Option<UserIdentity>> {
        self.read
            .find_active_user_by_identifier(instance_id, identifier)
            .await
    }

    pub async fn load_password_hash(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<String>> {
        self.read.load_password_hash(instance_id, user_id).await
    }

    pub async fn resolve_pat_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<ResolvedPatIdentity>> {
        self.read.resolve_pat_token(instance_id, raw_token).await
    }
}

pub type DefaultStatefulStorage = StatefulStorage<SqlStatefulStore, SqlReadStore>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolves_password_and_pat_from_stateful_storage() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();

        sqlx::query("INSERT INTO orgs (id, instance_id, name) VALUES ($1, $2, $3)")
            .bind("org-1")
            .bind(scoped.instance_id())
            .bind("Default")
            .execute(scoped.pool())
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state) \
             VALUES ($1, $2, $3, $4, $5, 'human', 'active')",
        )
        .bind("user-1")
        .bind(scoped.instance_id())
        .bind("org-1")
        .bind("admin")
        .bind("Admin")
        .execute(scoped.pool())
        .await
        .unwrap();

        let password_hash = "$plain$secret";
        let cred_sql = format!(
            "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, $2, $3, 'password', {})",
            scoped.json_bind(4),
        );
        sqlx::query(&cred_sql)
            .bind("cred-1")
            .bind(scoped.instance_id())
            .bind("user-1")
            .bind(format!(r#"{{"hash":"{password_hash}"}}"#))
            .execute(scoped.pool())
            .await
            .unwrap();

        let pat_token = "zit_pat_test";
        let pat_sql = format!(
            "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) VALUES ($1, $2, 'pat', $3, $4, $5, {})",
            scoped.json_bind(6),
        );
        sqlx::query(&pat_sql)
            .bind("pat-1")
            .bind(scoped.instance_id())
            .bind(token_hash(pat_token))
            .bind("user-1")
            .bind("test")
            .bind("[]")
            .execute(scoped.pool())
            .await
            .unwrap();

        let storage = DefaultStatefulStorage::new(
            SqlStatefulStore::new(db.clone()),
            SqlReadStore::new(db.clone()),
        );

        let user = storage
            .find_active_user_by_identifier(zitadel_db::DEFAULT_INSTANCE_ID, "admin")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(user.user_id, "user-1");

        let hash = storage
            .load_password_hash(zitadel_db::DEFAULT_INSTANCE_ID, "user-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(hash, password_hash);

        let pat = storage
            .resolve_pat_token(zitadel_db::DEFAULT_INSTANCE_ID, pat_token)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(pat.user_id, "user-1");
        assert_eq!(pat.token_type, "pat");
    }
}
