use crate::Db;
use serde::Deserialize;
use std::path::Path;
use uuid::Uuid;

/// Seed file structure (matches Go's YAML format).
#[derive(Debug, Deserialize)]
pub struct SeedFile {
    #[serde(default)]
    pub users: Vec<SeedUser>,
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

/// Validate a seed file without touching the database.
pub fn validate(path: &Path) -> anyhow::Result<SeedFile> {
    let content = std::fs::read_to_string(path)?;
    let seed: SeedFile = serde_yaml::from_str(&content)?;
    tracing::info!(users = seed.users.len(), "seed file valid");
    Ok(seed)
}

/// Apply a seed file to the database.
pub async fn apply(db: &Db, path: &Path) -> anyhow::Result<()> {
    let seed = validate(path)?;
    let pool = db.pool();

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
                "INSERT INTO orgs (id, instance_id, name, state) VALUES (?, 'default', 'Default', 'active')",
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
            "SELECT id FROM users WHERE instance_id = 'default' AND identifier = ?",
        )
        .bind(&user.identifier)
        .fetch_optional(&*pool)
        .await?;

        let user_id = if let Some(row) = existing {
            if user.on_conflict == "update" {
                sqlx::query(
                    "UPDATE users SET display_name = ?, updated_at = datetime('now') WHERE id = ?",
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
                 VALUES (?, 'default', ?, ?, ?, 'human', 'active')",
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

            let existing_cred: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM credentials WHERE instance_id = 'default' AND user_id = ? AND type = 'password'",
            )
            .bind(&user_id)
            .fetch_optional(&*pool)
            .await?;

            if let Some(cred) = existing_cred {
                sqlx::query("UPDATE credentials SET data = ? WHERE id = ?")
                    .bind(&format!(r#"{{"hash":"{}"}}"#, hash))
                    .bind(&cred.0)
                    .execute(&*pool)
                    .await?;
            } else {
                let cred_id = Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES (?, 'default', ?, 'password', ?)",
                )
                .bind(&cred_id)
                .bind(&user_id)
                .bind(&format!(r#"{{"hash":"{}"}}"#, hash))
                .execute(&*pool)
                .await?;
            }
        }

        // Create PATs.
        for pat in &user.pats {
            // Hash the token with SHA-256 (same as hodor_auth::session::hash_token).
            let token_hash = {
                use sha2::{Sha256, Digest};
                let mut hasher = Sha256::new();
                hasher.update(pat.token.as_bytes());
                format!("sha256:{}", hex::encode(hasher.finalize()))
            };
            let scopes = serde_json::to_string(&pat.scopes)?;

            let existing_pat: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM tokens WHERE instance_id = 'default' AND token_hash = ?",
            )
            .bind(&token_hash)
            .fetch_optional(&*pool)
            .await?;

            if existing_pat.is_none() {
                let pat_id = Uuid::new_v4().to_string();
                sqlx::query(
                    "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) \
                     VALUES (?, 'default', 'pat', ?, ?, ?, ?)",
                )
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

    tracing::info!(users = seed.users.len(), "seed applied");
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
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE identifier = 'testuser'")
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
