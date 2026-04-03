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
        Self {
            pool,
            dialect,
            instance_id,
        }
    }

    pub fn pool(&self) -> &AnyPool {
        &self.pool
    }
    pub fn dialect(&self) -> Dialect {
        self.dialect
    }
    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    /// Returns a positional parameter placeholder accepted by SQLite and Postgres.
    pub fn placeholder(&self, n: usize) -> String {
        format!("${n}")
    }

    /// Returns dialect-specific JSON extraction.
    /// SQLite: `json_extract(col, '$.path')`, Postgres: `col->>'path'`
    pub fn json_extract(&self, column: &str, path: &str) -> String {
        match self.dialect {
            Dialect::Postgres => format!("{column}->>'{path}'"),
            Dialect::Sqlite => format!("json_extract({column}, '$.{path}')"),
        }
    }

    /// Returns a placeholder expression suitable for inserting/updating JSON values.
    /// SQLite stores JSON as text; Postgres casts the bound text to JSONB.
    pub fn json_bind(&self, n: usize) -> String {
        match self.dialect {
            Dialect::Postgres => format!("CAST(${n} AS JSONB)"),
            Dialect::Sqlite => format!("${n}"),
        }
    }

    /// Cast a column/expression to text so handlers can decode through `sqlx::Any`.
    pub fn as_text(&self, expr: &str) -> String {
        format!("CAST({expr} AS TEXT)")
    }

    /// Normalize booleans to integer 0/1 across SQLite and Postgres.
    pub fn bool_as_int(&self, expr: &str) -> String {
        format!("CASE WHEN {expr} THEN 1 ELSE 0 END")
    }

    /// Convert a timestamp expression into unix epoch seconds.
    pub fn epoch_seconds(&self, expr: &str) -> String {
        match self.dialect {
            Dialect::Postgres => format!("CAST(EXTRACT(EPOCH FROM {expr}) AS BIGINT)"),
            Dialect::Sqlite => format!("CAST(strftime('%s', {expr}) AS INTEGER)"),
        }
    }

    /// Returns dialect-specific current timestamp expression.
    pub fn timestamp_now(&self) -> &str {
        "CURRENT_TIMESTAMP"
    }

    /// Convenience: returns `(as_text("created_at"), as_text("updated_at"))`.
    pub fn select_timestamps(&self) -> (String, String) {
        (self.as_text("created_at"), self.as_text("updated_at"))
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
