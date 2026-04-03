use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Instant;
use zitadel_db::{Db, Dialect};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsQuery {
    pub sql: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsQueryResult {
    pub columns: Vec<String>,
    pub column_types: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub row_count: usize,
    pub execution_ms: u64,
    pub error: Option<String>,
}

pub trait AnalyticsSink: Clone + Send + Sync + 'static {
    async fn append(&self, event: &Value) -> anyhow::Result<()>;
}

#[derive(Clone, Default)]
pub struct NoopAnalyticsSink;

impl AnalyticsSink for NoopAnalyticsSink {
    async fn append(&self, _event: &Value) -> anyhow::Result<()> {
        Ok(())
    }
}

pub trait AnalyticsQueryBackend: Clone + Send + Sync + 'static {
    async fn query(&self, query: &AnalyticsQuery) -> anyhow::Result<AnalyticsQueryResult>;
    async fn schema(&self) -> anyhow::Result<Value>;
}

#[derive(Clone)]
pub struct SqlAnalyticsQueryBackend {
    db: Db,
}

impl SqlAnalyticsQueryBackend {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl AnalyticsQueryBackend for SqlAnalyticsQueryBackend {
    async fn query(&self, query: &AnalyticsQuery) -> anyhow::Result<AnalyticsQueryResult> {
        let start = Instant::now();
        let trimmed = query.sql.trim();
        if trimmed.is_empty() {
            anyhow::bail!("sql is required");
        }

        let upper = trimmed.to_uppercase();
        if !upper.starts_with("SELECT")
            && !upper.starts_with("WITH")
            && !upper.starts_with("EXPLAIN")
        {
            anyhow::bail!("only SELECT, WITH, and EXPLAIN queries are allowed");
        }

        let limit = query.limit.unwrap_or(1000).min(10000);
        let sql = if upper.contains("LIMIT") {
            trimmed.to_string()
        } else {
            format!("{trimmed} LIMIT {limit}")
        };

        let rows = match sqlx::query(&sql).fetch_all(self.db.pool()).await {
            Ok(rows) => rows,
            Err(error) => {
                return Ok(AnalyticsQueryResult {
                    columns: Vec::new(),
                    column_types: Vec::new(),
                    rows: Vec::new(),
                    row_count: 0,
                    execution_ms: start.elapsed().as_millis() as u64,
                    error: Some(error.to_string()),
                });
            }
        };

        if rows.is_empty() {
            return Ok(AnalyticsQueryResult {
                columns: Vec::new(),
                column_types: Vec::new(),
                rows: Vec::new(),
                row_count: 0,
                execution_ms: start.elapsed().as_millis() as u64,
                error: None,
            });
        }

        use sqlx::{Column, Row, TypeInfo};

        let columns: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|col| col.name().to_string())
            .collect();
        let column_types: Vec<String> = rows[0]
            .columns()
            .iter()
            .map(|col| col.type_info().name().to_string())
            .collect();
        let mut result_rows = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut values = Vec::with_capacity(columns.len());
            for idx in 0..columns.len() {
                values.push(extract_value(row, idx));
            }
            result_rows.push(values);
        }

        Ok(AnalyticsQueryResult {
            columns,
            column_types,
            row_count: result_rows.len(),
            rows: result_rows,
            execution_ms: start.elapsed().as_millis() as u64,
            error: None,
        })
    }

    async fn schema(&self) -> anyhow::Result<Value> {
        let mut tables = serde_json::Map::new();
        for table_name in list_tables(&self.db).await? {
            let columns = list_columns(&self.db, &table_name).await?;
            let count_sql = format!(
                "SELECT COUNT(*) FROM \"{}\"",
                table_name.replace('"', "\"\"")
            );
            let row_count = sqlx::query_as::<_, (i64,)>(&count_sql)
                .fetch_one(self.db.pool())
                .await
                .map(|r| r.0)
                .unwrap_or(0);
            tables.insert(
                table_name.clone(),
                serde_json::json!({
                    "name": table_name,
                    "columns": columns,
                    "row_count": row_count,
                    "file_count": 0,
                    "last_update": "",
                }),
            );
        }
        Ok(Value::Object(tables))
    }
}

async fn list_tables(db: &Db) -> anyhow::Result<Vec<String>> {
    match db.dialect() {
        Dialect::Sqlite => {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name NOT LIKE '_sqlx_%' ORDER BY name",
            )
            .fetch_all(db.pool())
            .await?;
            Ok(rows.into_iter().map(|(name,)| name).collect())
        }
        Dialect::Postgres => {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT table_name FROM information_schema.tables WHERE table_schema = 'public' AND table_type = 'BASE TABLE' ORDER BY table_name",
            )
            .fetch_all(db.pool())
            .await?;
            Ok(rows.into_iter().map(|(name,)| name).collect())
        }
    }
}

async fn list_columns(db: &Db, table_name: &str) -> anyhow::Result<Vec<Value>> {
    match db.dialect() {
        Dialect::Sqlite => {
            let pragma_sql = format!("PRAGMA table_info('{}')", table_name.replace('\'', "''"));
            let rows: Vec<(i64, String, String, i64, Option<String>, i64)> =
                sqlx::query_as(&pragma_sql).fetch_all(db.pool()).await?;
            Ok(rows
                .into_iter()
                .map(|(_, name, col_type, _, _, _)| serde_json::json!({ "name": name, "type": col_type }))
                .collect())
        }
        Dialect::Postgres => {
            let rows: Vec<(String, String)> = sqlx::query_as(
                "SELECT column_name, data_type FROM information_schema.columns WHERE table_schema = 'public' AND table_name = $1 ORDER BY ordinal_position",
            )
            .bind(table_name)
            .fetch_all(db.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|(name, col_type)| serde_json::json!({ "name": name, "type": col_type }))
                .collect())
        }
    }
}

fn extract_value(row: &sqlx::any::AnyRow, idx: usize) -> Value {
    use sqlx::{Column, Row, TypeInfo, ValueRef};

    if row
        .try_get_raw(idx)
        .map(|value| value.is_null())
        .unwrap_or(true)
    {
        return Value::Null;
    }

    let type_name = row.columns()[idx].type_info().name().to_uppercase();
    if type_name.contains("INT")
        && let Ok(value) = row.try_get::<i64, _>(idx)
    {
        return Value::Number(value.into());
    }
    if (type_name.contains("REAL")
        || type_name.contains("FLOAT")
        || type_name.contains("DOUBLE")
        || type_name.contains("NUMERIC"))
        && let Ok(value) = row.try_get::<f64, _>(idx)
        && let Some(number) = serde_json::Number::from_f64(value)
    {
        return Value::Number(number);
    }
    if type_name.contains("BOOL")
        && let Ok(value) = row.try_get::<bool, _>(idx)
    {
        return Value::Bool(value);
    }
    if let Ok(value) = row.try_get::<String, _>(idx) {
        return Value::String(value);
    }
    if let Ok(value) = row.try_get::<i64, _>(idx) {
        return Value::Number(value.into());
    }
    if let Ok(value) = row.try_get::<i32, _>(idx) {
        return Value::Number(value.into());
    }
    Value::Null
}

#[derive(Clone)]
pub struct AnalyticsStorage<S, Q> {
    sink: S,
    query_backend: Q,
}

impl<S, Q> AnalyticsStorage<S, Q> {
    pub fn new(sink: S, query_backend: Q) -> Self {
        Self {
            sink,
            query_backend,
        }
    }

    pub fn sink(&self) -> &S {
        &self.sink
    }

    pub fn query_backend(&self) -> &Q {
        &self.query_backend
    }
}

impl<S, Q> AnalyticsStorage<S, Q>
where
    S: AnalyticsSink,
    Q: AnalyticsQueryBackend,
{
    pub async fn append(&self, event: &Value) -> anyhow::Result<()> {
        self.sink.append(event).await
    }

    pub async fn query(&self, query: &AnalyticsQuery) -> anyhow::Result<AnalyticsQueryResult> {
        self.query_backend.query(query).await
    }

    pub async fn schema(&self) -> anyhow::Result<Value> {
        self.query_backend.schema().await
    }
}

pub type DefaultAnalyticsStorage = AnalyticsStorage<NoopAnalyticsSink, SqlAnalyticsQueryBackend>;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn query_and_schema_work_for_sqlite_backend() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let backend = SqlAnalyticsQueryBackend::new(db.clone());
        let storage = DefaultAnalyticsStorage::new(NoopAnalyticsSink, backend);

        let result = storage
            .query(&AnalyticsQuery {
                sql: "SELECT COUNT(*) AS count FROM users".into(),
                limit: None,
            })
            .await
            .unwrap();
        assert_eq!(result.row_count, 1);
        assert!(result.error.is_none());

        let schema = storage.schema().await.unwrap();
        assert!(schema.get("users").is_some());
    }
}
