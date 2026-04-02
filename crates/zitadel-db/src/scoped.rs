use crate::Dialect;
use sqlx::AnyPool;
use std::fmt::Write;

/// ScopedDb wraps a pool + instance_id for multi-tenant query scoping.
/// All queries should use `instance_id()` in their WHERE clause.
#[derive(Clone)]
pub struct ScopedDb {
    pool: AnyPool,
    dialect: Dialect,
    instance_id: String,
}

impl ScopedDb {
    pub fn new(pool: AnyPool, dialect: Dialect, instance_id: String) -> Self {
        Self { pool, dialect, instance_id }
    }

    pub fn pool(&self) -> &AnyPool { &self.pool }
    pub fn dialect(&self) -> Dialect { self.dialect }
    pub fn instance_id(&self) -> &str { &self.instance_id }

    /// Returns dialect-specific parameter placeholder.
    /// SQLite: `?`, Postgres: `$n`
    pub fn placeholder(&self, n: usize) -> String {
        match self.dialect {
            Dialect::Postgres => format!("${n}"),
            Dialect::Sqlite => "?".to_string(),
        }
    }

    /// Returns dialect-specific JSON extraction.
    /// SQLite: `json_extract(col, '$.path')`, Postgres: `col->>'path'`
    pub fn json_extract(&self, column: &str, path: &str) -> String {
        match self.dialect {
            Dialect::Postgres => format!("{column}->>'{path}'"),
            Dialect::Sqlite => format!("json_extract({column}, '$.{path}')"),
        }
    }

    /// Returns dialect-specific current timestamp expression.
    pub fn timestamp_now(&self) -> &str {
        match self.dialect {
            Dialect::Postgres => "NOW()",
            Dialect::Sqlite => "datetime('now')",
        }
    }

    /// Rewrite `?` placeholders to dialect-specific format.
    /// SQLite keeps `?`, Postgres converts to `$1, $2, ...`
    pub fn rebind(&self, query: &str) -> String {
        rebind_placeholders(query, self.dialect)
    }
}

/// Rewrite `?` placeholders for the given dialect.
pub fn rebind_placeholders(query: &str, dialect: Dialect) -> String {
    if dialect != Dialect::Postgres {
        return query.to_string();
    }
    let mut out = String::with_capacity(query.len() + 8);
    let mut index = 1usize;
    for ch in query.chars() {
        if ch == '?' {
            let _ = write!(out, "${index}");
            index += 1;
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebind_sqlite_noop() {
        let q = "SELECT * FROM users WHERE instance_id = ? AND id = ?";
        assert_eq!(rebind_placeholders(q, Dialect::Sqlite), q);
    }

    #[test]
    fn rebind_postgres() {
        let q = "SELECT * FROM users WHERE instance_id = ? AND id = ?";
        assert_eq!(
            rebind_placeholders(q, Dialect::Postgres),
            "SELECT * FROM users WHERE instance_id = $1 AND id = $2"
        );
    }
}
