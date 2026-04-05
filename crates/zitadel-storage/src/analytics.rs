use google_cloud_spanner::statement::Statement;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
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

#[derive(Clone)]
pub struct SpannerAnalyticsQueryBackend {
    db: Db,
}

impl SpannerAnalyticsQueryBackend {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl AnalyticsQueryBackend for SpannerAnalyticsQueryBackend {
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
        let wrapped = format!(
            "SELECT TO_JSON_STRING(row_data) AS row_json FROM ({trimmed}) AS row_data LIMIT {limit}"
        );
        let client = self
            .db
            .spanner()
            .expect("spanner analytics backend requires native spanner client")
            .client();
        let mut tx = client.single().await?;
        let mut rows = match tx.query(Statement::new(wrapped)).await {
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

        let mut decoded = Vec::new();
        while let Some(row) = rows.next().await? {
            let json = row.column_by_name::<String>("row_json")?;
            let value = serde_json::from_str::<Value>(&json).unwrap_or(Value::String(json));
            decoded.push(value);
        }

        let (columns, result_rows) = values_to_tabular_rows(&decoded);
        let column_types = vec!["JSON".to_string(); columns.len()];
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
        let client = self
            .db
            .spanner()
            .expect("spanner analytics backend requires native spanner client")
            .client();
        let stmt = Statement::new(
            "SELECT table_name, column_name, spanner_type \
             FROM INFORMATION_SCHEMA.COLUMNS \
             WHERE table_schema = '' \
             ORDER BY table_name, ordinal_position",
        );
        let mut tx = client.single().await?;
        let mut rows = tx.query(stmt).await?;
        let mut grouped: BTreeMap<String, Vec<Value>> = BTreeMap::new();
        while let Some(row) = rows.next().await? {
            let table_name = row.column_by_name::<String>("table_name")?;
            let column_name = row.column_by_name::<String>("column_name")?;
            let column_type = row.column_by_name::<String>("spanner_type")?;
            grouped
                .entry(table_name)
                .or_default()
                .push(serde_json::json!({ "name": column_name, "type": column_type }));
        }

        let mut tables = serde_json::Map::new();
        for (table_name, columns) in grouped {
            tables.insert(
                table_name.clone(),
                serde_json::json!({
                    "name": table_name,
                    "columns": columns,
                    "row_count": 0,
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
        Dialect::Spanner => {
            let client = db
                .spanner()
                .expect("spanner analytics helper requires native spanner client")
                .client();
            let mut tx = client.single().await?;
            let mut rows = tx
                .query(Statement::new(
                    "SELECT table_name FROM INFORMATION_SCHEMA.TABLES WHERE table_schema = '' ORDER BY table_name",
                ))
                .await?;
            let mut tables = Vec::new();
            while let Some(row) = rows.next().await? {
                tables.push(row.column_by_name::<String>("table_name")?);
            }
            Ok(tables)
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
        Dialect::Spanner => {
            let client = db
                .spanner()
                .expect("spanner analytics helper requires native spanner client")
                .client();
            let mut tx = client.single().await?;
            let mut stmt = Statement::new(
                "SELECT column_name, spanner_type \
                 FROM INFORMATION_SCHEMA.COLUMNS \
                 WHERE table_schema = '' AND table_name = @table_name \
                 ORDER BY ordinal_position",
            );
            stmt.add_param("table_name", &table_name);
            let mut rows = tx.query(stmt).await?;
            let mut columns = Vec::new();
            while let Some(row) = rows.next().await? {
                columns.push(serde_json::json!({
                    "name": row.column_by_name::<String>("column_name")?,
                    "type": row.column_by_name::<String>("spanner_type")?,
                }));
            }
            Ok(columns)
        }
    }
}

fn values_to_tabular_rows(rows: &[Value]) -> (Vec<String>, Vec<Vec<Value>>) {
    if rows.is_empty() {
        return (Vec::new(), Vec::new());
    }

    if let Some(first) = rows.first().and_then(Value::as_object) {
        let columns = first.keys().cloned().collect::<Vec<_>>();
        let values = rows
            .iter()
            .map(|row| {
                row.as_object()
                    .map(|object| {
                        columns
                            .iter()
                            .map(|column| object.get(column).cloned().unwrap_or(Value::Null))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|| vec![row.clone()])
            })
            .collect::<Vec<_>>();
        return (columns, values);
    }

    (
        vec!["value".to_string()],
        rows.iter().map(|row| vec![row.clone()]).collect(),
    )
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

#[derive(Clone)]
pub enum DefaultAnalyticsStorage {
    Sql(AnalyticsStorage<NoopAnalyticsSink, SqlAnalyticsQueryBackend>),
    Spanner(AnalyticsStorage<NoopAnalyticsSink, SpannerAnalyticsQueryBackend>),
}

impl DefaultAnalyticsStorage {
    pub fn new_sql(sink: NoopAnalyticsSink, backend: SqlAnalyticsQueryBackend) -> Self {
        Self::Sql(AnalyticsStorage::new(sink, backend))
    }

    pub fn new_spanner(sink: NoopAnalyticsSink, backend: SpannerAnalyticsQueryBackend) -> Self {
        Self::Spanner(AnalyticsStorage::new(sink, backend))
    }

    pub async fn append(&self, event: &Value) -> anyhow::Result<()> {
        match self {
            Self::Sql(storage) => storage.append(event).await,
            Self::Spanner(storage) => storage.append(event).await,
        }
    }

    pub async fn query(&self, query: &AnalyticsQuery) -> anyhow::Result<AnalyticsQueryResult> {
        match self {
            Self::Sql(storage) => storage.query(query).await,
            Self::Spanner(storage) => storage.query(query).await,
        }
    }

    pub async fn schema(&self) -> anyhow::Result<Value> {
        match self {
            Self::Sql(storage) => storage.schema().await,
            Self::Spanner(storage) => storage.schema().await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn query_and_schema_work_for_sqlite_backend() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let backend = SqlAnalyticsQueryBackend::new(db.clone());
        let storage = DefaultAnalyticsStorage::new_sql(NoopAnalyticsSink, backend);

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
