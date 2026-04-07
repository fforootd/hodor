use crate::{BackendKind, Db, schema};
use sqlx::{Connection, Executor};
use zitadel_config::StatefulStorageConfig;

const POSTGRES_MIGRATION_LOCK_ID: i64 = 6_900_181_427_071;

/// Verify a Spanner database exists and reconcile its schema to the embedded
/// baseline before opening a normal data client.
pub async fn prepare_spanner(config: &StatefulStorageConfig) -> anyhow::Result<()> {
    let statements = schema::embedded_migrations(BackendKind::Spanner)
        .iter()
        .flat_map(|(_, sql)| split_statements(&extract_goose_up(sql)))
        .collect::<Vec<_>>();
    crate::spanner::ensure_database_from_config(config, &statements).await
}

/// Run all pending migrations.
pub async fn migrate(db: &Db) -> anyhow::Result<()> {
    let backend = db.backend();
    let dialect = db.dialect();

    let migrations = schema::embedded_migrations(backend);

    if backend == BackendKind::Spanner {
        let statements = schema::embedded_migrations(BackendKind::Spanner)
            .iter()
            .flat_map(|(_, sql)| split_statements(&extract_goose_up(sql)))
            .collect::<Vec<_>>();
        let spanner = db
            .spanner()
            .expect("spanner backend should expose native spanner client");
        spanner.ensure_database(&statements).await?;
        tracing::info!(
            backend = %backend,
            dialect = %dialect,
            statements = statements.len(),
            "spanner schema ready"
        );
        return Ok(());
    }

    let pool = db.pool();

    // Use a single dedicated connection for all migration work to avoid
    // deadlocking the pool (in-memory SQLite has only 1 connection).
    let mut conn = pool.acquire().await?;

    if backend == BackendKind::Postgres {
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(POSTGRES_MIGRATION_LOCK_ID)
            .execute(&mut *conn)
            .await?;
    }

    // Create version tracking table.
    conn.execute(
        "CREATE TABLE IF NOT EXISTS _schema_version (version INTEGER NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)"
    ).await?;

    // Get current version.
    let current: (i64,) = sqlx::query_as("SELECT COALESCE(MAX(version), 0) FROM _schema_version")
        .fetch_one(&mut *conn)
        .await?;
    let current = current.0;
    let target = migrations.len() as i64;
    if current > target {
        anyhow::bail!(
            "schema version {current} is ahead of squashed baseline {target} — reset the dev database and rerun migrations"
        );
    }
    let mut applied = 0u32;

    for (i, (name, sql)) in migrations.iter().enumerate() {
        let version = (i + 1) as i64;
        if version <= current {
            continue;
        }

        let up_sql = extract_goose_up(sql);
        tracing::debug!(version, name, "applying migration");

        if backend != BackendKind::Sqlite {
            let mut tx = conn.begin().await?;
            for (si, stmt) in split_statements(&up_sql).iter().enumerate() {
                let stmt = stmt.trim();
                if stmt.is_empty() {
                    continue;
                }
                tx.execute(sqlx::query(stmt)).await.map_err(|e| {
                    anyhow::anyhow!(
                        "migration {name} failed at stmt {si}: {e}\nStatement: {}",
                        &stmt[..stmt.len().min(200)]
                    )
                })?;
            }
            sqlx::query("INSERT INTO _schema_version (version) VALUES ($1)")
                .bind(version)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
        } else {
            conn.execute("PRAGMA foreign_keys = OFF").await?;
            for (si, stmt) in split_statements(&up_sql).iter().enumerate() {
                let stmt = stmt.trim();
                if stmt.is_empty() {
                    continue;
                }
                conn.execute(sqlx::query(stmt)).await.map_err(|e| {
                    anyhow::anyhow!(
                        "migration {name} failed at stmt {si}: {e}\nStatement: {}",
                        &stmt[..stmt.len().min(200)]
                    )
                })?;
            }
            conn.execute("PRAGMA foreign_keys = ON").await?;
            sqlx::query("INSERT INTO _schema_version (version) VALUES ($1)")
                .bind(version)
                .execute(&mut *conn)
                .await?;
        }
        applied += 1;
    }

    if backend == BackendKind::Postgres {
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(POSTGRES_MIGRATION_LOCK_ID)
            .execute(&mut *conn)
            .await;
    }

    drop(conn);

    tracing::info!(
        backend = %backend,
        dialect = %dialect,
        version = target,
        applied,
        "schema ready"
    );
    Ok(())
}

/// Check the current schema version without running DDL.
pub async fn check_version(db: &Db) -> anyhow::Result<()> {
    if db.backend() == BackendKind::Spanner {
        let spanner = db
            .spanner()
            .expect("spanner backend should expose native spanner client");
        let statements = spanner.current_ddl().await?;
        if statements.is_empty() {
            anyhow::bail!("spanner baseline has not been applied — run 'zitadel migrate' first");
        }
        return Ok(());
    }

    let pool = db.pool();
    let target = match db.backend() {
        BackendKind::Sqlite => schema::embedded_migrations(BackendKind::Sqlite).len() as i64,
        BackendKind::Postgres => schema::embedded_migrations(BackendKind::Postgres).len() as i64,
        BackendKind::Spanner => unreachable!("spanner is handled above"),
    };

    // Check if version table exists.
    let current: i64 =
        match sqlx::query_as::<_, (i64,)>("SELECT COALESCE(MAX(version), 0) FROM _schema_version")
            .fetch_one(pool)
            .await
        {
            Ok(row) => row.0,
            Err(_) => 0, // Table doesn't exist yet.
        };

    if current < target {
        anyhow::bail!(
            "schema version {current} is behind target {target} — run 'zitadel migrate' first"
        );
    }
    if current > target {
        anyhow::bail!(
            "schema version {current} is ahead of squashed baseline {target} — reset the dev database and rerun migrations"
        );
    }
    Ok(())
}

/// Extract the Up portion from a goose-formatted SQL file.
fn extract_goose_up(sql: &str) -> String {
    let mut in_up = false;
    let mut lines = Vec::new();

    for line in sql.lines() {
        let trimmed = line.trim();
        if trimmed == "-- +goose Up" {
            in_up = true;
            continue;
        }
        if trimmed == "-- +goose Down" {
            break;
        }
        if in_up {
            lines.push(line);
        }
    }

    lines.join("\n")
}

/// Split SQL into individual statements on `;`.
/// Respects string literals (won't split on `;` inside quotes).
/// Strips leading comment-only lines from each statement.
fn split_statements(sql: &str) -> Vec<String> {
    let mut stmts = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut chars = sql.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\'' && !in_string {
            in_string = true;
            current.push(ch);
        } else if ch == '\'' && in_string {
            if chars.peek() == Some(&'\'') {
                current.push(ch);
                current.push(chars.next().unwrap());
            } else {
                in_string = false;
                current.push(ch);
            }
        } else if ch == ';' && !in_string {
            if let Some(stmt) = strip_comments_and_validate(&current) {
                stmts.push(stmt);
            }
            current.clear();
        } else {
            current.push(ch);
        }
    }

    if let Some(stmt) = strip_comments_and_validate(&current) {
        stmts.push(stmt);
    }

    stmts
}

/// Strip leading comment lines and blank lines from a SQL chunk.
/// Returns None if the result is empty (pure comments).
fn strip_comments_and_validate(raw: &str) -> Option<String> {
    // Remove leading lines that are comments or blank.
    let mut lines: Vec<&str> = Vec::new();
    let mut found_sql = false;
    for line in raw.lines() {
        let trimmed = line.trim();
        if !found_sql && (trimmed.is_empty() || trimmed.starts_with("--")) {
            continue;
        }
        found_sql = true;
        lines.push(line);
    }
    let result = lines.join("\n").trim().to_string();
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Db;

    #[test]
    fn extract_goose_up_section() {
        let sql = "-- +goose Up\nCREATE TABLE foo (id TEXT);\n-- +goose Down\nDROP TABLE foo;";
        assert_eq!(extract_goose_up(sql), "CREATE TABLE foo (id TEXT);");
    }

    #[test]
    fn split_simple_statements() {
        let sql = "CREATE TABLE a (id TEXT);\nCREATE TABLE b (id TEXT);";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2);
    }

    #[test]
    fn split_respects_string_literals() {
        let sql = "INSERT INTO t VALUES ('a;b');\nSELECT 1;";
        let stmts = split_statements(sql);
        assert_eq!(stmts.len(), 2);
        assert!(stmts[0].contains("'a;b'"));
    }

    #[test]
    fn test_split_actual_migration() {
        let sql = include_str!("../../../migrations/sqlite/00001_baseline.sql");
        let up = extract_goose_up(sql);
        let stmts = split_statements(&up);
        println!("Total statements: {}", stmts.len());
        // Find the schemas-related statements
        for (i, s) in stmts.iter().enumerate() {
            if s.contains("schema") || s.contains("Schema") || s.contains("SCHEMA") {
                println!("  [{i}] ({} chars): {}...", s.len(), &s[..s.len().min(100)]);
            }
        }
        // Show first 5 statements
        for (i, s) in stmts.iter().take(5).enumerate() {
            println!(
                "  STMT[{i}] ({} chars): {}",
                s.len(),
                &s[..s.len().min(120)]
            );
        }
        // The schemas table should be one complete statement
        let schemas_create = stmts
            .iter()
            .find(|s| s.contains("CREATE TABLE") && s.contains("schemas"));
        assert!(
            schemas_create.is_some(),
            "schemas CREATE TABLE not found as a complete statement"
        );
        let stmt = schemas_create.unwrap();
        println!("schemas CREATE TABLE: {}", stmt);
        assert!(
            stmt.contains("created_at"),
            "Statement is incomplete: {}",
            stmt
        );
    }

    #[test]
    fn test_split_create_table_with_defaults() {
        let sql = "CREATE TABLE IF NOT EXISTS orgs (\n    id TEXT PRIMARY KEY,\n    name TEXT NOT NULL,\n    created_at TEXT NOT NULL DEFAULT (datetime('now'))\n);\n\nCREATE TABLE IF NOT EXISTS schemas (\n    id TEXT PRIMARY KEY,\n    type TEXT NOT NULL\n);\nCREATE INDEX IF NOT EXISTS idx_schema_type ON schemas(type);";
        let stmts = split_statements(sql);
        println!("Got {} statements:", stmts.len());
        for (i, s) in stmts.iter().enumerate() {
            println!("  [{i}]: {}", &s[..s.len().min(80)]);
        }
        assert_eq!(stmts.len(), 3);
    }

    #[test]
    fn spanner_baseline_is_native_googlesql_only() {
        let (_, sql) = schema::embedded_migrations(BackendKind::Spanner)
            .first()
            .expect("spanner baseline migration must exist");
        let up = extract_goose_up(sql);

        for forbidden in [
            "JSONB",
            "BYTEA",
            "TIMESTAMPTZ",
            "ON CONFLICT",
            "INSERT INTO ",
            "ALTER FOREIGN KEY",
            "ON DELETE SET NULL",
            "DROP TABLE IF EXISTS",
            "fga_instance_stores",
        ] {
            assert!(
                !up.contains(forbidden),
                "spanner baseline still contains forbidden token: {forbidden}"
            );
        }

        assert!(
            up.contains("STRING(MAX)") && up.contains("CREATE TABLE IF NOT EXISTS instances"),
            "spanner baseline should contain GoogleSQL string types and the instances table"
        );
        assert!(
            up.contains("CREATE TABLE IF NOT EXISTS `groups`"),
            "spanner baseline should quote reserved table names like `groups`"
        );
        assert!(
            up.contains("parent_instance_id")
                && !up.contains(
                    "FOREIGN KEY (parent_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE"
                ),
            "spanner baseline should avoid self-referential cascade deletes on instances"
        );
        assert!(
            !has_bare_default(&up),
            "spanner baseline still contains DEFAULT values without GoogleSQL parentheses"
        );
        assert!(
            up.contains("dns_authorization_id") && up.contains("source_key"),
            "spanner baseline should include the squashed domain and effects columns"
        );
        assert!(
            up.contains("GENERATED BY DEFAULT AS IDENTITY (BIT_REVERSED_POSITIVE)"),
            "spanner baseline should specify an explicit sequence kind for identity columns"
        );
    }

    fn has_bare_default(sql: &str) -> bool {
        let mut search_from = 0usize;
        while let Some(offset) = sql[search_from..].find("DEFAULT ") {
            let idx = search_from + offset + "DEFAULT ".len();
            let rest = &sql[idx..];
            if !rest.starts_with('(') && !rest.starts_with("AS IDENTITY") {
                return true;
            }
            search_from = idx;
        }
        false
    }

    #[tokio::test]
    async fn test_schemas_table_creation() {
        let db = Db::open("").await.unwrap();
        let schema_sql = "CREATE TABLE IF NOT EXISTS schemas (\n    id          TEXT PRIMARY KEY,\n    type        TEXT NOT NULL,\n    schema      TEXT NOT NULL,\n    version     INTEGER DEFAULT 1,\n    is_default  BOOLEAN DEFAULT false,\n    visibility  TEXT NOT NULL DEFAULT 'private',\n    message     TEXT DEFAULT '',\n    created_by  TEXT DEFAULT '',\n    created_at  TEXT NOT NULL DEFAULT (datetime('now'))\n)";
        let r = sqlx::query(schema_sql).execute(&*db.pool()).await;
        println!("schemas table: {:?}", r);
        assert!(r.is_ok(), "Failed: {:?}", r.err());

        // Now create index
        let r = sqlx::query("CREATE INDEX IF NOT EXISTS idx_schema_type ON schemas(type)")
            .execute(&*db.pool())
            .await;
        println!("schemas index: {:?}", r);
        assert!(r.is_ok(), "Failed: {:?}", r.err());
    }

    #[tokio::test]
    async fn generated_sqlite_auth_states_statement_executes() {
        let db = Db::open("").await.unwrap();
        let (_, sql) = schema::embedded_migrations(BackendKind::Sqlite)
            .first()
            .expect("sqlite baseline migration must exist");
        let up = extract_goose_up(sql);
        let statements = split_statements(&up);
        let statement = statements
            .get(2)
            .expect("sqlite baseline should include auth_states");
        let result = sqlx::query(statement).execute(&*db.pool()).await;
        assert!(
            result.is_ok(),
            "auth_states statement failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn sqlite_accepts_datetime_modifier_defaults() {
        let db = Db::open("").await.unwrap();
        let result = sqlx::query(
            "CREATE TABLE IF NOT EXISTS auth_state_probe (
                expires_at TEXT NOT NULL DEFAULT (datetime('now', '+10 minutes'))
            )",
        )
        .execute(&*db.pool())
        .await;
        assert!(
            result.is_ok(),
            "datetime modifier default failed: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn migrate_in_memory_sqlite() {
        let db = Db::open("").await.unwrap();
        migrate(&db).await.unwrap();

        // Verify tables exist.
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(row.0, 0);

        // Verify instance_id column exists (from migration 2).
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM users WHERE instance_id = 'default'")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(row.0, 0);

        let columns: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM pragma_table_info('effects')")
                .fetch_all(db.pool())
                .await
                .unwrap();
        let column_names = columns.into_iter().map(|row| row.0).collect::<Vec<_>>();
        assert!(column_names.iter().any(|column| column == "source_key"));
        assert!(column_names.iter().any(|column| column == "lease_owner"));
        assert!(
            column_names
                .iter()
                .any(|column| column == "lease_expires_at")
        );
    }
}
