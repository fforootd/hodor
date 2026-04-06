use crate::{BackendKind, Db};
use sqlx::{Connection, Executor};

/// Embedded migration SQL files.
const SQLITE_MIGRATIONS: &[(&str, &str)] = &[
    (
        "00001_initial",
        include_str!("../../../migrations/sqlite/00001_initial.sql"),
    ),
    (
        "00010_oidc_logout_runtime",
        include_str!("../../../migrations/sqlite/00010_oidc_logout_runtime.sql"),
    ),
    (
        "00011_optional_org",
        include_str!("../../../migrations/sqlite/00011_optional_org.sql"),
    ),
    (
        "00012_org_fk_set_null",
        include_str!("../../../migrations/sqlite/00012_org_fk_set_null.sql"),
    ),
    (
        "00013_role_catalog",
        include_str!("../../../migrations/sqlite/00013_role_catalog.sql"),
    ),
    (
        "00014_fga_scope_cleanup",
        include_str!("../../../migrations/sqlite/00014_fga_scope_cleanup.sql"),
    ),
];

const POSTGRES_MIGRATIONS: &[(&str, &str)] = &[
    (
        "00001_initial",
        include_str!("../../../migrations/postgres/00001_initial.sql"),
    ),
    (
        "00010_oidc_logout_runtime",
        include_str!("../../../migrations/postgres/00010_oidc_logout_runtime.sql"),
    ),
    (
        "00011_optional_org",
        include_str!("../../../migrations/postgres/00011_optional_org.sql"),
    ),
    (
        "00012_org_fk_set_null",
        include_str!("../../../migrations/postgres/00012_org_fk_set_null.sql"),
    ),
    (
        "00013_role_catalog",
        include_str!("../../../migrations/postgres/00013_role_catalog.sql"),
    ),
    (
        "00014_fga_scope_cleanup",
        include_str!("../../../migrations/postgres/00014_fga_scope_cleanup.sql"),
    ),
];

const SPANNER_MIGRATIONS: &[(&str, &str)] = &[
    (
        "00001_initial",
        include_str!("../../../migrations/spanner/00001_initial.sql"),
    ),
    (
        "00002_oidc_logout_runtime",
        include_str!("../../../migrations/spanner/00002_oidc_logout_runtime.sql"),
    ),
    (
        "00003_optional_org",
        include_str!("../../../migrations/spanner/00003_optional_org.sql"),
    ),
    (
        "00004_org_fk_set_null",
        include_str!("../../../migrations/spanner/00004_org_fk_set_null.sql"),
    ),
    (
        "00005_role_catalog",
        include_str!("../../../migrations/spanner/00005_role_catalog.sql"),
    ),
    (
        "00006_fga_scope_cleanup",
        include_str!("../../../migrations/spanner/00006_fga_scope_cleanup.sql"),
    ),
];

const POSTGRES_MIGRATION_LOCK_ID: i64 = 6_900_181_427_071;

/// Run all pending migrations.
pub async fn migrate(db: &Db) -> anyhow::Result<()> {
    let backend = db.backend();
    let dialect = db.dialect();

    let migrations = match backend {
        BackendKind::Sqlite => SQLITE_MIGRATIONS,
        BackendKind::Postgres => POSTGRES_MIGRATIONS,
        BackendKind::Spanner => SPANNER_MIGRATIONS,
    };

    if backend == BackendKind::Spanner {
        let statements = SPANNER_MIGRATIONS
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

    let total = migrations.len() as i64;
    tracing::info!(
        backend = %backend,
        dialect = %dialect,
        version = total,
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
        BackendKind::Sqlite => SQLITE_MIGRATIONS.len() as i64,
        BackendKind::Postgres => POSTGRES_MIGRATIONS.len() as i64,
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
        tracing::warn!(current, target, "schema version is ahead of binary target");
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

    #[tokio::test]
    async fn sqlite_fga_scope_cleanup_migration_copies_data_and_deletes_platform_instance() {
        let db = Db::open("").await.unwrap();
        let pool = db.pool();

        sqlx::query(
            "CREATE TABLE _schema_version (
                version INTEGER NOT NULL,
                applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
            )",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO _schema_version (version) VALUES (5)")
            .execute(pool)
            .await
            .unwrap();

        sqlx::query(
            "CREATE TABLE instances (
                instance_id TEXT PRIMARY KEY,
                kind TEXT NOT NULL DEFAULT 'managed',
                state TEXT NOT NULL DEFAULT 'active',
                placement_mode TEXT NOT NULL DEFAULT 'global',
                feature_overrides TEXT NOT NULL DEFAULT '{}'
            )",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE fga_instance_stores (
                instance_id TEXT PRIMARY KEY,
                store_id TEXT NOT NULL UNIQUE
            )",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE fga_authorization_models (
                instance_id TEXT NOT NULL,
                store_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                schema_version TEXT NOT NULL,
                core_model_version TEXT NOT NULL DEFAULT '',
                compiled_model TEXT NOT NULL,
                custom_model TEXT NOT NULL DEFAULT '{}',
                module_fragments TEXT NOT NULL DEFAULT '[]',
                is_active INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (instance_id, store_id, model_id)
            )",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE fga_tuples (
                instance_id TEXT NOT NULL,
                store_id TEXT NOT NULL,
                object_type TEXT NOT NULL,
                object_id TEXT NOT NULL,
                relation TEXT NOT NULL,
                user_type TEXT NOT NULL,
                user_id TEXT NOT NULL,
                user_relation TEXT NOT NULL DEFAULT '',
                raw_object TEXT NOT NULL,
                raw_user TEXT NOT NULL,
                inserted_at TEXT NOT NULL DEFAULT (datetime('now')),
                PRIMARY KEY (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation)
            )",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TABLE fga_tuple_changes (
                seq INTEGER PRIMARY KEY AUTOINCREMENT,
                instance_id TEXT NOT NULL,
                store_id TEXT NOT NULL,
                operation TEXT NOT NULL,
                object_type TEXT NOT NULL,
                object_id TEXT NOT NULL,
                relation TEXT NOT NULL,
                user_type TEXT NOT NULL,
                user_id TEXT NOT NULL,
                user_relation TEXT NOT NULL DEFAULT '',
                raw_object TEXT NOT NULL,
                raw_user TEXT NOT NULL,
                authorization_model_id TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO instances (instance_id, kind, state, placement_mode, feature_overrides)
             VALUES ('_platform', 'managed', 'active', 'global', '{}')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO fga_instance_stores (instance_id, store_id) VALUES ('_platform', '_platform')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO fga_authorization_models
             (instance_id, store_id, model_id, schema_version, compiled_model, is_active)
             VALUES ('_platform', '_platform', 'model-1', '1.1', '{\"type_definitions\":[],\"conditions\":{}}', 1)",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO fga_tuples
             (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user)
             VALUES ('_platform', '_platform', 'instance', 'child-a', 'admin', 'user', 'anne', '', 'instance:child-a', 'user:anne')",
        )
        .execute(pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO fga_tuple_changes
             (seq, instance_id, store_id, operation, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user, authorization_model_id)
             VALUES (42, '_platform', '_platform', 'WRITE', 'instance', 'child-a', 'admin', 'user', 'anne', '', 'instance:child-a', 'user:anne', 'model-1')",
        )
        .execute(pool)
        .await
        .unwrap();

        migrate(&db).await.unwrap();

        let scope_id: String =
            sqlx::query_scalar("SELECT scope_id FROM fga_stores WHERE store_id = '_platform'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(scope_id, "_platform");

        let tuple_scope: String = sqlx::query_scalar(
            "SELECT scope_id FROM fga_tuples WHERE store_id = '_platform' LIMIT 1",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(tuple_scope, "_platform");

        let change_seq: i64 = sqlx::query_scalar(
            "SELECT seq FROM fga_tuple_changes WHERE store_id = '_platform' LIMIT 1",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(change_seq, 42);

        let platform_row_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM instances WHERE instance_id = '_platform'")
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(platform_row_count, 0);
    }

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
        let sql = include_str!("../../../migrations/sqlite/00001_initial.sql");
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
        let (_, sql) = SPANNER_MIGRATIONS
            .first()
            .expect("spanner baseline migration must exist");
        let up = extract_goose_up(sql);

        for forbidden in [
            "JSONB",
            "BYTEA",
            "TIMESTAMPTZ",
            "ON CONFLICT",
            "INSERT INTO ",
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
    }
}
