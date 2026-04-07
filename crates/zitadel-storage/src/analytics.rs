use google_cloud_googleapis::spanner::v1::{
    Type as SpannerType, TypeCode, struct_type::Field as SpannerField,
};
use google_cloud_spanner::row::{
    Error as SpannerRowError, TryFromValue as SpannerTryFromValue, as_ref as spanner_as_ref,
};
use google_cloud_spanner::statement::Statement;
use prost_types::{Value as ProtoValue, value::Kind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Instant;
use zitadel_db::{Db, Dialect};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsQuery {
    pub sql: String,
    /// Bind parameters for `$1, $2, ...` placeholders in the SQL.
    /// All values are bound as strings — the DB engine handles type coercion.
    #[serde(default)]
    pub params: Vec<String>,
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

        let mut q = sqlx::query(&sql);
        for p in &query.params {
            q = q.bind(p);
        }
        let rows = match q.fetch_all(self.db.pool()).await {
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
        let rewritten = rewrite_spanner_placeholders(trimmed);
        let sql = if upper.contains("LIMIT") {
            rewritten
        } else {
            format!("{rewritten} LIMIT {limit}")
        };
        let client = self
            .db
            .spanner()
            .expect("spanner analytics backend requires native spanner client")
            .client();
        let mut stmt = Statement::new(sql);
        for (i, p) in query.params.iter().enumerate() {
            stmt.add_param(&format!("p{}", i + 1), p);
        }
        let mut tx = client.single().await?;
        let mut rows = match tx.query(stmt).await {
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

        let mut columns = Vec::new();
        let mut column_types = Vec::new();
        let mut result_rows = Vec::new();
        while let Some(row) = rows.next().await? {
            if columns.is_empty() {
                let metadata = rows.columns_metadata();
                columns = metadata
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        if field.name.is_empty() {
                            format!("column_{}", index + 1)
                        } else {
                            field.name.clone()
                        }
                    })
                    .collect();
                column_types = metadata.iter().map(spanner_column_type_name).collect();
            }

            let mut values = Vec::with_capacity(columns.len());
            for idx in 0..columns.len() {
                values.push(row.column::<SpannerJsonCell>(idx)?.0);
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

#[derive(Debug, Clone)]
struct SpannerJsonCell(Value);

impl SpannerTryFromValue for SpannerJsonCell {
    fn try_from(item: &ProtoValue, field: &SpannerField) -> Result<Self, SpannerRowError> {
        Ok(Self(spanner_value_to_json(item, field)?))
    }
}

fn spanner_value_to_json(
    item: &ProtoValue,
    field: &SpannerField,
) -> Result<Value, SpannerRowError> {
    let kind = spanner_as_ref(item, field)?;
    if matches!(kind, Kind::NullValue(_)) {
        return Ok(Value::Null);
    }

    let Some(spanner_type) = field.r#type.as_ref() else {
        return Ok(spanner_kind_to_json(kind));
    };

    let type_code = TypeCode::try_from(spanner_type.code).unwrap_or(TypeCode::Unspecified);
    match type_code {
        TypeCode::Bool => Ok(spanner_kind_to_json(kind)),
        TypeCode::Int64 | TypeCode::Enum => Ok(match kind {
            Kind::StringValue(raw) => raw
                .parse::<i64>()
                .map(|value| Value::Number(value.into()))
                .unwrap_or_else(|_| Value::String(raw.clone())),
            _ => spanner_kind_to_json(kind),
        }),
        TypeCode::Float64 | TypeCode::Float32 => Ok(match kind {
            Kind::NumberValue(value) => serde_json::Number::from_f64(*value)
                .map(Value::Number)
                .unwrap_or_else(|| Value::String(value.to_string())),
            Kind::StringValue(raw) => Value::String(raw.clone()),
            _ => spanner_kind_to_json(kind),
        }),
        TypeCode::Json => Ok(match kind {
            Kind::StringValue(raw) => {
                serde_json::from_str::<Value>(raw).unwrap_or_else(|_| Value::String(raw.clone()))
            }
            _ => spanner_kind_to_json(kind),
        }),
        TypeCode::Array => spanner_array_to_json(kind, spanner_type, field.name.as_str()),
        TypeCode::Struct => spanner_struct_to_json(kind, spanner_type),
        TypeCode::Unspecified
        | TypeCode::Timestamp
        | TypeCode::Date
        | TypeCode::String
        | TypeCode::Bytes
        | TypeCode::Numeric
        | TypeCode::Proto => Ok(spanner_kind_to_json(kind)),
    }
}

fn spanner_array_to_json(
    kind: &Kind,
    spanner_type: &SpannerType,
    field_name: &str,
) -> Result<Value, SpannerRowError> {
    let Kind::ListValue(list) = kind else {
        return Ok(spanner_kind_to_json(kind));
    };
    let Some(element_type) = spanner_type.array_element_type.as_ref() else {
        return Ok(spanner_kind_to_json(kind));
    };
    let element_field = spanner_nested_field(field_name, *element_type.clone());
    let mut values = Vec::with_capacity(list.values.len());
    for value in &list.values {
        values.push(spanner_value_to_json(value, &element_field)?);
    }
    Ok(Value::Array(values))
}

fn spanner_struct_to_json(
    kind: &Kind,
    spanner_type: &SpannerType,
) -> Result<Value, SpannerRowError> {
    let Some(struct_type) = spanner_type.struct_type.as_ref() else {
        return Ok(spanner_kind_to_json(kind));
    };

    let mut object = serde_json::Map::new();
    match kind {
        Kind::ListValue(list) => {
            for (index, nested_field) in struct_type.fields.iter().enumerate() {
                let key = spanner_struct_field_name(nested_field, index);
                let value = match list.values.get(index) {
                    Some(value) => spanner_value_to_json(value, nested_field)?,
                    None => Value::Null,
                };
                object.insert(key, value);
            }
        }
        Kind::StructValue(values) => {
            for (index, nested_field) in struct_type.fields.iter().enumerate() {
                let key = spanner_struct_field_name(nested_field, index);
                let value = match values.fields.get(&nested_field.name) {
                    Some(value) => spanner_value_to_json(value, nested_field)?,
                    None => Value::Null,
                };
                object.insert(key, value);
            }
        }
        _ => return Ok(spanner_kind_to_json(kind)),
    }

    Ok(Value::Object(object))
}

fn spanner_kind_to_json(kind: &Kind) -> Value {
    match kind {
        Kind::NullValue(_) => Value::Null,
        Kind::BoolValue(flag) => Value::Bool(*flag),
        Kind::NumberValue(value) => serde_json::Number::from_f64(*value)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(value.to_string())),
        Kind::StringValue(raw) => Value::String(raw.clone()),
        Kind::ListValue(list) => Value::Array(
            list.values
                .iter()
                .map(spanner_proto_kindless_to_json)
                .collect::<Vec<_>>(),
        ),
        Kind::StructValue(values) => {
            let mut object = serde_json::Map::new();
            for (key, value) in &values.fields {
                object.insert(key.clone(), spanner_proto_kindless_to_json(value));
            }
            Value::Object(object)
        }
    }
}

fn spanner_proto_kindless_to_json(value: &ProtoValue) -> Value {
    value
        .kind
        .as_ref()
        .map(spanner_kind_to_json)
        .unwrap_or(Value::Null)
}

fn spanner_nested_field(name: &str, spanner_type: SpannerType) -> SpannerField {
    SpannerField {
        name: name.to_string(),
        r#type: Some(spanner_type),
    }
}

fn spanner_struct_field_name(field: &SpannerField, index: usize) -> String {
    if field.name.is_empty() {
        format!("field_{}", index + 1)
    } else {
        field.name.clone()
    }
}

fn spanner_column_type_name(field: &SpannerField) -> String {
    field
        .r#type
        .as_ref()
        .map(spanner_type_name)
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

fn spanner_type_name(spanner_type: &SpannerType) -> String {
    match TypeCode::try_from(spanner_type.code).unwrap_or(TypeCode::Unspecified) {
        TypeCode::Bool => "BOOL".into(),
        TypeCode::Int64 => "INT64".into(),
        TypeCode::Float64 => "FLOAT64".into(),
        TypeCode::Float32 => "FLOAT32".into(),
        TypeCode::Timestamp => "TIMESTAMP".into(),
        TypeCode::Date => "DATE".into(),
        TypeCode::String => "STRING".into(),
        TypeCode::Bytes => "BYTES".into(),
        TypeCode::Numeric => "NUMERIC".into(),
        TypeCode::Json => "JSON".into(),
        TypeCode::Proto => "PROTO".into(),
        TypeCode::Enum => "ENUM".into(),
        TypeCode::Array => spanner_type
            .array_element_type
            .as_ref()
            .map(|element| format!("ARRAY<{}>", spanner_type_name(element)))
            .unwrap_or_else(|| "ARRAY<UNKNOWN>".into()),
        TypeCode::Struct => spanner_type
            .struct_type
            .as_ref()
            .map(|struct_type| {
                let fields = struct_type
                    .fields
                    .iter()
                    .map(|field| {
                        let name = if field.name.is_empty() {
                            "field".to_string()
                        } else {
                            field.name.clone()
                        };
                        format!("{name} {}", spanner_column_type_name(field))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("STRUCT<{fields}>")
            })
            .unwrap_or_else(|| "STRUCT<>".into()),
        TypeCode::Unspecified => "UNKNOWN".into(),
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

fn rewrite_spanner_placeholders(query: &str) -> String {
    let mut rewritten = String::with_capacity(query.len());
    let chars: Vec<char> = query.chars().collect();
    let mut index = 0;
    let mut in_string = false;

    while index < chars.len() {
        let ch = chars[index];
        if ch == '\'' {
            rewritten.push(ch);
            if in_string && chars.get(index + 1) == Some(&'\'') {
                rewritten.push('\'');
                index += 2;
                continue;
            }
            in_string = !in_string;
            index += 1;
            continue;
        }

        if !in_string && ch == '$' {
            let start = index + 1;
            let mut end = start;
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }

            if end > start {
                rewritten.push('@');
                rewritten.push('p');
                for digit in &chars[start..end] {
                    rewritten.push(*digit);
                }
                index = end;
                continue;
            }
        }

        rewritten.push(ch);
        index += 1;
    }

    rewritten
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

    #[test]
    fn rewrites_spanner_placeholders_without_touching_strings() {
        assert_eq!(
            rewrite_spanner_placeholders(
                "SELECT * FROM events WHERE category = $1 AND note = '$2 literal' AND seq = $10"
            ),
            "SELECT * FROM events WHERE category = @p1 AND note = '$2 literal' AND seq = @p10"
        );
    }

    #[tokio::test]
    async fn query_and_schema_work_for_sqlite_backend() {
        let db = Db::open("").await.unwrap();
        zitadel_db::migrate::migrate(&db).await.unwrap();
        let backend = SqlAnalyticsQueryBackend::new(db.clone());
        let storage = DefaultAnalyticsStorage::new_sql(NoopAnalyticsSink, backend);

        let result = storage
            .query(&AnalyticsQuery {
                sql: "SELECT COUNT(*) AS count FROM users".into(),
                params: vec![],
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
