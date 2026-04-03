use crate::{
    Db,
    provider::{
        ProviderCatalogRef, ProviderConnection, ProviderLinking, ProviderLinkingMode,
        ProviderMapping, ProviderPayload, ProviderTarget, ProviderUi, get_provider,
        insert_provider, update_provider,
    },
};
use serde::Deserialize;
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

impl SeedProvider {
    fn into_payload(&self) -> ProviderPayload {
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
        "SELECT id FROM orgs WHERE instance_id = 'default' LIMIT 1",
    )
    .fetch_optional(&*pool)
    .await?
    {
        Some(row) => row.0,
        None => {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO orgs (id, instance_id, name, state) VALUES ($1, 'default', 'Default', 'active')",
            )
            .bind(&id)
            .execute(&*pool)
            .await?;
            id
        }
    };

    for user in &seed.users {
        let display_name = if user.display_name.is_empty() {
            &user.identifier
        } else {
            &user.display_name
        };

        // Check if user exists.
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM users WHERE instance_id = 'default' AND identifier = $1",
        )
        .bind(&user.identifier)
        .fetch_optional(&*pool)
        .await?;

        let user_id = if let Some(row) = existing {
            if user.on_conflict == "update" {
                sqlx::query(
                    "UPDATE users SET display_name = $1, updated_at = CURRENT_TIMESTAMP WHERE id = $2",
                )
                .bind(display_name)
                .bind(&row.0)
                .execute(&*pool)
                .await?;
            }
            row.0
        } else {
            let id = Uuid::new_v4().to_string();
            sqlx::query(
                "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state) \
                 VALUES ($1, 'default', $2, $3, $4, 'human', 'active')",
            )
            .bind(&id)
            .bind(&org_id)
            .bind(&user.identifier)
            .bind(display_name)
            .execute(&*pool)
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
            .fetch_optional(&*pool)
            .await?;

            if let Some(cred) = existing_cred {
                let sql = format!(
                    "UPDATE credentials SET data = {} WHERE id = $1",
                    scoped.json_bind(2)
                );
                sqlx::query(&sql)
                    .bind(&cred.0)
                    .bind(&cred_json)
                    .execute(&*pool)
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
                    .execute(&*pool)
                    .await?;
            }
        }

        // Create PATs.
        for pat in &user.pats {
            // Hash the token with SHA-256 (same as zitadel_auth::session::hash_token).
            let token_hash = {
                use sha2::{Digest, Sha256};
                let mut hasher = Sha256::new();
                hasher.update(pat.token.as_bytes());
                format!("sha256:{}", hex::encode(hasher.finalize()))
            };
            let scopes = serde_json::to_string(&pat.scopes)?;

            let existing_pat: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM tokens WHERE instance_id = 'default' AND token_hash = $1",
            )
            .bind(&token_hash)
            .fetch_optional(&*pool)
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
                    .execute(&*pool)
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
                .fetch_optional(&*pool)
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
                    .execute(&*pool)
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
                .execute(&*pool)
                .await?;
        }

        tracing::debug!(client_id = app.client_id, "seeded app");
    }

    // Seed providers (external OIDC/SSO providers).
    for provider in &seed.providers {
        let payload = provider.into_payload();

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

    // Seed observability data (events + fingerprints) if tables are empty.
    let event_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM events WHERE instance_id = 'default'")
            .fetch_one(&*pool)
            .await?;
    if event_count.0 == 0 {
        seed_observability(&pool, &org_id, &seed).await?;
    }

    tracing::info!(
        users = seed.users.len(),
        apps = seed.apps.len(),
        providers = seed.providers.len(),
        "seed applied"
    );
    Ok(())
}

/// Seed realistic events and fingerprints so observability views have data.
async fn seed_observability(
    pool: &sqlx::AnyPool,
    org_id: &str,
    seed: &SeedFile,
) -> anyhow::Result<()> {
    // Collect seeded user IDs.
    let mut user_ids: Vec<(String, String)> = Vec::new();
    for user in &seed.users {
        if let Some(row) = sqlx::query_as::<_, (String,)>(
            "SELECT id FROM users WHERE instance_id = 'default' AND identifier = $1",
        )
        .bind(&user.identifier)
        .fetch_optional(pool)
        .await?
        {
            user_ids.push((row.0, user.identifier.clone()));
        }
    }
    if user_ids.is_empty() {
        return Ok(());
    }

    // Collect app client_ids.
    let app_clients: Vec<String> = seed.apps.iter().map(|a| a.client_id.clone()).collect();

    // Generate fingerprints.
    let fingerprints = [
        ("fp_browser_chrome_win", "browser", r#"{"ua":"Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/125","screen":"1920x1080","lang":"en-US"}"#),
        ("fp_browser_firefox_mac", "browser", r#"{"ua":"Mozilla/5.0 (Macintosh; Intel Mac OS X 14.5) Firefox/128","screen":"2560x1600","lang":"en-US"}"#),
        ("fp_mobile_ios", "mobile", r#"{"ua":"Zitadel-iOS/2.1","device":"iPhone15,2","os":"iOS 18.0"}"#),
    ];
    for (id, type_, raw) in &fingerprints {
        sqlx::query(
            "INSERT OR IGNORE INTO fingerprints (id, instance_id, type, raw_data) VALUES ($1, 'default', $2, $3)",
        )
        .bind(id)
        .bind(type_)
        .bind(raw)
        .execute(pool)
        .await?;
    }

    // Event templates: (event_type, category, actor_type, resource_type, delegation_type)
    let event_types = [
        ("user.login.succeeded", "auth", "human", "session", "direct"),
        ("user.login.failed", "auth", "human", "session", "direct"),
        ("token.issued", "auth", "human", "token", "direct"),
        ("session.created", "session", "human", "session", "direct"),
        ("session.refreshed", "session", "human", "session", "direct"),
        ("user.updated", "identity", "human", "user", "direct"),
        ("user.password.changed", "identity", "human", "user", "direct"),
        ("org.member.added", "identity", "human", "org", "direct"),
        ("app.token.exchanged", "auth", "service", "token", "exchanged"),
        ("user.login.succeeded", "auth", "human", "session", "pat_shared"),
    ];

    let ips = ["192.168.1.42", "10.0.0.15", "172.16.0.100", "203.0.113.50"];
    let sdks = [
        ("zitadel-js", "2.1.0"),
        ("zitadel-go", "1.4.0"),
        ("", ""),
    ];

    let mut seq = 1i64;
    // Generate events spread over the last 24 hours.
    for i in 0..60 {
        let minutes_ago = (60 - i) * 24; // spread over ~24h
        let (user_id, identifier) = &user_ids[i % user_ids.len()];
        let (event_type, category, actor_type, resource_type, delegation) =
            event_types[i % event_types.len()];
        let fp = fingerprints[i % fingerprints.len()].0;
        let ip = ips[i % ips.len()];
        let (sdk_name, sdk_version) = sdks[i % sdks.len()];
        let client_id = if !app_clients.is_empty() {
            &app_clients[i % app_clients.len()]
        } else {
            ""
        };

        let event_id = Uuid::new_v4().to_string();
        let request_id = Uuid::new_v4().to_string();
        let flow_id = if category == "auth" {
            Uuid::new_v4().to_string()
        } else {
            String::new()
        };

        sqlx::query(
            "INSERT INTO events (id, instance_id, event_type, category, org_id, actor_id, actor_type, \
             aggregate_id, aggregate_type, resource_type, \
             payload, metadata, request_id, session_id, flow_id, fingerprint, \
             client_id, delegation_type, sdk_name, sdk_version, sequence, \
             created_at) \
             VALUES ($1, 'default', $2, $3, $4, $5, $6, $7, $8, $9, \
             '{}', $10, $11, '', $12, $13, $14, $15, $16, $17, $18, \
             datetime('now', $19))",
        )
        .bind(&event_id)
        .bind(event_type)
        .bind(category)
        .bind(org_id)
        .bind(user_id)
        .bind(actor_type)
        .bind(user_id)     // aggregate_id
        .bind("user")      // aggregate_type
        .bind(resource_type)
        .bind(format!(r#"{{"ip":"{}","identifier":"{}"}}"#, ip, identifier))
        .bind(&request_id)
        .bind(&flow_id)
        .bind(fp)
        .bind(client_id)
        .bind(delegation)
        .bind(sdk_name)
        .bind(sdk_version)
        .bind(seq)
        .bind(format!("-{minutes_ago} minutes"))
        .execute(pool)
        .await?;

        seq += 1;
    }

    tracing::info!(events = 60, fingerprints = fingerprints.len(), "seeded observability data");
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
