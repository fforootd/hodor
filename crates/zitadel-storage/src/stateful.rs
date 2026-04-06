use google_cloud_spanner::statement::Statement;
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
pub struct SpannerStatefulStore {
    db: Db,
}

impl SpannerStatefulStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl StatefulStore for SpannerStatefulStore {
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
            "SELECT id, COALESCE(org_id, '') FROM users WHERE instance_id = $1 AND identifier = $2 AND state = 'active'",
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
             WHERE t.instance_id = $1 AND t.token_hash = $2 AND t.type = 'pat' AND t.revoked_at IS NULL AND u.state = 'active'",
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
pub struct SpannerReadStore {
    db: Db,
}

impl SpannerReadStore {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl ReadStore for SpannerReadStore {
    async fn find_active_user_by_identifier(
        &self,
        instance_id: &str,
        identifier: &str,
    ) -> anyhow::Result<Option<UserIdentity>> {
        let client = self
            .db
            .spanner()
            .expect("spanner read store requires native spanner backend")
            .client();
        let mut stmt = Statement::new(
            "SELECT id, IFNULL(org_id, '') AS org_id FROM users \
             WHERE instance_id = @instance_id AND identifier = @identifier AND state = 'active' \
             LIMIT 1",
        );
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("identifier", &identifier);

        let mut tx = client.single().await?;
        let mut rows = tx.query(stmt).await?;
        let row = match rows.next().await? {
            Some(row) => row,
            None => return Ok(None),
        };

        Ok(Some(UserIdentity {
            user_id: row.column_by_name::<String>("id")?,
            org_id: row.column_by_name::<String>("org_id")?,
        }))
    }

    async fn load_password_hash(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<String>> {
        let client = self
            .db
            .spanner()
            .expect("spanner read store requires native spanner backend")
            .client();
        let mut stmt = Statement::new(
            "SELECT data FROM credentials \
             WHERE instance_id = @instance_id AND user_id = @user_id AND type = 'password' \
             LIMIT 1",
        );
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("user_id", &user_id);

        let mut tx = client.single().await?;
        let mut rows = tx.query(stmt).await?;
        let row = match rows.next().await? {
            Some(row) => row,
            None => return Ok(None),
        };
        let json = row.column_by_name::<String>("data")?;

        Ok(serde_json::from_str::<serde_json::Value>(&json)
            .ok()
            .and_then(|value| {
                value
                    .get("hash")
                    .and_then(|hash| hash.as_str())
                    .map(str::to_owned)
            }))
    }

    async fn resolve_pat_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<ResolvedPatIdentity>> {
        let client = self
            .db
            .spanner()
            .expect("spanner read store requires native spanner backend")
            .client();
        let hashed = token_hash(raw_token);
        let mut stmt = Statement::new(
            "SELECT t.user_id, t.type, t.session_id, u.org_id \
             FROM tokens t \
             JOIN users u ON u.id = t.user_id AND u.instance_id = t.instance_id \
             WHERE t.instance_id = @instance_id AND t.token_hash = @token_hash AND t.type = 'pat' AND t.revoked_at IS NULL AND u.state = 'active' \
             LIMIT 1",
        );
        stmt.add_param("instance_id", &instance_id);
        stmt.add_param("token_hash", &hashed);

        let mut tx = client.single().await?;
        let mut rows = tx.query(stmt).await?;
        let row = match rows.next().await? {
            Some(row) => row,
            None => return Ok(None),
        };

        Ok(Some(ResolvedPatIdentity {
            user_id: row.column_by_name::<String>("user_id")?,
            session_id: row
                .column_by_name::<Option<String>>("session_id")?
                .unwrap_or_default(),
            token_type: row.column_by_name::<String>("type")?,
            org_id: row.column_by_name::<String>("org_id")?,
        }))
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

#[derive(Clone)]
pub enum DefaultStatefulStorage {
    Sql(StatefulStorage<SqlStatefulStore, SqlReadStore>),
    Spanner(StatefulStorage<SpannerStatefulStore, SpannerReadStore>),
}

impl DefaultStatefulStorage {
    pub fn new_sql(stateful: SqlStatefulStore, read: SqlReadStore) -> Self {
        Self::Sql(StatefulStorage::new(stateful, read))
    }

    pub fn new_spanner(stateful: SpannerStatefulStore, read: SpannerReadStore) -> Self {
        Self::Spanner(StatefulStorage::new(stateful, read))
    }

    pub fn db(&self) -> &Db {
        match self {
            Self::Sql(storage) => storage.db(),
            Self::Spanner(storage) => storage.db(),
        }
    }

    pub async fn find_active_user_by_identifier(
        &self,
        instance_id: &str,
        identifier: &str,
    ) -> anyhow::Result<Option<UserIdentity>> {
        match self {
            Self::Sql(storage) => {
                storage
                    .find_active_user_by_identifier(instance_id, identifier)
                    .await
            }
            Self::Spanner(storage) => {
                storage
                    .find_active_user_by_identifier(instance_id, identifier)
                    .await
            }
        }
    }

    pub async fn load_password_hash(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> anyhow::Result<Option<String>> {
        match self {
            Self::Sql(storage) => storage.load_password_hash(instance_id, user_id).await,
            Self::Spanner(storage) => storage.load_password_hash(instance_id, user_id).await,
        }
    }

    pub async fn resolve_pat_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<ResolvedPatIdentity>> {
        match self {
            Self::Sql(storage) => storage.resolve_pat_token(instance_id, raw_token).await,
            Self::Spanner(storage) => storage.resolve_pat_token(instance_id, raw_token).await,
        }
    }
}

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

        let storage = DefaultStatefulStorage::new_sql(
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

    #[tokio::test]
    async fn ignores_non_pat_tokens_when_resolving_pat_identity() {
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

        let token_sql = format!(
            "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) VALUES ($1, $2, $3, $4, $5, $6, {})",
            scoped.json_bind(7),
        );
        sqlx::query(&token_sql)
            .bind("token-1")
            .bind(scoped.instance_id())
            .bind("access")
            .bind(token_hash("not-a-pat"))
            .bind("user-1")
            .bind("test")
            .bind("[]")
            .execute(scoped.pool())
            .await
            .unwrap();

        let storage = DefaultStatefulStorage::new_sql(
            SqlStatefulStore::new(db.clone()),
            SqlReadStore::new(db.clone()),
        );

        let resolved = storage
            .resolve_pat_token(zitadel_db::DEFAULT_INSTANCE_ID, "not-a-pat")
            .await
            .unwrap();
        assert!(resolved.is_none());
    }

    #[tokio::test]
    async fn ignores_pat_tokens_for_disabled_users() {
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
             VALUES ($1, $2, $3, $4, $5, 'human', 'disabled')",
        )
        .bind("user-1")
        .bind(scoped.instance_id())
        .bind("org-1")
        .bind("admin")
        .bind("Admin")
        .execute(scoped.pool())
        .await
        .unwrap();

        let pat_sql = format!(
            "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) VALUES ($1, $2, 'pat', $3, $4, $5, {})",
            scoped.json_bind(6),
        );
        sqlx::query(&pat_sql)
            .bind("pat-1")
            .bind(scoped.instance_id())
            .bind(token_hash("disabled-pat"))
            .bind("user-1")
            .bind("test")
            .bind("[]")
            .execute(scoped.pool())
            .await
            .unwrap();

        let storage = DefaultStatefulStorage::new_sql(
            SqlStatefulStore::new(db.clone()),
            SqlReadStore::new(db.clone()),
        );

        let resolved = storage
            .resolve_pat_token(zitadel_db::DEFAULT_INSTANCE_ID, "disabled-pat")
            .await
            .unwrap();
        assert!(resolved.is_none());
    }
}
