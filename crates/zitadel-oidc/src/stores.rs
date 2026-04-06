use crate::adapters::RuntimeKeyStore;
use crate::oidc::SigningKeys;
use crate::op::{KeyStore, NewStoredToken, StoredToken, TokenStore};
use google_cloud_spanner::{client::Error as SpannerError, statement::Statement};
use std::sync::Arc;
use zitadel_config::oidc::OidcConfig;
use zitadel_crypto::{SecretBox, token_hash};
use zitadel_db::{Db, Dialect};

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

#[derive(Clone, Default)]
pub struct PersistentTokenStore {
    db: Option<Db>,
}

impl PersistentTokenStore {
    pub fn new(db: Db) -> Self {
        Self { db: Some(db) }
    }

    pub fn disabled() -> Self {
        Self { db: None }
    }
}

impl TokenStore for PersistentTokenStore {
    fn enforces_storage(&self) -> bool {
        self.db.is_some()
    }

    async fn store_token(&self, instance_id: &str, token: &NewStoredToken) -> anyhow::Result<()> {
        let Some(db) = self.db.as_ref() else {
            return Ok(());
        };
        let scopes_json = scopes_to_json(&token.scope)?;
        match db {
            Db::Sql(_) => {
                let scoped = db.scoped(instance_id.to_string());
                let expires_at_expr = sql_expiry_expr(scoped.dialect(), 12);
                let sql = format!(
                    "INSERT INTO tokens \
                     (id, instance_id, type, token_hash, user_id, session_id, name, scopes, audience, application_id, auth_method, refresh_token_id, expires_at, auth_time) \
                     VALUES \
                     ($1, $2, $3, $4, $5, $6, '', {}, $8, $9, $10, $11, {expires_at_expr}, {})",
                    scoped.json_bind(7),
                    scoped.timestamp_now(),
                );
                sqlx::query(&sql)
                    .bind(&token.token_id)
                    .bind(instance_id)
                    .bind(&token.token_type)
                    .bind(token_hash(&token.raw_token))
                    .bind(token.user_id.as_deref())
                    .bind(token.session_id.as_deref())
                    .bind(&scopes_json)
                    .bind(&token.client_id)
                    .bind(&token.application_id)
                    .bind(&token.auth_method)
                    .bind(token.refresh_family_id.as_deref().unwrap_or_default())
                    .bind(token.expires_in_secs as i64)
                    .execute(scoped.pool())
                    .await?;
            }
            Db::Spanner(spanner) => {
                let mut stmt = Statement::new(
                    "INSERT INTO tokens \
                     (id, instance_id, type, token_hash, user_id, session_id, name, scopes, audience, application_id, auth_method, refresh_token_id, expires_at, auth_time) \
                     VALUES \
                     (@id, @instance_id, @type, @token_hash, @user_id, @session_id, '', @scopes, @audience, @application_id, @auth_method, @refresh_token_id, \
                      TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL @expires_in_secs SECOND), CURRENT_TIMESTAMP())",
                );
                stmt.add_param("id", &token.token_id);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("type", &token.token_type);
                stmt.add_param("token_hash", &token_hash(&token.raw_token));
                stmt.add_param("user_id", &token.user_id);
                stmt.add_param("session_id", &token.session_id);
                stmt.add_param("scopes", &scopes_json);
                stmt.add_param("audience", &token.client_id);
                stmt.add_param("application_id", &token.application_id);
                stmt.add_param("auth_method", &token.auth_method);
                stmt.add_param(
                    "refresh_token_id",
                    &token.refresh_family_id.as_deref().unwrap_or_default(),
                );
                stmt.add_param("expires_in_secs", &(token.expires_in_secs as i64));
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = stmt.clone();
                        Box::pin(async move {
                            tx.update(stmt).await?;
                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await?;
            }
        }
        Ok(())
    }

    async fn lookup_active_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> anyhow::Result<Option<StoredToken>> {
        let Some(db) = self.db.as_ref() else {
            return Ok(None);
        };
        let hashed = token_hash(raw_token);
        match db {
            Db::Sql(_) => {
                let scoped = db.scoped(instance_id.to_string());
                let scopes = scoped.as_text("scopes");
                let sql = format!(
                    "SELECT id, type, user_id, session_id, COALESCE(audience, ''), COALESCE(application_id, ''), COALESCE({scopes}, '[]'), COALESCE(refresh_token_id, '') \
                     FROM tokens \
                     WHERE instance_id = $1 AND token_hash = $2 AND revoked_at IS NULL \
                       AND (expires_at IS NULL OR expires_at > {}) \
                     LIMIT 1",
                    scoped.timestamp_now(),
                );
                let row: Option<(
                    String,
                    String,
                    Option<String>,
                    Option<String>,
                    String,
                    String,
                    String,
                    String,
                )> = sqlx::query_as(&sql)
                    .bind(instance_id)
                    .bind(&hashed)
                    .fetch_optional(scoped.pool())
                    .await?;
                Ok(row.map(|row| StoredToken {
                    token_id: row.0,
                    token_type: row.1,
                    user_id: row.2,
                    session_id: row.3,
                    client_id: row.4,
                    application_id: row.5,
                    scope: scope_from_json(&row.6),
                    refresh_family_id: non_empty(row.7),
                }))
            }
            Db::Spanner(spanner) => {
                let mut stmt = Statement::new(
                    "SELECT id, type, user_id, session_id, IFNULL(audience, '') AS audience, \
                            IFNULL(application_id, '') AS application_id, IFNULL(scopes, '[]') AS scopes, \
                            IFNULL(refresh_token_id, '') AS refresh_token_id \
                     FROM tokens \
                     WHERE instance_id = @instance_id AND token_hash = @token_hash AND revoked_at IS NULL \
                       AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP()) \
                     LIMIT 1",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("token_hash", &hashed);
                let row = spanner_query_optional(spanner, stmt).await?;
                Ok(row.map(|row| StoredToken {
                    token_id: row.column_by_name::<String>("id").unwrap_or_default(),
                    token_type: row.column_by_name::<String>("type").unwrap_or_default(),
                    user_id: row
                        .column_by_name::<Option<String>>("user_id")
                        .unwrap_or(None)
                        .filter(|value| !value.is_empty()),
                    session_id: row
                        .column_by_name::<Option<String>>("session_id")
                        .unwrap_or(None)
                        .filter(|value| !value.is_empty()),
                    client_id: row.column_by_name::<String>("audience").unwrap_or_default(),
                    application_id: row
                        .column_by_name::<String>("application_id")
                        .unwrap_or_default(),
                    scope: scope_from_json(
                        &row.column_by_name::<String>("scopes").unwrap_or_default(),
                    ),
                    refresh_family_id: row
                        .column_by_name::<String>("refresh_token_id")
                        .ok()
                        .and_then(non_empty),
                }))
            }
        }
    }

    async fn revoke_token_by_id(&self, instance_id: &str, token_id: &str) -> anyhow::Result<()> {
        let Some(db) = self.db.as_ref() else {
            return Ok(());
        };
        match db {
            Db::Sql(_) => {
                let scoped = db.scoped(instance_id.to_string());
                sqlx::query(
                    "UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP \
                     WHERE instance_id = $1 AND id = $2 AND revoked_at IS NULL",
                )
                .bind(instance_id)
                .bind(token_id)
                .execute(scoped.pool())
                .await?;
            }
            Db::Spanner(spanner) => {
                let mut stmt = Statement::new(
                    "UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP() \
                     WHERE instance_id = @instance_id AND id = @id AND revoked_at IS NULL",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("id", &token_id);
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = stmt.clone();
                        Box::pin(async move {
                            tx.update(stmt).await?;
                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await?;
            }
        }
        Ok(())
    }

    async fn revoke_refresh_family(
        &self,
        instance_id: &str,
        refresh_family_id: &str,
    ) -> anyhow::Result<()> {
        let Some(db) = self.db.as_ref() else {
            return Ok(());
        };
        match db {
            Db::Sql(_) => {
                let scoped = db.scoped(instance_id.to_string());
                sqlx::query(
                    "UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP \
                     WHERE instance_id = $1 AND revoked_at IS NULL AND (refresh_token_id = $2 OR id = $2)",
                )
                .bind(instance_id)
                .bind(refresh_family_id)
                .execute(scoped.pool())
                .await?;
            }
            Db::Spanner(spanner) => {
                let mut stmt = Statement::new(
                    "UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP() \
                     WHERE instance_id = @instance_id AND revoked_at IS NULL \
                       AND (refresh_token_id = @refresh_token_id OR id = @refresh_token_id)",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("refresh_token_id", &refresh_family_id);
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = stmt.clone();
                        Box::pin(async move {
                            tx.update(stmt).await?;
                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await?;
            }
        }
        Ok(())
    }

    async fn revoke_session_tokens(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> anyhow::Result<()> {
        let Some(db) = self.db.as_ref() else {
            return Ok(());
        };
        match db {
            Db::Sql(_) => {
                let scoped = db.scoped(instance_id.to_string());
                sqlx::query(
                    "UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP \
                     WHERE instance_id = $1 AND session_id = $2 AND revoked_at IS NULL \
                       AND type IN ('oidc_access', 'oidc_refresh')",
                )
                .bind(instance_id)
                .bind(session_id)
                .execute(scoped.pool())
                .await?;
            }
            Db::Spanner(spanner) => {
                let mut stmt = Statement::new(
                    "UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP() \
                     WHERE instance_id = @instance_id AND session_id = @session_id AND revoked_at IS NULL \
                       AND type IN ('oidc_access', 'oidc_refresh')",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("session_id", &session_id);
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = stmt.clone();
                        Box::pin(async move {
                            tx.update(stmt).await?;
                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await?;
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct PersistentKeyStore {
    db: Option<Db>,
    secret_box: Option<Arc<SecretBox>>,
    oidc_config: OidcConfig,
    runtime: RuntimeKeyStore,
}

impl PersistentKeyStore {
    pub fn new(db: Db, secret_box: Arc<SecretBox>, oidc_config: OidcConfig) -> Self {
        Self {
            db: Some(db),
            secret_box: Some(secret_box),
            oidc_config,
            runtime: RuntimeKeyStore::new(),
        }
    }

    pub fn ephemeral(oidc_config: OidcConfig) -> Self {
        Self {
            db: None,
            secret_box: None,
            oidc_config,
            runtime: RuntimeKeyStore::new(),
        }
    }

    async fn list_active_records(
        &self,
        instance_id: &str,
    ) -> anyhow::Result<Vec<StoredSigningKeyRecord>> {
        let Some(db) = self.db.as_ref() else {
            return Ok(Vec::new());
        };
        match db {
            Db::Sql(_) => {
                let scoped = db.scoped(instance_id.to_string());
                let created_at_epoch = scoped.epoch_seconds("created_at");
                let sql = format!(
                    "SELECT id, COALESCE(algorithm, 'RS256'), COALESCE(encryption_key_id, ''), ciphertext, nonce, public_key, {created_at_epoch} \
                     FROM secrets \
                     WHERE instance_id = $1 AND secret_type = 'oidc_signing_key' \
                       AND (expires_at IS NULL OR expires_at > {}) \
                     ORDER BY created_at DESC",
                    scoped.timestamp_now(),
                );
                let rows: Vec<(
                    String,
                    String,
                    String,
                    Vec<u8>,
                    Option<Vec<u8>>,
                    Option<Vec<u8>>,
                    i64,
                )> = sqlx::query_as(&sql)
                    .bind(instance_id)
                    .fetch_all(scoped.pool())
                    .await?;
                Ok(rows
                    .into_iter()
                    .map(|row| StoredSigningKeyRecord {
                        kid: row.0,
                        algorithm: row.1,
                        encryption_key_id: row.2,
                        ciphertext: row.3,
                        nonce: row.4.unwrap_or_default(),
                        public_key: row.5.unwrap_or_default(),
                        created_at_epoch: row.6.max(0) as u64,
                    })
                    .collect())
            }
            Db::Spanner(spanner) => {
                let mut stmt = Statement::new(
                    "SELECT id, IFNULL(algorithm, 'RS256') AS algorithm, IFNULL(encryption_key_id, '') AS encryption_key_id, \
                            ciphertext, nonce, public_key, UNIX_SECONDS(created_at) AS created_at_epoch \
                     FROM secrets \
                     WHERE instance_id = @instance_id AND secret_type = 'oidc_signing_key' \
                       AND (expires_at IS NULL OR expires_at > CURRENT_TIMESTAMP()) \
                     ORDER BY created_at DESC",
                );
                stmt.add_param("instance_id", &instance_id);
                Ok(spanner_query_all(spanner, stmt)
                    .await?
                    .into_iter()
                    .map(|row| StoredSigningKeyRecord {
                        kid: row.column_by_name::<String>("id").unwrap_or_default(),
                        algorithm: row
                            .column_by_name::<String>("algorithm")
                            .unwrap_or_default(),
                        encryption_key_id: row
                            .column_by_name::<String>("encryption_key_id")
                            .unwrap_or_default(),
                        ciphertext: row
                            .column_by_name::<Vec<u8>>("ciphertext")
                            .unwrap_or_default(),
                        nonce: row
                            .column_by_name::<Option<Vec<u8>>>("nonce")
                            .unwrap_or(None)
                            .unwrap_or_default(),
                        public_key: row
                            .column_by_name::<Option<Vec<u8>>>("public_key")
                            .unwrap_or(None)
                            .unwrap_or_default(),
                        created_at_epoch: row
                            .column_by_name::<i64>("created_at_epoch")
                            .unwrap_or_default()
                            .max(0) as u64,
                    })
                    .collect())
            }
        }
    }

    async fn create_signing_key(&self, instance_id: &str) -> anyhow::Result<Arc<SigningKeys>> {
        let Some(db) = self.db.as_ref() else {
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

        match db {
            Db::Sql(_) => {
                let scoped = db.scoped(instance_id.to_string());
                let expires_at_expr = sql_expiry_expr(scoped.dialect(), 8);
                let sql = format!(
                    "INSERT INTO secrets \
                     (id, instance_id, secret_type, algorithm, encryption_key_id, ciphertext, nonce, public_key, expires_at) \
                     VALUES \
                     ($1, $2, 'oidc_signing_key', $3, $4, $5, $6, $7, {expires_at_expr})"
                );
                sqlx::query(&sql)
                    .bind(&key.kid)
                    .bind(instance_id)
                    .bind(&key.alg)
                    .bind(&sealed.key_id)
                    .bind(&sealed.ciphertext)
                    .bind(&sealed.nonce)
                    .bind(&key.public_pem)
                    .bind(overlap_secs as i64)
                    .execute(scoped.pool())
                    .await?;
            }
            Db::Spanner(spanner) => {
                let mut stmt = Statement::new(
                    "INSERT INTO secrets \
                     (id, instance_id, secret_type, algorithm, encryption_key_id, ciphertext, nonce, public_key, expires_at) \
                     VALUES \
                     (@id, @instance_id, 'oidc_signing_key', @algorithm, @encryption_key_id, @ciphertext, @nonce, @public_key, \
                      TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL @expires_in_secs SECOND))",
                );
                stmt.add_param("id", &key.kid);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("algorithm", &key.alg);
                stmt.add_param("encryption_key_id", &sealed.key_id);
                stmt.add_param("ciphertext", &sealed.ciphertext);
                stmt.add_param("nonce", &sealed.nonce);
                stmt.add_param("public_key", &key.public_pem);
                stmt.add_param("expires_in_secs", &(overlap_secs as i64));
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = stmt.clone();
                        Box::pin(async move {
                            tx.update(stmt).await?;
                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await?;
            }
        }

        Ok(key.shared())
    }

    fn key_rotation_window_secs(&self) -> u64 {
        self.oidc_config.private_key_lifetime_secs.max(1)
    }

    fn can_use_persistent_store(&self) -> bool {
        self.db.is_some() && self.secret_box.is_some()
    }

    fn load_key(&self, record: &StoredSigningKeyRecord) -> anyhow::Result<Arc<SigningKeys>> {
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

#[derive(Debug, Clone)]
struct StoredSigningKeyRecord {
    kid: String,
    algorithm: String,
    encryption_key_id: String,
    ciphertext: Vec<u8>,
    nonce: Vec<u8>,
    public_key: Vec<u8>,
    created_at_epoch: u64,
}

fn sql_expiry_expr(dialect: Dialect, param_n: usize) -> String {
    match dialect {
        Dialect::Postgres => format!("CURRENT_TIMESTAMP + (${param_n} * INTERVAL '1 second')"),
        Dialect::Sqlite => {
            format!("datetime(CURRENT_TIMESTAMP, '+' || ${param_n} || ' seconds')")
        }
        Dialect::Spanner => unreachable!("spanner does not use sqlx expiry expressions"),
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

fn scope_from_json(scopes_json: &str) -> String {
    serde_json::from_str::<Vec<String>>(scopes_json)
        .map(|scopes| scopes.join(" "))
        .unwrap_or_else(|_| scopes_json.to_string())
}

fn non_empty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}

async fn spanner_query_optional(
    spanner: &zitadel_db::SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Option<google_cloud_spanner::row::Row>> {
    let client = spanner.client();
    let mut tx = client.single().await?;
    let mut rows = tx.query(stmt).await?;
    Ok(rows.next().await?)
}

async fn spanner_query_all(
    spanner: &zitadel_db::SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Vec<google_cloud_spanner::row::Row>> {
    let client = spanner.client();
    let mut tx = client.single().await?;
    let mut rows = tx.query(stmt).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(row);
    }
    Ok(out)
}
