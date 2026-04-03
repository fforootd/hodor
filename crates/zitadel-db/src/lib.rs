pub mod bootstrap;
pub mod migrate;
pub mod provider;
pub mod scoped;
pub mod seed;

use sqlx::{AnyPool, any::AnyPoolOptions};
use std::fmt;

pub const DEFAULT_INSTANCE_ID: &str = "default";

/// Supported SQL dialects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Sqlite,
    Postgres,
}

impl Dialect {
    /// Whether this dialect uses SQLite-compatible SQL syntax.
    pub fn is_sqlite_compat(self) -> bool {
        matches!(self, Dialect::Sqlite)
    }
}

impl fmt::Display for Dialect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Dialect::Sqlite => write!(f, "sqlite"),
            Dialect::Postgres => write!(f, "postgres"),
        }
    }
}

/// DB wraps a sqlx AnyPool with dialect awareness.
#[derive(Clone)]
pub struct Db {
    pool: AnyPool,
    dialect: Dialect,
}

impl Db {
    /// Open a database connection based on the connection string.
    /// Empty string or "sqlite://..." opens SQLite; "postgres://..." opens Postgres.
    pub async fn open(conn_str: &str) -> anyhow::Result<Self> {
        let (dialect, url) = parse_connection_string(conn_str)?;

        sqlx::any::install_default_drivers();

        let is_memory = url == "sqlite::memory:";
        let max_conns = if is_memory {
            // In-memory SQLite: single connection (shared state).
            1
        } else if dialect == Dialect::Sqlite {
            16
        } else {
            25
        };

        let pool = AnyPoolOptions::new()
            .max_connections(max_conns)
            .connect(&url)
            .await?;

        if dialect == Dialect::Sqlite && !is_memory {
            sqlx::query("PRAGMA journal_mode = WAL")
                .execute(&pool)
                .await?;
        }
        if dialect == Dialect::Sqlite {
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&pool)
                .await?;
            sqlx::query("PRAGMA busy_timeout = 5000")
                .execute(&pool)
                .await?;
        }

        Ok(Self { pool, dialect })
    }

    /// Open with explicit pool settings from config.
    pub async fn open_with_config(
        conn_str: &str,
        config: &zitadel_config::StatefulStorageConfig,
    ) -> anyhow::Result<Self> {
        let (dialect, url) = parse_connection_string(conn_str)?;

        sqlx::any::install_default_drivers();

        let is_memory = url == "sqlite::memory:";
        let max_conns = if is_memory {
            1
        } else if dialect == Dialect::Sqlite {
            16
        } else {
            config.max_open_conns
        };

        let pool = AnyPoolOptions::new()
            .max_connections(max_conns)
            .connect(&url)
            .await?;

        if dialect == Dialect::Sqlite {
            sqlx::query("PRAGMA journal_mode = WAL")
                .execute(&pool)
                .await?;
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&pool)
                .await?;
            sqlx::query("PRAGMA busy_timeout = 5000")
                .execute(&pool)
                .await?;
        }

        Ok(Self { pool, dialect })
    }

    pub fn pool(&self) -> &AnyPool {
        &self.pool
    }
    pub fn dialect(&self) -> Dialect {
        self.dialect
    }

    /// ScopedDb bound to the default instance ID (for startup operations).
    pub fn scoped_default(&self) -> scoped::ScopedDb {
        scoped::ScopedDb::new(
            self.pool.clone(),
            self.dialect,
            DEFAULT_INSTANCE_ID.to_string(),
        )
    }

    /// ScopedDb bound to a specific instance ID.
    pub fn scoped(&self, instance_id: String) -> scoped::ScopedDb {
        scoped::ScopedDb::new(self.pool.clone(), self.dialect, instance_id)
    }

    pub async fn close(&self) {
        self.pool.close().await;
    }
}

/// Parse connection string into (dialect, sqlx-compatible URL).
fn parse_connection_string(conn_str: &str) -> anyhow::Result<(Dialect, String)> {
    if conn_str.is_empty() || conn_str.starts_with("sqlite://") {
        let path = conn_str.strip_prefix("sqlite://").unwrap_or("").to_string();

        // Ensure parent directory exists for file-based SQLite.
        if !path.is_empty()
            && path != ":memory:"
            && let Some(parent) = std::path::Path::new(&path).parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        let url = if path.is_empty() || path == ":memory:" {
            "sqlite::memory:".to_string()
        } else {
            // Normalize the path (resolve .. and symlinks where possible).
            let p = std::path::Path::new(&path);
            let normalized = if p.exists() {
                p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
            } else {
                // File doesn't exist yet — normalize parent, append filename.
                if let Some(parent) = p.parent() {
                    if parent.exists() {
                        let canon_parent = parent
                            .canonicalize()
                            .unwrap_or_else(|_| parent.to_path_buf());
                        canon_parent.join(p.file_name().unwrap_or_default())
                    } else {
                        p.to_path_buf()
                    }
                } else {
                    p.to_path_buf()
                }
            };
            format!("sqlite:{}?mode=rwc", normalized.display())
        };
        Ok((Dialect::Sqlite, url))
    } else if conn_str.starts_with("postgres://") || conn_str.starts_with("postgresql://") {
        Ok((Dialect::Postgres, conn_str.to_string()))
    } else {
        anyhow::bail!("unsupported database URL scheme: {}", conn_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sqlite_empty() {
        let (d, url) = parse_connection_string("").unwrap();
        assert_eq!(d, Dialect::Sqlite);
        assert_eq!(url, "sqlite::memory:");
    }

    #[test]
    fn parse_sqlite_path() {
        let (d, url) = parse_connection_string("sqlite://./data/test.db").unwrap();
        assert_eq!(d, Dialect::Sqlite);
        // Path gets normalized — just check prefix and suffix.
        assert!(url.starts_with("sqlite:"), "url={url}");
        assert!(url.contains("data/test.db"), "url={url}");
    }

    #[test]
    fn parse_postgres() {
        let (d, url) = parse_connection_string("postgres://user:pass@localhost/db").unwrap();
        assert_eq!(d, Dialect::Postgres);
        assert_eq!(url, "postgres://user:pass@localhost/db");
    }

    #[test]
    fn parse_unsupported() {
        assert!(parse_connection_string("mysql://localhost/db").is_err());
    }

    #[tokio::test]
    async fn open_in_memory_sqlite() {
        let db = Db::open("").await.unwrap();
        assert_eq!(db.dialect(), Dialect::Sqlite);
        sqlx::query("SELECT 1").execute(db.pool()).await.unwrap();
    }
}
