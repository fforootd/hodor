use crate::{
    DEFAULT_ORG_ID, Db,
    provider::{
        ProviderCatalogRef, ProviderConnection, ProviderLinking, ProviderLinkingMode,
        ProviderMapping, ProviderPayload, ProviderTarget, ProviderUi, get_provider,
        insert_provider, update_provider,
    },
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::Path;
use uuid::Uuid;

/// Seed file structure (matches Go's YAML format).
#[derive(Debug, Deserialize)]
pub struct SeedFile {
    #[serde(default)]
    pub users: Vec<SeedUser>,
    #[serde(default)]
    pub apps: Vec<SeedApp>,
    #[serde(default)]
    pub providers: Vec<SeedProvider>,
    #[serde(default)]
    pub settings: Vec<SeedSetting>,
    #[serde(default)]
    pub login_flows: Vec<SeedLoginFlow>,
}

#[derive(Debug, Deserialize)]
pub struct SeedSetting {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct SeedLoginFlow {
    pub name: String,
    #[serde(default = "default_strategy")]
    pub strategy: String,
    #[serde(default)]
    pub is_default: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_active")]
    pub state: String,
    #[serde(default)]
    pub priority: i64,
    #[serde(default)]
    pub auth_methods: serde_json::Value,
    #[serde(default)]
    pub config: serde_json::Value,
}

fn default_strategy() -> String {
    "identifier_first".into()
}
fn default_true() -> bool {
    true
}
fn default_active() -> String {
    "active".into()
}

#[derive(Debug, Deserialize)]
pub struct SeedUser {
    pub identifier: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(rename = "type", default)]
    pub user_type: String,
    #[serde(default)]
    pub password: String,
    #[serde(default)]
    pub on_conflict: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub pats: Vec<SeedPat>,
    #[serde(default)]
    pub profile: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct SeedPat {
    #[serde(default)]
    pub name: String,
    pub token: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct SeedApp {
    pub name: String,
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default = "default_app_type")]
    pub app_type: String,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    #[serde(default)]
    pub post_logout_redirect_uris: Vec<String>,
    #[serde(default)]
    pub grant_types: Vec<String>,
    #[serde(default)]
    pub response_types: Vec<String>,
    #[serde(default)]
    pub on_conflict: String,
}

fn default_app_type() -> String {
    "web".into()
}

#[derive(Debug, Deserialize)]
pub struct SeedProvider {
    pub id: String,
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
    #[serde(default = "default_template")]
    pub template: String,
    #[serde(default)]
    pub auto_register: Option<bool>,
    #[serde(default)]
    pub config: serde_json::Value,
    #[serde(default)]
    pub connection: Option<ProviderConnection>,
    #[serde(default)]
    pub mapping: Option<ProviderMapping>,
    #[serde(default)]
    pub target: Option<ProviderTarget>,
    #[serde(default)]
    pub linking: Option<ProviderLinking>,
    #[serde(default)]
    pub session: Option<serde_json::Value>,
    #[serde(default)]
    pub ui: Option<ProviderUi>,
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub catalog_ref: Option<ProviderCatalogRef>,
    #[serde(default)]
    pub kind: Option<String>,
}

fn default_protocol() -> String {
    "oidc".into()
}

fn default_template() -> String {
    "custom".into()
}

fn token_hash(token: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(token.as_bytes()))
}

fn seed_user_metadata(user: &SeedUser) -> serde_json::Value {
    let mut metadata = serde_json::Map::new();
    if !user.capabilities.is_empty() {
        metadata.insert(
            "capabilities".into(),
            serde_json::Value::Array(
                user.capabilities
                    .iter()
                    .cloned()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
    if !user.profile.is_null() {
        metadata.insert("profile".into(), user.profile.clone());
    }
    serde_json::Value::Object(metadata)
}

impl SeedProvider {
    fn to_payload(&self) -> ProviderPayload {
        let display_name = if self.display_name.is_empty() {
            self.name.clone()
        } else {
            self.display_name.clone()
        };

        let connection = self.connection.clone().unwrap_or_else(|| {
            serde_json::from_value::<ProviderConnection>(self.config.clone()).unwrap_or_default()
        });

        let linking = self.linking.clone().unwrap_or_else(|| ProviderLinking {
            mode: if self.auto_register.unwrap_or(true) {
                ProviderLinkingMode::CreateOrLink
            } else {
                ProviderLinkingMode::LinkOnly
            },
            ..ProviderLinking::default()
        });

        ProviderPayload {
            display_name,
            kind: self
                .kind
                .clone()
                .unwrap_or_else(|| self.template.clone())
                .if_empty_then("custom"),
            protocol: self.protocol.clone(),
            connection,
            mapping: self.mapping.clone().unwrap_or_default(),
            target: self.target.clone().unwrap_or_else(|| ProviderTarget {
                schema_type: "human_user".into(),
                schema_id: String::new(),
            }),
            linking,
            session: self
                .session
                .clone()
                .unwrap_or_else(|| serde_json::Value::Object(Default::default())),
            ui: self.ui.clone().unwrap_or_default(),
            enabled: self.enabled.unwrap_or(true),
            catalog_ref: self.catalog_ref.clone().unwrap_or_default(),
        }
    }
}

trait EmptyStringFallback {
    fn if_empty_then(self, fallback: &str) -> String;
}

impl EmptyStringFallback for String {
    fn if_empty_then(self, fallback: &str) -> String {
        if self.is_empty() {
            fallback.to_string()
        } else {
            self
        }
    }
}

/// Validate a seed file without touching the database.
pub fn validate(path: &Path) -> anyhow::Result<SeedFile> {
    let content = std::fs::read_to_string(path)?;
    let seed: SeedFile = serde_yaml::from_str(&content)?;
    tracing::info!(
        users = seed.users.len(),
        apps = seed.apps.len(),
        providers = seed.providers.len(),
        "seed file valid"
    );
    Ok(seed)
}

/// Apply a seed file to the database.
pub async fn apply(db: &Db, path: &Path) -> anyhow::Result<()> {
    let seed = validate(path)?;
    let pool = db.pool();
    let scoped = db.scoped_default();

    // Get default org (first org, or create one).
    let org_id: String = match sqlx::query_as::<_, (String,)>(
        "SELECT id FROM orgs WHERE instance_id = 'default' AND id = $1 LIMIT 1",
    )
    .bind(DEFAULT_ORG_ID)
    .fetch_optional(pool)
    .await?
    {
        Some(row) => row.0,
        None => {
            sqlx::query(
                "INSERT INTO orgs (id, instance_id, name, state) VALUES ($1, 'default', 'Default', 'active')",
            )
            .bind(DEFAULT_ORG_ID)
            .execute(pool)
            .await?;
            DEFAULT_ORG_ID.to_string()
        }
    };

    for user in &seed.users {
        let display_name = if user.display_name.is_empty() {
            &user.identifier
        } else {
            &user.display_name
        };
        let metadata = seed_user_metadata(user);
        let metadata_json = serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".into());

        // Check if user exists.
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM users WHERE instance_id = 'default' AND identifier = $1",
        )
        .bind(&user.identifier)
        .fetch_optional(pool)
        .await?;

        let user_id = if let Some(row) = existing {
            if user.on_conflict == "update" {
                let sql = format!(
                    "UPDATE users SET display_name = $1, metadata = {}, updated_at = CURRENT_TIMESTAMP WHERE id = $3",
                    scoped.json_bind(2),
                );
                sqlx::query(&sql)
                    .bind(display_name)
                    .bind(&metadata_json)
                    .bind(&row.0)
                    .execute(pool)
                    .await?;
            }
            row.0
        } else {
            let sql = format!(
                "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, metadata) \
                 VALUES ($1, 'default', $2, $3, $4, 'human', 'active', {})",
                scoped.json_bind(5),
            );
            let id = Uuid::new_v4().to_string();
            sqlx::query(&sql)
                .bind(&id)
                .bind(&org_id)
                .bind(&user.identifier)
                .bind(display_name)
                .bind(&metadata_json)
                .execute(pool)
                .await?;
            id
        };

        // Set password if provided.
        if !user.password.is_empty() {
            // Store plaintext hash marker for now — Phase 2 adds argon2id.
            let hash = format!("$plain${}", user.password);
            let cred_json = format!(r#"{{"hash":"{}"}}"#, hash);

            let existing_cred: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM credentials WHERE instance_id = 'default' AND user_id = $1 AND type = 'password'",
            )
            .bind(&user_id)
            .fetch_optional(pool)
            .await?;

            if let Some(cred) = existing_cred {
                let sql = format!(
                    "UPDATE credentials SET data = {} WHERE id = $1",
                    scoped.json_bind(2)
                );
                sqlx::query(&sql)
                    .bind(&cred.0)
                    .bind(&cred_json)
                    .execute(pool)
                    .await?;
            } else {
                let cred_id = Uuid::new_v4().to_string();
                let sql = format!(
                    "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, 'default', $2, 'password', {})",
                    scoped.json_bind(3),
                );
                sqlx::query(&sql)
                    .bind(&cred_id)
                    .bind(&user_id)
                    .bind(&cred_json)
                    .execute(pool)
                    .await?;
            }
        }

        // Create PATs.
        for pat in &user.pats {
            let token_hash = token_hash(&pat.token);
            let scopes = serde_json::to_string(&pat.scopes)?;

            let existing_pat: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM tokens WHERE instance_id = 'default' AND token_hash = $1",
            )
            .bind(&token_hash)
            .fetch_optional(pool)
            .await?;

            if existing_pat.is_none() {
                let pat_id = Uuid::new_v4().to_string();
                let sql = format!(
                    "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) \
                     VALUES ($1, 'default', 'pat', $2, $3, $4, {})",
                    scoped.json_bind(5),
                );
                sqlx::query(&sql)
                    .bind(&pat_id)
                    .bind(&token_hash)
                    .bind(&user_id)
                    .bind(&pat.name)
                    .bind(&scopes)
                    .execute(pool)
                    .await?;
            }
        }

        tracing::debug!(identifier = user.identifier, user_id, "seeded user");
    }

    // Seed apps (OIDC clients).
    for app in &seed.apps {
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM apps WHERE instance_id = 'default' AND client_id = $1")
                .bind(&app.client_id)
                .fetch_optional(pool)
                .await?;

        let redirect_uris = serde_json::to_string(&app.redirect_uris)?;
        let grant_types = serde_json::to_string(&app.grant_types)?;
        let response_types = serde_json::to_string(&app.response_types)?;

        if let Some(row) = existing {
            if app.on_conflict == "update" {
                let sql = format!(
                    "UPDATE apps SET name = $1, client_secret = $2, redirect_uris = {}, grant_types = {}, response_types = {}, updated_at = CURRENT_TIMESTAMP WHERE id = $3",
                    scoped.json_bind(4),
                    scoped.json_bind(5),
                    scoped.json_bind(6),
                );
                sqlx::query(&sql)
                    .bind(&app.name)
                    .bind(&app.client_secret)
                    .bind(&row.0)
                    .bind(&redirect_uris)
                    .bind(&grant_types)
                    .bind(&response_types)
                    .execute(pool)
                    .await?;
            }
        } else {
            let id = Uuid::new_v4().to_string();
            let sql = format!(
                "INSERT INTO apps (id, instance_id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state) \
                 VALUES ($1, 'default', $2, $3, $4, $5, $6, {}, {}, {}, 'active')",
                scoped.json_bind(7),
                scoped.json_bind(8),
                scoped.json_bind(9),
            );
            sqlx::query(&sql)
                .bind(&id)
                .bind(&org_id)
                .bind(&app.name)
                .bind(&app.app_type)
                .bind(&app.client_id)
                .bind(&app.client_secret)
                .bind(&redirect_uris)
                .bind(&grant_types)
                .bind(&response_types)
                .execute(pool)
                .await?;
        }

        tracing::debug!(client_id = app.client_id, "seeded app");
    }

    // Seed providers (external OIDC/SSO providers).
    for provider in &seed.providers {
        let payload = provider.to_payload();

        if get_provider(&scoped, &provider.id).await?.is_some() {
            update_provider(&scoped, &provider.id, &payload).await?;
        } else {
            insert_provider(&scoped, &provider.id, &org_id, &payload).await?;
        }

        tracing::debug!(
            id = provider.id,
            name = payload.display_name,
            "seeded provider"
        );
    }

    // Seed login flows.
    for flow in &seed.login_flows {
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM login_flows WHERE instance_id = 'default' AND name = $1",
        )
        .bind(&flow.name)
        .fetch_optional(pool)
        .await?;

        let auth_methods =
            serde_json::to_string(&flow.auth_methods).unwrap_or_else(|_| "{}".into());
        let config = serde_json::to_string(&flow.config).unwrap_or_else(|_| "{}".into());

        if let Some(row) = existing {
            let sql = format!(
                "UPDATE login_flows SET strategy = $1, is_default = $2, enabled = $3, state = $4, priority = $5, \
                 auth_methods = {}, config = {}, updated_at = CURRENT_TIMESTAMP WHERE id = $8",
                scoped.json_bind(6),
                scoped.json_bind(7),
            );
            sqlx::query(&sql)
                .bind(&flow.strategy)
                .bind(flow.is_default)
                .bind(flow.enabled)
                .bind(&flow.state)
                .bind(flow.priority)
                .bind(&auth_methods)
                .bind(&config)
                .bind(&row.0)
                .execute(pool)
                .await?;
        } else {
            let id = Uuid::new_v4().to_string();
            let sql = format!(
                "INSERT INTO login_flows (id, instance_id, name, strategy, is_default, enabled, state, priority, auth_methods, config) \
                 VALUES ($1, 'default', $2, $3, $4, $5, $6, $7, {}, {})",
                scoped.json_bind(8),
                scoped.json_bind(9),
            );
            sqlx::query(&sql)
                .bind(&id)
                .bind(&flow.name)
                .bind(&flow.strategy)
                .bind(flow.is_default)
                .bind(flow.enabled)
                .bind(&flow.state)
                .bind(flow.priority)
                .bind(&auth_methods)
                .bind(&config)
                .execute(pool)
                .await?;
        }

        tracing::debug!(name = flow.name, "seeded login flow");
    }

    // Seed settings (bot_protection, branding, etc.).
    for setting in &seed.settings {
        let id = Uuid::new_v4().to_string();
        let data_str = serde_json::to_string(&setting.data).unwrap_or_else(|_| "{}".into());
        let json_bind = scoped.json_bind(4);
        let sql = format!(
            "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) \
             VALUES ($1, $2, $3, 'instance', '', {json_bind}) \
             ON CONFLICT(instance_id, type, scope, scope_id) DO UPDATE SET data = {json_bind}"
        );
        sqlx::query(&sql)
            .bind(&id)
            .bind(scoped.instance_id())
            .bind(&setting.type_)
            .bind(&data_str)
            .execute(pool)
            .await?;
        tracing::debug!(type_ = setting.type_, "seeded setting");
    }

    tracing::info!(
        users = seed.users.len(),
        apps = seed.apps.len(),
        providers = seed.providers.len(),
        login_flows = seed.login_flows.len(),
        "seed applied"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migrate;

    #[test]
    fn parse_seed_yaml() {
        let yaml = r#"
users:
  - identifier: admin
    display_name: Admin
    password: admin123
    capabilities: [admin, password]
    pats:
      - name: dev-token
        token: test-pat-token
        scopes: [admin]
"#;
        let seed: SeedFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(seed.users.len(), 1);
        assert_eq!(seed.users[0].identifier, "admin");
        assert_eq!(seed.users[0].pats.len(), 1);
    }

    #[tokio::test]
    async fn seed_apply_in_memory() {
        let db = Db::open("").await.unwrap();
        migrate::migrate(&db).await.unwrap();

        // Write a temp seed file.
        let dir = std::env::temp_dir().join("hodor-test-seed");
        std::fs::create_dir_all(&dir).unwrap();
        let seed_path = dir.join("test.yaml");
        std::fs::write(
            &seed_path,
            r#"
users:
  - identifier: testuser
    display_name: Test User
    password: pass123
    pats:
      - name: test-pat
        token: test-token-123
        scopes: [admin]
"#,
        )
        .unwrap();

        apply(&db, &seed_path).await.unwrap();

        // Verify user was created.
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE identifier = 'testuser'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(row.0, 1);

        // Verify PAT was created.
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tokens WHERE type = 'pat'")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(row.0, 1);

        // Cleanup.
        let _ = std::fs::remove_dir_all(&dir);
    }
}
