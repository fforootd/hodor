use std::{collections::BTreeMap, fmt::Write as _, sync::OnceLock};

use google_cloud_spanner::statement::Statement;
use serde::{Deserialize, Serialize};

use crate::{BackendKind, Db, Dialect};

const CANONICAL_SCHEMA_MANIFEST_JSON: &str = include_str!("schema_manifest.json");

const UNIQUE_INDEX_PREFIX: &str = "uk_";
const SPANNER_HELPER_PREFIX: &str = "spx_";

const JSON_COLUMN_NAMES: &[&str] = &[
    "allowed_scopes",
    "audience",
    "auth_methods",
    "catalog_ref",
    "config",
    "config_json",
    "connection",
    "custom_model",
    "data",
    "feature_overrides",
    "grant_types",
    "linking",
    "mapping",
    "metadata",
    "module_fragments",
    "payload",
    "permissions_json",
    "post_logout_redirect_uris",
    "prompt",
    "raw_claims",
    "raw_data",
    "redirect_uris",
    "response_types",
    "schema",
    "scopes",
    "session",
    "steps",
    "target",
    "ui",
];

const TIMESTAMP_COLUMN_NAMES: &[&str] = &[
    "added_at",
    "auth_time",
    "completed_at",
    "created_at",
    "expires_at",
    "fetched_at",
    "inserted_at",
    "last_active_at",
    "last_heartbeat_at",
    "last_run_at",
    "last_used",
    "last_used_at",
    "lease_expires_at",
    "linked_at",
    "next_retry_at",
    "next_run_at",
    "revoked_at",
    "shipped_at",
    "updated_at",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnTypeFamily {
    String,
    Integer,
    Float,
    Bool,
    Json,
    Timestamp,
    Bytes,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value")]
pub enum DefaultValueManifest {
    Bool(bool),
    Integer(i64),
    String(String),
    Json(serde_json::Value),
    CurrentTimestamp,
    CurrentTimestampPlusSeconds(i64),
    Expression(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ColumnManifest {
    pub name: String,
    pub family: ColumnTypeFamily,
    pub nullable: bool,
    pub default: Option<DefaultValueManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexManifest {
    pub name: String,
    pub unique: bool,
    pub columns: Vec<String>,
    pub predicate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UniqueKeyManifest {
    pub columns: Vec<String>,
    pub predicate: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckConstraintManifest {
    pub expression: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ForeignKeyDeleteAction {
    NoAction,
    Restrict,
    Cascade,
    SetNull,
    SetDefault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForeignKeyManifest {
    pub columns: Vec<String>,
    pub referenced_table: String,
    pub referenced_columns: Vec<String>,
    pub on_delete: ForeignKeyDeleteAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableManifest {
    pub name: String,
    pub columns: Vec<ColumnManifest>,
    pub primary_key: Vec<String>,
    pub indexes: Vec<IndexManifest>,
    pub unique_keys: Vec<UniqueKeyManifest>,
    pub foreign_keys: Vec<ForeignKeyManifest>,
    pub checks: Vec<CheckConstraintManifest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaManifest {
    pub tables: Vec<TableManifest>,
}

type SqliteForeignKeyPragmaRow = (i64, i64, String, String, String, String, String, String);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendSchemaCapabilities {
    pub native_transport: bool,
    pub transactional_ddl: bool,
    pub named_secondary_index_prefix: &'static str,
}

const SQLITE_SCHEMA_CAPABILITIES: BackendSchemaCapabilities = BackendSchemaCapabilities {
    native_transport: false,
    transactional_ddl: false,
    named_secondary_index_prefix: "idx_",
};

const POSTGRES_SCHEMA_CAPABILITIES: BackendSchemaCapabilities = BackendSchemaCapabilities {
    native_transport: false,
    transactional_ddl: true,
    named_secondary_index_prefix: "idx_",
};

const SPANNER_SCHEMA_CAPABILITIES: BackendSchemaCapabilities = BackendSchemaCapabilities {
    native_transport: true,
    transactional_ddl: false,
    named_secondary_index_prefix: "idx_",
};

pub fn canonical_manifest() -> &'static SchemaManifest {
    static CANONICAL_MANIFEST: OnceLock<SchemaManifest> = OnceLock::new();
    CANONICAL_MANIFEST.get_or_init(|| {
        serde_json::from_str(CANONICAL_SCHEMA_MANIFEST_JSON)
            .expect("canonical schema manifest must deserialize")
    })
}

pub const fn backend_capabilities(backend: BackendKind) -> BackendSchemaCapabilities {
    match backend {
        BackendKind::Sqlite => SQLITE_SCHEMA_CAPABILITIES,
        BackendKind::Postgres => POSTGRES_SCHEMA_CAPABILITIES,
        BackendKind::Spanner => SPANNER_SCHEMA_CAPABILITIES,
    }
}

pub fn render_baseline_migration(backend: BackendKind) -> String {
    let mut rendered = String::from("-- +goose Up\n\n");
    rendered.push_str(&render_schema_ddl(canonical_manifest(), backend));
    if !rendered.ends_with('\n') {
        rendered.push('\n');
    }
    rendered
}

pub fn embedded_migrations(backend: BackendKind) -> &'static [(&'static str, String)] {
    static SQLITE_MIGRATIONS: OnceLock<Vec<(&'static str, String)>> = OnceLock::new();
    static POSTGRES_MIGRATIONS: OnceLock<Vec<(&'static str, String)>> = OnceLock::new();
    static SPANNER_MIGRATIONS: OnceLock<Vec<(&'static str, String)>> = OnceLock::new();

    match backend {
        BackendKind::Sqlite => SQLITE_MIGRATIONS.get_or_init(|| {
            vec![(
                "00001_baseline",
                render_baseline_migration(BackendKind::Sqlite),
            )]
        }),
        BackendKind::Postgres => POSTGRES_MIGRATIONS.get_or_init(|| {
            vec![(
                "00001_baseline",
                render_baseline_migration(BackendKind::Postgres),
            )]
        }),
        BackendKind::Spanner => SPANNER_MIGRATIONS.get_or_init(|| {
            vec![(
                "00001_baseline",
                render_baseline_migration(BackendKind::Spanner),
            )]
        }),
    }
}

pub fn embedded_baseline_sql(backend: BackendKind) -> &'static str {
    embedded_migrations(backend)[0].1.as_str()
}

fn render_schema_ddl(manifest: &SchemaManifest, backend: BackendKind) -> String {
    let mut ddl = String::new();

    for (position, table) in manifest.tables.iter().enumerate() {
        if position > 0 {
            ddl.push('\n');
        }
        ddl.push_str(&render_create_table(table, backend));
        ddl.push('\n');
    }

    let mut trailing = Vec::new();
    for table in &manifest.tables {
        if backend != BackendKind::Sqlite {
            trailing.extend(render_foreign_key_statements(table, backend));
        }
        trailing.extend(render_unique_key_statements(table, backend));
        trailing.extend(render_index_statements(table, backend));
    }

    if !trailing.is_empty() {
        ddl.push('\n');
        ddl.push_str(&trailing.join("\n\n"));
        ddl.push('\n');
    }

    ddl
}

fn render_create_table(table: &TableManifest, backend: BackendKind) -> String {
    let inline_identity_primary_key = if table.primary_key.len() == 1 {
        let column_name = table.primary_key[0].as_str();
        if is_identity_primary_key(table, column_name) {
            Some(column_name)
        } else {
            None
        }
    } else {
        None
    };

    let mut lines = Vec::new();
    for column in &table.columns {
        lines.push(format!(
            "    {}",
            render_column_definition(
                table,
                column,
                backend,
                inline_identity_primary_key == Some(column.name.as_str()),
            )
        ));
    }

    if backend == BackendKind::Spanner {
        for helper in render_spanner_partial_helper_columns(table) {
            lines.push(format!("    {helper}"));
        }
    }

    if inline_identity_primary_key.is_none() && !table.primary_key.is_empty() {
        lines.push(format!(
            "    PRIMARY KEY ({})",
            render_identifier_list(&table.primary_key, backend)
        ));
    }

    for check in &table.checks {
        lines.push(format!(
            "    CHECK ({})",
            render_expression(table, &check.expression, backend)
        ));
    }

    if backend == BackendKind::Sqlite {
        for foreign_key in &table.foreign_keys {
            lines.push(format!(
                "    FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE {}",
                render_identifier_list(&foreign_key.columns, backend),
                render_table_name(&foreign_key.referenced_table, backend),
                render_identifier_list(&foreign_key.referenced_columns, backend),
                render_delete_action(foreign_key.on_delete, backend),
            ));
        }
    }

    format!(
        "CREATE TABLE IF NOT EXISTS {} (\n{}\n);",
        render_table_name(&table.name, backend),
        lines.join(",\n")
    )
}

fn render_column_definition(
    table: &TableManifest,
    column: &ColumnManifest,
    backend: BackendKind,
    inline_identity_primary_key: bool,
) -> String {
    let mut rendered = format!(
        "{} {}",
        render_identifier(&column.name, backend),
        render_column_type(column, backend, inline_identity_primary_key)
    );

    if inline_identity_primary_key {
        match backend {
            BackendKind::Sqlite => rendered.push_str(" PRIMARY KEY AUTOINCREMENT"),
            BackendKind::Postgres => {
                rendered.push_str(" GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY")
            }
            BackendKind::Spanner => rendered
                .push_str(" GENERATED BY DEFAULT AS IDENTITY (BIT_REVERSED_POSITIVE) PRIMARY KEY"),
        }
        return rendered;
    }

    if !column.nullable {
        rendered.push_str(" NOT NULL");
    }

    if let Some(default) = render_default_value(table, column, backend) {
        rendered.push(' ');
        rendered.push_str(&default);
    }

    rendered
}

fn render_column_type(
    column: &ColumnManifest,
    backend: BackendKind,
    inline_identity_primary_key: bool,
) -> &'static str {
    match backend {
        BackendKind::Sqlite => {
            if inline_identity_primary_key {
                "INTEGER"
            } else {
                match column.family {
                    ColumnTypeFamily::Integer => "INTEGER",
                    ColumnTypeFamily::Float => "REAL",
                    ColumnTypeFamily::Bool => "BOOLEAN",
                    ColumnTypeFamily::Bytes => "BLOB",
                    ColumnTypeFamily::Json
                    | ColumnTypeFamily::String
                    | ColumnTypeFamily::Timestamp
                    | ColumnTypeFamily::Unknown => "TEXT",
                }
            }
        }
        BackendKind::Postgres => {
            if inline_identity_primary_key {
                "BIGINT"
            } else {
                match column.family {
                    ColumnTypeFamily::Integer => "INTEGER",
                    ColumnTypeFamily::Float => "DOUBLE PRECISION",
                    ColumnTypeFamily::Bool => "BOOLEAN",
                    ColumnTypeFamily::Bytes => "BYTEA",
                    ColumnTypeFamily::Json => "JSONB",
                    ColumnTypeFamily::Timestamp => "TIMESTAMPTZ",
                    ColumnTypeFamily::String | ColumnTypeFamily::Unknown => "TEXT",
                }
            }
        }
        BackendKind::Spanner => {
            if inline_identity_primary_key {
                "INT64"
            } else {
                match column.family {
                    ColumnTypeFamily::Integer => "INT64",
                    ColumnTypeFamily::Float => "FLOAT64",
                    ColumnTypeFamily::Bool => "BOOL",
                    ColumnTypeFamily::Bytes => "BYTES(MAX)",
                    ColumnTypeFamily::Json
                    | ColumnTypeFamily::String
                    | ColumnTypeFamily::Unknown => "STRING(MAX)",
                    ColumnTypeFamily::Timestamp => "TIMESTAMP",
                }
            }
        }
    }
}

fn render_default_value(
    table: &TableManifest,
    column: &ColumnManifest,
    backend: BackendKind,
) -> Option<String> {
    let default = column.default.as_ref()?;
    Some(match default {
        DefaultValueManifest::Bool(value) => match backend {
            BackendKind::Sqlite => format!("DEFAULT {}", if *value { 1 } else { 0 }),
            BackendKind::Postgres => format!("DEFAULT {}", if *value { "TRUE" } else { "FALSE" }),
            BackendKind::Spanner => {
                format!("DEFAULT ({})", if *value { "TRUE" } else { "FALSE" })
            }
        },
        DefaultValueManifest::Integer(value) => match backend {
            BackendKind::Spanner => format!("DEFAULT ({value})"),
            _ => format!("DEFAULT {value}"),
        },
        DefaultValueManifest::String(value) => match backend {
            BackendKind::Spanner => format!("DEFAULT ({})", render_sql_string(value)),
            _ => format!("DEFAULT {}", render_sql_string(value)),
        },
        DefaultValueManifest::Json(value) => {
            let json = serde_json::to_string(value).expect("json default must serialize");
            match backend {
                BackendKind::Sqlite => format!("DEFAULT {}", render_sql_string(&json)),
                BackendKind::Postgres => {
                    format!("DEFAULT {}::jsonb", render_sql_string(&json))
                }
                BackendKind::Spanner => format!("DEFAULT ({})", render_sql_string(&json)),
            }
        }
        DefaultValueManifest::CurrentTimestamp => match backend {
            BackendKind::Sqlite => "DEFAULT (datetime('now'))".to_string(),
            BackendKind::Postgres => "DEFAULT NOW()".to_string(),
            BackendKind::Spanner => "DEFAULT (CURRENT_TIMESTAMP())".to_string(),
        },
        DefaultValueManifest::CurrentTimestampPlusSeconds(seconds) => match backend {
            BackendKind::Sqlite => {
                if *seconds % 60 == 0 {
                    let minutes = seconds / 60;
                    format!(
                        "DEFAULT (datetime('now', '{}{} minutes'))",
                        if minutes >= 0 { "+" } else { "" },
                        minutes
                    )
                } else {
                    format!(
                        "DEFAULT (datetime('now', '{}{} seconds'))",
                        if *seconds >= 0 { "+" } else { "" },
                        seconds
                    )
                }
            }
            BackendKind::Postgres => {
                if *seconds % 60 == 0 {
                    let minutes = seconds / 60;
                    format!(
                        "DEFAULT (NOW() + INTERVAL '{}{} minutes')",
                        if minutes >= 0 { "+" } else { "" },
                        minutes
                    )
                } else {
                    format!(
                        "DEFAULT (NOW() + INTERVAL '{}{} seconds')",
                        if *seconds >= 0 { "+" } else { "" },
                        seconds
                    )
                }
            }
            BackendKind::Spanner => {
                if *seconds % 60 == 0 {
                    let minutes = seconds / 60;
                    format!(
                        "DEFAULT (TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL {minutes} MINUTE))"
                    )
                } else {
                    format!(
                        "DEFAULT (TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL {seconds} SECOND))"
                    )
                }
            }
        },
        DefaultValueManifest::Expression(expression) => {
            let rendered = render_expression(table, strip_wrapping_parens(expression), backend);
            format!("DEFAULT ({rendered})")
        }
    })
}

fn render_foreign_key_statements(table: &TableManifest, backend: BackendKind) -> Vec<String> {
    table
        .foreign_keys
        .iter()
        .enumerate()
        .map(|(position, foreign_key)| {
            format!(
                "ALTER TABLE {} ADD CONSTRAINT {} FOREIGN KEY ({}) REFERENCES {}({}) ON DELETE {};",
                render_table_name(&table.name, backend),
                derived_foreign_key_name(&table.name, foreign_key, position),
                render_identifier_list(&foreign_key.columns, backend),
                render_table_name(&foreign_key.referenced_table, backend),
                render_identifier_list(&foreign_key.referenced_columns, backend),
                render_foreign_key_delete_action(table, foreign_key, backend),
            )
        })
        .collect()
}

fn render_unique_key_statements(table: &TableManifest, backend: BackendKind) -> Vec<String> {
    table
        .unique_keys
        .iter()
        .filter(|unique_key| unique_key_backing_index(table, unique_key).is_none())
        .map(|unique_key| {
            render_index_statement(
                table,
                &IndexManifest {
                    name: derived_unique_key_name(&table.name, unique_key),
                    unique: true,
                    columns: unique_key.columns.clone(),
                    predicate: unique_key.predicate.clone(),
                },
                backend,
            )
        })
        .collect()
}

fn render_index_statements(table: &TableManifest, backend: BackendKind) -> Vec<String> {
    table
        .indexes
        .iter()
        .map(|index| render_index_statement(table, index, backend))
        .collect()
}

fn render_index_statement(
    table: &TableManifest,
    index: &IndexManifest,
    backend: BackendKind,
) -> String {
    match backend {
        BackendKind::Sqlite | BackendKind::Postgres => {
            let mut rendered = format!(
                "CREATE {}INDEX IF NOT EXISTS {} ON {}({})",
                if index.unique { "UNIQUE " } else { "" },
                index.name,
                render_table_name(&table.name, backend),
                render_identifier_list(&index.columns, backend),
            );
            if let Some(predicate) = &index.predicate {
                write!(
                    rendered,
                    " WHERE {}",
                    render_expression(table, predicate, backend)
                )
                .expect("write to string");
            }
            rendered.push(';');
            rendered
        }
        BackendKind::Spanner => render_spanner_index_statement(table, index),
    }
}

fn render_spanner_index_statement(table: &TableManifest, index: &IndexManifest) -> String {
    let mut physical_columns = index
        .columns
        .iter()
        .filter(|column| {
            index
                .predicate
                .as_deref()
                .map(|predicate| !predicate_guarantees_null(predicate, column))
                .unwrap_or(true)
        })
        .map(|column| render_identifier(column, BackendKind::Spanner))
        .collect::<Vec<_>>();

    let null_filtered = index.predicate.is_some();
    if null_filtered {
        physical_columns.push(render_identifier(
            &spanner_partial_marker_name(&index.name),
            BackendKind::Spanner,
        ));
    }
    if physical_columns.is_empty() {
        physical_columns.push(render_identifier(
            &spanner_partial_marker_name(&index.name),
            BackendKind::Spanner,
        ));
    }

    format!(
        "CREATE {}{}INDEX IF NOT EXISTS {} ON {}({});",
        if index.unique { "UNIQUE " } else { "" },
        if null_filtered { "NULL_FILTERED " } else { "" },
        index.name,
        render_table_name(&table.name, BackendKind::Spanner),
        physical_columns.join(", "),
    )
}

fn render_spanner_partial_helper_columns(table: &TableManifest) -> Vec<String> {
    let mut helpers = Vec::new();
    for index in &table.indexes {
        if let Some(predicate) = &index.predicate {
            helpers.push(render_spanner_partial_helper_column(
                table,
                &index.name,
                predicate,
            ));
        }
    }
    for unique_key in table
        .unique_keys
        .iter()
        .filter(|unique_key| unique_key_backing_index(table, unique_key).is_none())
    {
        if let Some(predicate) = &unique_key.predicate {
            helpers.push(render_spanner_partial_helper_column(
                table,
                &derived_unique_key_name(&table.name, unique_key),
                predicate,
            ));
        }
    }
    helpers.sort();
    helpers.dedup();
    helpers
}

fn render_spanner_partial_helper_column(
    table: &TableManifest,
    index_name: &str,
    predicate: &str,
) -> String {
    format!(
        "{} BOOL AS (IF({}, TRUE, NULL)) STORED",
        render_identifier(
            &spanner_partial_marker_name(index_name),
            BackendKind::Spanner
        ),
        render_expression(table, predicate, BackendKind::Spanner),
    )
}

fn render_expression(table: &TableManifest, expression: &str, backend: BackendKind) -> String {
    let mut rendered = expression.to_string();
    for column in table
        .columns
        .iter()
        .filter(|column| column.family == ColumnTypeFamily::Bool)
    {
        rendered = rendered.replace(
            &format!("{} = 1", column.name),
            &format!(
                "{} = {}",
                column.name,
                if backend == BackendKind::Sqlite {
                    "1"
                } else {
                    "TRUE"
                }
            ),
        );
        rendered = rendered.replace(
            &format!("{} = 0", column.name),
            &format!(
                "{} = {}",
                column.name,
                if backend == BackendKind::Sqlite {
                    "0"
                } else {
                    "FALSE"
                }
            ),
        );
    }
    rendered
}

fn render_identifier(name: &str, backend: BackendKind) -> String {
    if backend == BackendKind::Spanner && matches!(name, "groups") {
        format!("`{name}`")
    } else {
        name.to_string()
    }
}

fn render_table_name(name: &str, backend: BackendKind) -> String {
    render_identifier(name, backend)
}

fn render_identifier_list(columns: &[String], backend: BackendKind) -> String {
    columns
        .iter()
        .map(|column| render_identifier(column, backend))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_delete_action(action: ForeignKeyDeleteAction, backend: BackendKind) -> &'static str {
    match action {
        ForeignKeyDeleteAction::Cascade => "CASCADE",
        ForeignKeyDeleteAction::SetNull if backend == BackendKind::Spanner => "NO ACTION",
        ForeignKeyDeleteAction::SetNull => "SET NULL",
        ForeignKeyDeleteAction::SetDefault if backend == BackendKind::Spanner => "NO ACTION",
        ForeignKeyDeleteAction::SetDefault => "SET DEFAULT",
        ForeignKeyDeleteAction::Restrict if backend == BackendKind::Spanner => "NO ACTION",
        ForeignKeyDeleteAction::Restrict => "RESTRICT",
        ForeignKeyDeleteAction::NoAction => "NO ACTION",
    }
}

fn render_foreign_key_delete_action(
    table: &TableManifest,
    foreign_key: &ForeignKeyManifest,
    backend: BackendKind,
) -> &'static str {
    if backend == BackendKind::Spanner
        && table.name == foreign_key.referenced_table
        && foreign_key.on_delete == ForeignKeyDeleteAction::Cascade
    {
        "NO ACTION"
    } else {
        render_delete_action(foreign_key.on_delete, backend)
    }
}

fn render_sql_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn unique_key_backing_index<'a>(
    table: &'a TableManifest,
    unique_key: &UniqueKeyManifest,
) -> Option<&'a IndexManifest> {
    table.indexes.iter().find(|index| {
        index.unique
            && index.columns == unique_key.columns
            && index.predicate == unique_key.predicate
    })
}

fn derived_unique_key_name(table_name: &str, unique_key: &UniqueKeyManifest) -> String {
    let identity = format!(
        "{table_name}|{}|{}",
        unique_key.columns.join(","),
        unique_key.predicate.as_deref().unwrap_or(""),
    );
    format!(
        "{UNIQUE_INDEX_PREFIX}{}_{}",
        table_name,
        stable_name_hash(&identity)
    )
}

fn expected_unique_key_index_name(table: &TableManifest, unique_key: &UniqueKeyManifest) -> String {
    unique_key_backing_index(table, unique_key)
        .map(|index| index.name.clone())
        .unwrap_or_else(|| derived_unique_key_name(&table.name, unique_key))
}

fn derived_foreign_key_name(
    table_name: &str,
    foreign_key: &ForeignKeyManifest,
    position: usize,
) -> String {
    let identity = format!(
        "{table_name}|{}|{}|{}|{position}",
        foreign_key.columns.join(","),
        foreign_key.referenced_table,
        foreign_key.referenced_columns.join(","),
    );
    format!("fk_{table_name}_{}", stable_name_hash(&identity))
}

fn spanner_partial_marker_name(index_name: &str) -> String {
    format!("{SPANNER_HELPER_PREFIX}{}_m", stable_name_hash(index_name))
}

fn stable_name_hash(input: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn predicate_guarantees_null(predicate: &str, column_name: &str) -> bool {
    predicate.contains(&format!("{column_name} IS NULL"))
}

fn is_identity_primary_key(table: &TableManifest, column_name: &str) -> bool {
    table.name == "fga_tuple_changes" && column_name == "seq"
}

fn canonical_table_manifest(table_name: &str) -> Option<&'static TableManifest> {
    canonical_manifest()
        .tables
        .iter()
        .find(|table| table.name == table_name)
}

pub async fn inspect_schema(db: &Db) -> anyhow::Result<SchemaManifest> {
    let (spanner_column_defaults, spanner_foreign_keys, spanner_checks) = match db.dialect() {
        Dialect::Spanner => {
            let ddl = db
                .spanner()
                .expect("schema inspection requires native Spanner client")
                .current_ddl()
                .await?;
            (
                Some(parse_spanner_column_defaults(&ddl)),
                Some(parse_spanner_foreign_keys(&ddl)),
                Some(parse_spanner_checks(&ddl)),
            )
        }
        _ => (None, None, None),
    };

    let mut tables = list_tables(db).await?;
    tables.retain(|table| table != "_schema_version");

    let mut manifests = Vec::with_capacity(tables.len());
    for table_name in tables {
        manifests.push(TableManifest {
            name: table_name.clone(),
            columns: list_columns(db, &table_name, spanner_column_defaults.as_ref()).await?,
            primary_key: list_primary_key_columns(db, &table_name).await?,
            indexes: list_named_secondary_indexes(db, &table_name).await?,
            unique_keys: list_unique_keys(db, &table_name).await?,
            foreign_keys: list_foreign_keys(db, &table_name, spanner_foreign_keys.as_ref()).await?,
            checks: list_check_constraints(db, &table_name, spanner_checks.as_ref()).await?,
        });
    }

    manifests.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(SchemaManifest { tables: manifests })
}

fn normalize_type_family(raw: &str) -> ColumnTypeFamily {
    let upper = raw.trim().to_uppercase();
    if upper.is_empty() {
        return ColumnTypeFamily::Unknown;
    }
    if upper.contains("JSON") {
        return ColumnTypeFamily::Json;
    }
    if upper.contains("TIMESTAMP") || upper.contains("DATETIME") {
        return ColumnTypeFamily::Timestamp;
    }
    if upper.contains("BOOL") {
        return ColumnTypeFamily::Bool;
    }
    if upper.contains("INT") {
        return ColumnTypeFamily::Integer;
    }
    if upper.contains("REAL")
        || upper.contains("FLOAT")
        || upper.contains("DOUBLE")
        || upper.contains("NUMERIC")
        || upper.contains("DECIMAL")
    {
        return ColumnTypeFamily::Float;
    }
    if upper.contains("BYTEA") || upper.contains("BYTES") || upper.contains("BLOB") {
        return ColumnTypeFamily::Bytes;
    }
    if upper.contains("CHAR")
        || upper.contains("TEXT")
        || upper.starts_with("STRING")
        || upper.starts_with("VARCHAR")
    {
        return ColumnTypeFamily::String;
    }
    ColumnTypeFamily::Unknown
}

fn logical_type_family(column_name: &str, raw: &str) -> ColumnTypeFamily {
    if JSON_COLUMN_NAMES.contains(&column_name) {
        return ColumnTypeFamily::Json;
    }
    if TIMESTAMP_COLUMN_NAMES.contains(&column_name) {
        return ColumnTypeFamily::Timestamp;
    }
    normalize_type_family(raw)
}

fn normalize_default_value(
    family: ColumnTypeFamily,
    raw_default: Option<&str>,
) -> Option<DefaultValueManifest> {
    let raw_default = raw_default?.trim();
    if raw_default.is_empty() {
        return None;
    }

    let normalized = strip_postgres_casts(strip_wrapping_parens(raw_default)).trim();
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("NULL") {
        return None;
    }

    if matches_current_timestamp(normalized) {
        return Some(DefaultValueManifest::CurrentTimestamp);
    }
    if let Some(seconds) = parse_current_timestamp_offset_seconds(normalized) {
        return Some(DefaultValueManifest::CurrentTimestampPlusSeconds(seconds));
    }

    match family {
        ColumnTypeFamily::Bool => match normalized.to_ascii_uppercase().as_str() {
            "TRUE" | "1" => Some(DefaultValueManifest::Bool(true)),
            "FALSE" | "0" => Some(DefaultValueManifest::Bool(false)),
            _ => Some(DefaultValueManifest::Expression(normalized.to_string())),
        },
        ColumnTypeFamily::Integer => normalized
            .parse::<i64>()
            .map(DefaultValueManifest::Integer)
            .ok()
            .or_else(|| Some(DefaultValueManifest::Expression(normalized.to_string()))),
        ColumnTypeFamily::Json => {
            let json_raw = unquote_sql_string(normalized).unwrap_or_else(|| normalized.to_string());
            serde_json::from_str::<serde_json::Value>(&json_raw)
                .map(DefaultValueManifest::Json)
                .ok()
                .or_else(|| Some(DefaultValueManifest::Expression(normalized.to_string())))
        }
        ColumnTypeFamily::String => Some(DefaultValueManifest::String(
            unquote_sql_string(normalized).unwrap_or_else(|| normalized.to_string()),
        )),
        _ => Some(DefaultValueManifest::Expression(normalized.to_string())),
    }
}

fn strip_postgres_casts(mut raw: &str) -> &str {
    loop {
        let Some((before, after)) = raw.rsplit_once("::") else {
            return raw;
        };
        if after
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '[' | ']'))
        {
            raw = before.trim_end();
            continue;
        }
        return raw;
    }
}

fn strip_wrapping_parens(mut raw: &str) -> &str {
    loop {
        let trimmed = raw.trim();
        if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
            return trimmed;
        }
        if !outer_parens_wrap_all(trimmed) {
            return trimmed;
        }
        raw = &trimmed[1..trimmed.len() - 1];
    }
}

fn outer_parens_wrap_all(raw: &str) -> bool {
    let mut depth = 0i64;
    let mut in_string = false;
    let chars = raw.chars().collect::<Vec<_>>();
    let mut idx = 0usize;
    while idx < chars.len() {
        let ch = chars[idx];
        if ch == '\'' {
            if in_string && chars.get(idx + 1) == Some(&'\'') {
                idx += 2;
                continue;
            }
            in_string = !in_string;
        } else if !in_string {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
                if depth == 0 && idx + 1 != chars.len() {
                    return false;
                }
            }
        }
        idx += 1;
    }
    depth == 0
}

fn unquote_sql_string(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('\'') && trimmed.ends_with('\'') {
        Some(trimmed[1..trimmed.len() - 1].replace("''", "'"))
    } else {
        None
    }
}

fn matches_current_timestamp(raw: &str) -> bool {
    matches!(
        raw.to_ascii_uppercase().replace(' ', "").as_str(),
        "CURRENT_TIMESTAMP" | "CURRENT_TIMESTAMP()" | "NOW()" | "DATETIME('NOW')"
    )
}

fn parse_current_timestamp_offset_seconds(raw: &str) -> Option<i64> {
    let compact = raw.to_ascii_uppercase().replace(' ', "");

    if let Some(minutes) = compact
        .strip_prefix("DATETIME('NOW','+")
        .and_then(|rest| rest.strip_suffix("MINUTES')"))
        .or_else(|| {
            compact
                .strip_prefix("DATETIME('NOW','+")
                .and_then(|rest| rest.strip_suffix("MINUTE')"))
        })
        .and_then(|value| value.parse::<i64>().ok())
    {
        return Some(minutes * 60);
    }

    if let Some(seconds) = compact
        .strip_prefix("DATETIME('NOW','+")
        .and_then(|rest| rest.strip_suffix("SECONDS')"))
        .or_else(|| {
            compact
                .strip_prefix("DATETIME('NOW','+")
                .and_then(|rest| rest.strip_suffix("SECOND')"))
        })
        .and_then(|value| value.parse::<i64>().ok())
    {
        return Some(seconds);
    }

    if let Some(minutes) = compact
        .strip_prefix("NOW()+INTERVAL'")
        .and_then(|rest| rest.strip_suffix("MINUTES'"))
        .or_else(|| {
            compact
                .strip_prefix("NOW()+INTERVAL'")
                .and_then(|rest| rest.strip_suffix("MINUTE'"))
        })
        .and_then(|value| value.parse::<i64>().ok())
    {
        return Some(minutes * 60);
    }

    if let Some(seconds) = compact
        .strip_prefix("NOW()+INTERVAL'")
        .and_then(|rest| rest.strip_suffix("SECONDS'"))
        .or_else(|| {
            compact
                .strip_prefix("NOW()+INTERVAL'")
                .and_then(|rest| rest.strip_suffix("SECOND'"))
        })
        .and_then(|value| value.parse::<i64>().ok())
    {
        return Some(seconds);
    }

    if let Some(minutes) = compact
        .strip_prefix("TIMESTAMP_ADD(CURRENT_TIMESTAMP(),INTERVAL")
        .and_then(|rest| rest.strip_suffix("MINUTE)"))
        .or_else(|| {
            compact
                .strip_prefix("TIMESTAMP_ADD(CURRENT_TIMESTAMP(),INTERVAL")
                .and_then(|rest| rest.strip_suffix("MINUTES)"))
        })
        .and_then(|value| value.parse::<i64>().ok())
    {
        return Some(minutes * 60);
    }

    if let Some(seconds) = compact
        .strip_prefix("TIMESTAMP_ADD(CURRENT_TIMESTAMP(),INTERVAL")
        .and_then(|rest| rest.strip_suffix("SECOND)"))
        .or_else(|| {
            compact
                .strip_prefix("TIMESTAMP_ADD(CURRENT_TIMESTAMP(),INTERVAL")
                .and_then(|rest| rest.strip_suffix("SECONDS)"))
        })
        .and_then(|value| value.parse::<i64>().ok())
    {
        return Some(seconds);
    }

    None
}

fn parse_spanner_column_defaults(
    statements: &[String],
) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut tables = BTreeMap::<String, BTreeMap<String, String>>::new();

    for statement in statements {
        let trimmed = statement.trim();
        if !trimmed.to_ascii_uppercase().starts_with("CREATE TABLE") {
            continue;
        }

        let Some((header, body)) = trimmed.split_once('(') else {
            continue;
        };
        let Some(table_name) = header.split_whitespace().last() else {
            continue;
        };
        let columns = tables
            .entry(table_name.trim_matches('`').to_string())
            .or_default();

        for line in body.lines() {
            let line = line.trim().trim_end_matches(',').trim();
            if line.is_empty()
                || line == ")"
                || line.starts_with("PRIMARY KEY")
                || line.starts_with("FOREIGN KEY")
            {
                continue;
            }

            let Some(column_name) = line.split_whitespace().next() else {
                continue;
            };
            let Some((_, default_raw)) = line.split_once(" DEFAULT ") else {
                continue;
            };
            columns.insert(
                column_name.trim_matches('`').to_string(),
                default_raw.trim().to_string(),
            );
        }
    }

    tables
}

fn parse_spanner_foreign_keys(statements: &[String]) -> BTreeMap<String, Vec<ForeignKeyManifest>> {
    let mut tables = BTreeMap::<String, Vec<ForeignKeyManifest>>::new();

    for statement in statements {
        let trimmed = statement.trim();
        if !trimmed.to_ascii_uppercase().starts_with("CREATE TABLE") {
            continue;
        }

        let Some((header, body)) = trimmed.split_once('(') else {
            continue;
        };
        let Some(table_name) = header.split_whitespace().last() else {
            continue;
        };
        let table_name = table_name.trim_matches('`').to_string();
        let foreign_keys = tables.entry(table_name).or_default();

        for line in body.lines() {
            let line = line.trim().trim_end_matches(',').trim();
            if !line.starts_with("FOREIGN KEY") {
                continue;
            }

            let Some((left, right)) = line.split_once("REFERENCES") else {
                continue;
            };
            let Some(local_columns_raw) = left
                .strip_prefix("FOREIGN KEY")
                .map(str::trim)
                .and_then(|rest| rest.strip_prefix('('))
                .and_then(|rest| rest.split_once(')'))
                .map(|(cols, _)| cols)
            else {
                continue;
            };

            let referenced = right.trim();
            let Some((referenced_table_raw, referenced_rest)) = referenced.split_once('(') else {
                continue;
            };
            let Some((referenced_columns_raw, after_columns)) = referenced_rest.split_once(')')
            else {
                continue;
            };

            let on_delete = after_columns
                .split_once("ON DELETE")
                .map(|(_, action)| normalize_delete_action(action.trim()))
                .unwrap_or(ForeignKeyDeleteAction::NoAction);

            foreign_keys.push(ForeignKeyManifest {
                columns: parse_identifier_list(local_columns_raw),
                referenced_table: referenced_table_raw.trim().trim_matches('`').to_string(),
                referenced_columns: parse_identifier_list(referenced_columns_raw),
                on_delete,
            });
        }
    }

    for foreign_keys in tables.values_mut() {
        foreign_keys.sort_by(|left, right| {
            left.columns
                .cmp(&right.columns)
                .then(left.referenced_table.cmp(&right.referenced_table))
        });
    }

    tables
}

fn parse_spanner_checks(statements: &[String]) -> BTreeMap<String, Vec<CheckConstraintManifest>> {
    let mut tables = BTreeMap::<String, Vec<CheckConstraintManifest>>::new();

    for statement in statements {
        let trimmed = statement.trim();
        if !trimmed.to_ascii_uppercase().starts_with("CREATE TABLE") {
            continue;
        }

        let Some((header, _)) = trimmed.split_once('(') else {
            continue;
        };
        let Some(table_name) = header.split_whitespace().last() else {
            continue;
        };
        let checks = extract_check_expressions(trimmed)
            .into_iter()
            .map(|expression| CheckConstraintManifest {
                expression: normalize_predicate(&expression),
            })
            .collect::<Vec<_>>();
        tables.insert(table_name.trim_matches('`').to_string(), checks);
    }

    tables
}

fn parse_identifier_list(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|value| value.trim().trim_matches('`').to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_delete_action(raw: &str) -> ForeignKeyDeleteAction {
    match raw.trim().to_ascii_uppercase().as_str() {
        "CASCADE" => ForeignKeyDeleteAction::Cascade,
        "RESTRICT" => ForeignKeyDeleteAction::Restrict,
        "SET NULL" => ForeignKeyDeleteAction::SetNull,
        "SET DEFAULT" => ForeignKeyDeleteAction::SetDefault,
        _ => ForeignKeyDeleteAction::NoAction,
    }
}

fn normalize_postgres_delete_action(raw: &str) -> ForeignKeyDeleteAction {
    match raw.trim() {
        "c" => ForeignKeyDeleteAction::Cascade,
        "r" => ForeignKeyDeleteAction::Restrict,
        "n" => ForeignKeyDeleteAction::SetNull,
        "d" => ForeignKeyDeleteAction::SetDefault,
        _ => ForeignKeyDeleteAction::NoAction,
    }
}

fn normalize_predicate(raw: &str) -> String {
    let mut normalized = raw.trim().replace("::text", "");
    normalized = normalized.replace("::character varying", "");
    normalized = normalized.replace("<>", "!=");
    normalized = strip_wrapping_parens(&normalized).trim().to_string();
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_check_expressions(sql: &str) -> Vec<String> {
    let upper = sql.to_ascii_uppercase();
    let upper_bytes = upper.as_bytes();
    let bytes = sql.as_bytes();
    let mut expressions = Vec::new();
    let mut idx = 0usize;

    while idx + 5 <= upper_bytes.len() {
        if &upper_bytes[idx..idx + 5] == b"CHECK" {
            let mut cursor = idx + 5;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            if cursor >= bytes.len() || bytes[cursor] != b'(' {
                idx += 5;
                continue;
            }
            if let Some((end, expression)) = capture_parenthesized(sql, cursor) {
                expressions.push(expression);
                idx = end;
                continue;
            }
        }
        idx += 1;
    }

    expressions
}

fn capture_parenthesized(sql: &str, open_idx: usize) -> Option<(usize, String)> {
    let bytes = sql.as_bytes();
    let mut depth = 0i64;
    let mut in_string = false;
    let mut idx = open_idx;
    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch == '\'' {
            if in_string && idx + 1 < bytes.len() && bytes[idx + 1] as char == '\'' {
                idx += 2;
                continue;
            }
            in_string = !in_string;
        } else if !in_string {
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                depth -= 1;
                if depth == 0 {
                    return Some((idx + 1, sql[open_idx + 1..idx].to_string()));
                }
            }
        }
        idx += 1;
    }
    None
}

async fn sqlite_index_predicate(db: &Db, index_name: &str) -> anyhow::Result<Option<String>> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT sql FROM sqlite_master WHERE type = 'index' AND name = $1")
            .bind(index_name)
            .fetch_optional(db.pool())
            .await?;
    Ok(row
        .and_then(|(sql,)| sql)
        .and_then(|sql| extract_where_clause(&sql))
        .map(|predicate| normalize_predicate(&predicate)))
}

async fn sqlite_index_columns(db: &Db, index_name: &str) -> anyhow::Result<Vec<String>> {
    let info_sql = format!("PRAGMA index_info('{}')", index_name.replace('\'', "''"));
    let mut info_rows: Vec<(i64, i64, String)> =
        sqlx::query_as(&info_sql).fetch_all(db.pool()).await?;
    info_rows.sort_by_key(|(position, _, _)| *position);
    Ok(info_rows
        .into_iter()
        .map(|(_, _, column_name)| column_name)
        .collect())
}

fn extract_where_clause(sql: &str) -> Option<String> {
    let upper = sql.to_ascii_uppercase();
    upper
        .find(" WHERE ")
        .map(|idx| sql[idx + 7..].trim().trim_end_matches(';').to_string())
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
                .expect("schema inspection requires native Spanner client")
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

async fn list_columns(
    db: &Db,
    table_name: &str,
    spanner_column_defaults: Option<&BTreeMap<String, BTreeMap<String, String>>>,
) -> anyhow::Result<Vec<ColumnManifest>> {
    let mut columns = match db.dialect() {
        Dialect::Sqlite => {
            let pragma_sql = format!("PRAGMA table_info('{}')", table_name.replace('\'', "''"));
            let rows: Vec<(i64, String, String, i64, Option<String>, i64)> =
                sqlx::query_as(&pragma_sql).fetch_all(db.pool()).await?;
            rows.into_iter()
                .map(|(_, name, raw_type, notnull, default, _)| {
                    let family = logical_type_family(&name, &raw_type);
                    ColumnManifest {
                        default: normalize_default_value(family, default.as_deref()),
                        family,
                        nullable: notnull == 0,
                        name,
                    }
                })
                .collect()
        }
        Dialect::Postgres => {
            let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
                "SELECT column_name, data_type, is_nullable, column_default \
                 FROM information_schema.columns \
                 WHERE table_schema = 'public' AND table_name = $1 \
                 ORDER BY ordinal_position",
            )
            .bind(table_name)
            .fetch_all(db.pool())
            .await?;
            rows.into_iter()
                .map(|(name, raw_type, is_nullable, default)| {
                    let family = logical_type_family(&name, &raw_type);
                    ColumnManifest {
                        default: normalize_default_value(family, default.as_deref()),
                        family,
                        nullable: is_nullable.eq_ignore_ascii_case("YES"),
                        name,
                    }
                })
                .collect()
        }
        Dialect::Spanner => {
            let client = db
                .spanner()
                .expect("schema inspection requires native Spanner client")
                .client();
            let mut tx = client.single().await?;
            let mut stmt = Statement::new(
                "SELECT column_name, spanner_type, CAST(is_nullable AS STRING) AS is_nullable \
                 FROM INFORMATION_SCHEMA.COLUMNS \
                 WHERE table_schema = '' AND table_name = @table_name \
                 ORDER BY ordinal_position",
            );
            stmt.add_param("table_name", &table_name);
            let mut rows = tx.query(stmt).await?;
            let mut columns = Vec::new();
            while let Some(row) = rows.next().await? {
                let name = row.column_by_name::<String>("column_name")?;
                let raw_type = row.column_by_name::<String>("spanner_type")?;
                let family = logical_type_family(&name, &raw_type);
                columns.push(ColumnManifest {
                    default: spanner_column_defaults
                        .and_then(|tables| tables.get(table_name))
                        .and_then(|columns| columns.get(&name))
                        .and_then(|raw| normalize_default_value(family, Some(raw))),
                    name,
                    family,
                    nullable: row
                        .column_by_name::<String>("is_nullable")?
                        .eq_ignore_ascii_case("YES"),
                });
            }
            columns
        }
    };

    if db.dialect() == Dialect::Spanner {
        columns.retain(|column| !column.name.starts_with(SPANNER_HELPER_PREFIX));
    }

    columns.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(columns)
}

async fn list_primary_key_columns(db: &Db, table_name: &str) -> anyhow::Result<Vec<String>> {
    let columns = match db.dialect() {
        Dialect::Sqlite => {
            let pragma_sql = format!("PRAGMA table_info('{}')", table_name.replace('\'', "''"));
            let rows: Vec<(i64, String, String, i64, Option<String>, i64)> =
                sqlx::query_as(&pragma_sql).fetch_all(db.pool()).await?;
            let mut keyed = rows
                .into_iter()
                .filter_map(|(_, name, _, _, _, pk)| (pk > 0).then_some((pk, name)))
                .collect::<Vec<_>>();
            keyed.sort_by_key(|(position, _)| *position);
            keyed.into_iter().map(|(_, name)| name).collect()
        }
        Dialect::Postgres => {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT kcu.column_name \
                 FROM information_schema.table_constraints tc \
                 JOIN information_schema.key_column_usage kcu \
                   ON tc.constraint_name = kcu.constraint_name \
                  AND tc.table_schema = kcu.table_schema \
                  AND tc.table_name = kcu.table_name \
                 WHERE tc.table_schema = 'public' \
                   AND tc.table_name = $1 \
                   AND tc.constraint_type = 'PRIMARY KEY' \
                 ORDER BY kcu.ordinal_position",
            )
            .bind(table_name)
            .fetch_all(db.pool())
            .await?;
            rows.into_iter().map(|(name,)| name).collect()
        }
        Dialect::Spanner => {
            let client = db
                .spanner()
                .expect("schema inspection requires native Spanner client")
                .client();
            let mut tx = client.single().await?;
            let mut stmt = Statement::new(
                "SELECT column_name \
                 FROM INFORMATION_SCHEMA.INDEX_COLUMNS \
                 WHERE table_schema = '' \
                   AND table_name = @table_name \
                   AND index_name = 'PRIMARY_KEY' \
                 ORDER BY ordinal_position",
            );
            stmt.add_param("table_name", &table_name);
            let mut rows = tx.query(stmt).await?;
            let mut columns = Vec::new();
            while let Some(row) = rows.next().await? {
                columns.push(row.column_by_name::<String>("column_name")?);
            }
            columns
        }
    };

    Ok(columns)
}

async fn list_named_secondary_indexes(
    db: &Db,
    table_name: &str,
) -> anyhow::Result<Vec<IndexManifest>> {
    let prefix = backend_capabilities(db.backend()).named_secondary_index_prefix;
    let mut grouped = BTreeMap::<String, (bool, Option<String>, Vec<(i64, String)>)>::new();

    match db.dialect() {
        Dialect::Sqlite => {
            let pragma_sql = format!("PRAGMA index_list('{}')", table_name.replace('\'', "''"));
            let rows: Vec<(i64, String, i64, String, i64)> =
                sqlx::query_as(&pragma_sql).fetch_all(db.pool()).await?;

            for (_, index_name, unique, _, _) in rows
                .into_iter()
                .filter(|(_, index_name, _, _, _)| index_name.starts_with(prefix))
            {
                let predicate = sqlite_index_predicate(db, &index_name).await?;
                let info_rows = sqlite_index_columns(db, &index_name).await?;
                let entry =
                    grouped
                        .entry(index_name)
                        .or_insert((unique != 0, predicate, Vec::new()));
                for (position, column_name) in info_rows.into_iter().enumerate() {
                    entry.2.push((position as i64, column_name));
                }
            }
        }
        Dialect::Postgres => {
            let rows: Vec<(String, bool, String, i64, Option<String>)> = sqlx::query_as(
                "SELECT idxcls.relname AS index_name, \
                        idx.indisunique AS is_unique, \
                        att.attname AS column_name, \
                        ord.ordinality::bigint AS position, \
                        pg_get_expr(idx.indpred, idx.indrelid) AS predicate \
                 FROM pg_class tbl \
                 JOIN pg_namespace ns ON ns.oid = tbl.relnamespace \
                 JOIN pg_index idx ON idx.indrelid = tbl.oid \
                 JOIN pg_class idxcls ON idxcls.oid = idx.indexrelid \
                 JOIN unnest(idx.indkey) WITH ORDINALITY AS ord(attnum, ordinality) ON true \
                 JOIN pg_attribute att ON att.attrelid = tbl.oid AND att.attnum = ord.attnum \
                 WHERE ns.nspname = 'public' \
                   AND tbl.relname = $1 \
                   AND NOT idx.indisprimary \
                   AND idxcls.relname LIKE $2 \
                 ORDER BY idxcls.relname, ord.ordinality",
            )
            .bind(table_name)
            .bind(format!("{prefix}%"))
            .fetch_all(db.pool())
            .await?;

            for (index_name, is_unique, column_name, position, predicate) in rows {
                let entry = grouped.entry(index_name).or_insert((
                    is_unique,
                    predicate.as_deref().map(normalize_predicate),
                    Vec::new(),
                ));
                entry.2.push((position, column_name));
            }
        }
        Dialect::Spanner => {
            let client = db
                .spanner()
                .expect("schema inspection requires native Spanner client")
                .client();
            let mut tx = client.single().await?;

            let mut unique_stmt = Statement::new(
                "SELECT index_name, CAST(is_unique AS STRING) AS is_unique \
                 FROM INFORMATION_SCHEMA.INDEXES \
                 WHERE table_schema = '' \
                   AND table_name = @table_name \
                   AND index_name LIKE @prefix \
                 ORDER BY index_name",
            );
            let prefix_pattern = format!("{prefix}%");
            unique_stmt.add_param("table_name", &table_name);
            unique_stmt.add_param("prefix", &prefix_pattern);
            let mut unique_rows = tx.query(unique_stmt).await?;
            while let Some(row) = unique_rows.next().await? {
                let index_name = row.column_by_name::<String>("index_name")?;
                let is_unique = row
                    .column_by_name::<String>("is_unique")?
                    .eq_ignore_ascii_case("TRUE")
                    || row
                        .column_by_name::<String>("is_unique")?
                        .eq_ignore_ascii_case("YES");
                grouped
                    .entry(index_name)
                    .or_insert((is_unique, None, Vec::new()));
            }

            let mut column_stmt = Statement::new(
                "SELECT index_name, column_name, ordinal_position \
                 FROM INFORMATION_SCHEMA.INDEX_COLUMNS \
                 WHERE table_schema = '' \
                   AND table_name = @table_name \
                   AND index_name LIKE @prefix \
                 ORDER BY index_name, ordinal_position",
            );
            column_stmt.add_param("table_name", &table_name);
            column_stmt.add_param("prefix", &prefix_pattern);
            let mut column_rows = tx.query(column_stmt).await?;
            while let Some(row) = column_rows.next().await? {
                let index_name = row.column_by_name::<String>("index_name")?;
                let position = row.column_by_name::<i64>("ordinal_position")?;
                let column_name = row.column_by_name::<String>("column_name")?;
                let entry = grouped
                    .entry(index_name)
                    .or_insert((false, None, Vec::new()));
                entry.2.push((position, column_name));
            }
        }
    }

    let mut indexes = if db.dialect() == Dialect::Spanner {
        match canonical_table_manifest(table_name) {
            Some(canonical_table) => canonical_table
                .indexes
                .iter()
                .filter(|index| grouped.contains_key(&index.name))
                .cloned()
                .collect::<Vec<_>>(),
            None => Vec::new(),
        }
    } else {
        grouped
            .into_iter()
            .map(|(name, (unique, predicate, mut columns))| {
                columns.sort_by_key(|(position, _)| *position);
                IndexManifest {
                    name,
                    unique,
                    columns: columns.into_iter().map(|(_, column)| column).collect(),
                    predicate,
                }
            })
            .collect::<Vec<_>>()
    };
    indexes.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(indexes)
}

async fn list_foreign_keys(
    db: &Db,
    table_name: &str,
    spanner_foreign_keys: Option<&BTreeMap<String, Vec<ForeignKeyManifest>>>,
) -> anyhow::Result<Vec<ForeignKeyManifest>> {
    let mut foreign_keys = match db.dialect() {
        Dialect::Sqlite => {
            let pragma_sql = format!(
                "PRAGMA foreign_key_list('{}')",
                table_name.replace('\'', "''")
            );
            let rows: Vec<SqliteForeignKeyPragmaRow> =
                sqlx::query_as(&pragma_sql).fetch_all(db.pool()).await?;
            let mut grouped =
                BTreeMap::<i64, (String, ForeignKeyDeleteAction, Vec<(i64, String, String)>)>::new(
                );

            for (id, seq, referenced_table, from_column, to_column, _, on_delete, _) in rows {
                let entry = grouped.entry(id).or_insert((
                    referenced_table,
                    normalize_delete_action(&on_delete),
                    Vec::new(),
                ));
                entry.2.push((seq, from_column, to_column));
            }

            grouped
                .into_values()
                .map(|(referenced_table, on_delete, mut pairs)| {
                    pairs.sort_by_key(|(seq, _, _)| *seq);
                    ForeignKeyManifest {
                        columns: pairs.iter().map(|(_, from, _)| from.clone()).collect(),
                        referenced_table,
                        referenced_columns: pairs.into_iter().map(|(_, _, to)| to).collect(),
                        on_delete,
                    }
                })
                .collect::<Vec<_>>()
        }
        Dialect::Postgres => {
            let rows: Vec<(String, i64, String, String, String, String)> = sqlx::query_as(
                "SELECT con.conname, \
                        lcols.ordinality::bigint AS position, \
                        latt.attname AS column_name, \
                        frel.relname AS referenced_table, \
                        ratt.attname AS referenced_column, \
                        con.confdeltype::text AS delete_action \
                 FROM pg_constraint con \
                 JOIN pg_class rel ON rel.oid = con.conrelid \
                 JOIN pg_namespace ns ON ns.oid = rel.relnamespace \
                 JOIN pg_class frel ON frel.oid = con.confrelid \
                 JOIN unnest(con.conkey) WITH ORDINALITY AS lcols(attnum, ordinality) ON true \
                 JOIN unnest(con.confkey) WITH ORDINALITY AS rcols(attnum, ordinality) \
                   ON rcols.ordinality = lcols.ordinality \
                 JOIN pg_attribute latt ON latt.attrelid = con.conrelid AND latt.attnum = lcols.attnum \
                 JOIN pg_attribute ratt ON ratt.attrelid = con.confrelid AND ratt.attnum = rcols.attnum \
                 WHERE ns.nspname = 'public' \
                   AND rel.relname = $1 \
                   AND con.contype = 'f' \
                 ORDER BY con.conname, lcols.ordinality",
            )
            .bind(table_name)
            .fetch_all(db.pool())
            .await?;

            let mut grouped = BTreeMap::<
                String,
                (String, ForeignKeyDeleteAction, Vec<(i64, String, String)>),
            >::new();

            for (
                constraint_name,
                position,
                column_name,
                referenced_table,
                referenced_column,
                delete_action,
            ) in rows
            {
                let entry = grouped.entry(constraint_name).or_insert((
                    referenced_table,
                    normalize_postgres_delete_action(&delete_action),
                    Vec::new(),
                ));
                entry.2.push((position, column_name, referenced_column));
            }

            grouped
                .into_values()
                .map(|(referenced_table, on_delete, mut pairs)| {
                    pairs.sort_by_key(|(position, _, _)| *position);
                    ForeignKeyManifest {
                        columns: pairs.iter().map(|(_, from, _)| from.clone()).collect(),
                        referenced_table,
                        referenced_columns: pairs.into_iter().map(|(_, _, to)| to).collect(),
                        on_delete,
                    }
                })
                .collect::<Vec<_>>()
        }
        Dialect::Spanner => spanner_foreign_keys
            .and_then(|tables| tables.get(table_name))
            .cloned()
            .unwrap_or_default(),
    };

    foreign_keys.sort_by(|left, right| {
        left.columns
            .cmp(&right.columns)
            .then(left.referenced_table.cmp(&right.referenced_table))
    });
    Ok(foreign_keys)
}

async fn list_unique_keys(db: &Db, table_name: &str) -> anyhow::Result<Vec<UniqueKeyManifest>> {
    let mut unique_keys = match db.dialect() {
        Dialect::Sqlite => {
            let pragma_sql = format!("PRAGMA index_list('{}')", table_name.replace('\'', "''"));
            let rows: Vec<(i64, String, i64, String, i64)> =
                sqlx::query_as(&pragma_sql).fetch_all(db.pool()).await?;

            let mut unique_keys = Vec::new();
            for (_, index_name, unique, origin, _) in rows {
                if unique == 0 || origin == "pk" {
                    continue;
                }
                unique_keys.push(UniqueKeyManifest {
                    columns: sqlite_index_columns(db, &index_name).await?,
                    predicate: sqlite_index_predicate(db, &index_name).await?,
                });
            }
            unique_keys
        }
        Dialect::Postgres => {
            let rows: Vec<(String, String, i64, Option<String>)> = sqlx::query_as(
                "SELECT idxcls.relname AS index_name, \
                        att.attname AS column_name, \
                        ord.ordinality::bigint AS position, \
                        pg_get_expr(idx.indpred, idx.indrelid) AS predicate \
                 FROM pg_class tbl \
                 JOIN pg_namespace ns ON ns.oid = tbl.relnamespace \
                 JOIN pg_index idx ON idx.indrelid = tbl.oid \
                 JOIN pg_class idxcls ON idxcls.oid = idx.indexrelid \
                 JOIN unnest(idx.indkey) WITH ORDINALITY AS ord(attnum, ordinality) ON true \
                 JOIN pg_attribute att ON att.attrelid = tbl.oid AND att.attnum = ord.attnum \
                 WHERE ns.nspname = 'public' \
                   AND tbl.relname = $1 \
                   AND idx.indisunique \
                   AND NOT idx.indisprimary \
                 ORDER BY idxcls.relname, ord.ordinality",
            )
            .bind(table_name)
            .fetch_all(db.pool())
            .await?;

            let mut grouped = BTreeMap::<String, (Option<String>, Vec<(i64, String)>)>::new();
            for (index_name, column_name, position, predicate) in rows {
                let entry = grouped
                    .entry(index_name)
                    .or_insert((predicate.as_deref().map(normalize_predicate), Vec::new()));
                entry.1.push((position, column_name));
            }

            grouped
                .into_values()
                .map(|(predicate, mut columns)| {
                    columns.sort_by_key(|(position, _)| *position);
                    UniqueKeyManifest {
                        columns: columns.into_iter().map(|(_, column)| column).collect(),
                        predicate,
                    }
                })
                .collect::<Vec<_>>()
        }
        Dialect::Spanner => {
            let client = db
                .spanner()
                .expect("schema inspection requires native Spanner client")
                .client();
            let mut tx = client.single().await?;

            let mut unique_stmt = Statement::new(
                "SELECT index_name \
                 FROM INFORMATION_SCHEMA.INDEXES \
                 WHERE table_schema = '' \
                   AND table_name = @table_name \
                   AND is_unique = TRUE \
                 ORDER BY index_name",
            );
            unique_stmt.add_param("table_name", &table_name);
            let mut unique_rows = tx.query(unique_stmt).await?;
            let mut live_unique_indexes = Vec::new();
            while let Some(row) = unique_rows.next().await? {
                live_unique_indexes.push(row.column_by_name::<String>("index_name")?);
            }

            match canonical_table_manifest(table_name) {
                Some(canonical_table) => canonical_table
                    .unique_keys
                    .iter()
                    .filter(|unique_key| {
                        let expected_name =
                            expected_unique_key_index_name(canonical_table, unique_key);
                        live_unique_indexes
                            .iter()
                            .any(|live| live == &expected_name)
                    })
                    .cloned()
                    .collect::<Vec<_>>(),
                None => Vec::new(),
            }
        }
    };

    unique_keys.sort_by(|left, right| {
        left.columns
            .cmp(&right.columns)
            .then(left.predicate.cmp(&right.predicate))
    });
    Ok(unique_keys)
}

async fn list_check_constraints(
    db: &Db,
    table_name: &str,
    spanner_checks: Option<&BTreeMap<String, Vec<CheckConstraintManifest>>>,
) -> anyhow::Result<Vec<CheckConstraintManifest>> {
    let mut checks = match db.dialect() {
        Dialect::Sqlite => {
            let row: Option<(Option<String>,)> =
                sqlx::query_as("SELECT sql FROM sqlite_master WHERE type = 'table' AND name = $1")
                    .bind(table_name)
                    .fetch_optional(db.pool())
                    .await?;
            row.and_then(|(sql,)| sql)
                .map(|sql| {
                    extract_check_expressions(&sql)
                        .into_iter()
                        .map(|expression| CheckConstraintManifest {
                            expression: normalize_predicate(&expression),
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        }
        Dialect::Postgres => {
            let rows: Vec<(String,)> = sqlx::query_as(
                "SELECT pg_get_constraintdef(con.oid, true) AS definition \
                 FROM pg_constraint con \
                 JOIN pg_class rel ON rel.oid = con.conrelid \
                 JOIN pg_namespace ns ON ns.oid = rel.relnamespace \
                 WHERE ns.nspname = 'public' \
                   AND rel.relname = $1 \
                   AND con.contype = 'c' \
                 ORDER BY con.conname",
            )
            .bind(table_name)
            .fetch_all(db.pool())
            .await?;

            rows.into_iter()
                .flat_map(|(definition,)| extract_check_expressions(&definition))
                .map(|expression| CheckConstraintManifest {
                    expression: normalize_predicate(&expression),
                })
                .collect::<Vec<_>>()
        }
        Dialect::Spanner => spanner_checks
            .and_then(|tables| tables.get(table_name))
            .cloned()
            .unwrap_or_default(),
    };

    checks.sort_by(|left, right| left.expression.cmp(&right.expression));
    checks.dedup_by(|left, right| left.expression == right.expression);
    Ok(checks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_capabilities_make_spanner_differences_explicit() {
        let sqlite = backend_capabilities(BackendKind::Sqlite);
        let postgres = backend_capabilities(BackendKind::Postgres);
        let spanner = backend_capabilities(BackendKind::Spanner);

        assert!(!sqlite.native_transport);
        assert!(postgres.transactional_ddl);
        assert!(spanner.native_transport);
        assert!(!spanner.transactional_ddl);
        assert_eq!(spanner.named_secondary_index_prefix, "idx_");
    }

    #[test]
    fn normalize_type_family_collapses_dialect_specific_names() {
        assert_eq!(normalize_type_family("text"), ColumnTypeFamily::String);
        assert_eq!(
            normalize_type_family("timestamp with time zone"),
            ColumnTypeFamily::Timestamp
        );
        assert_eq!(
            normalize_type_family("STRING(MAX)"),
            ColumnTypeFamily::String
        );
        assert_eq!(normalize_type_family("jsonb"), ColumnTypeFamily::Json);
    }

    #[test]
    fn logical_type_family_promotes_storage_encoded_json_and_timestamps() {
        assert_eq!(
            logical_type_family("metadata", "TEXT"),
            ColumnTypeFamily::Json
        );
        assert_eq!(
            logical_type_family("created_at", "TEXT"),
            ColumnTypeFamily::Timestamp
        );
        assert_eq!(
            logical_type_family("name", "TEXT"),
            ColumnTypeFamily::String
        );
    }

    #[test]
    fn normalize_default_value_collapses_timestamp_expressions_and_json_literals() {
        assert_eq!(
            normalize_default_value(ColumnTypeFamily::Timestamp, Some("(datetime('now'))")),
            Some(DefaultValueManifest::CurrentTimestamp)
        );
        assert_eq!(
            normalize_default_value(
                ColumnTypeFamily::Timestamp,
                Some("(NOW() + INTERVAL '10 minutes')")
            ),
            Some(DefaultValueManifest::CurrentTimestampPlusSeconds(600))
        );
        assert_eq!(
            normalize_default_value(
                ColumnTypeFamily::Timestamp,
                Some("(datetime('now', '+600 seconds'))")
            ),
            Some(DefaultValueManifest::CurrentTimestampPlusSeconds(600))
        );
        assert_eq!(
            normalize_default_value(ColumnTypeFamily::Json, Some("'{}'::jsonb")),
            Some(DefaultValueManifest::Json(serde_json::json!({})))
        );
        assert_eq!(
            normalize_default_value(ColumnTypeFamily::Bool, Some("FALSE")),
            Some(DefaultValueManifest::Bool(false))
        );
    }

    #[test]
    fn foreign_key_delete_actions_normalize_across_dialects() {
        assert_eq!(
            normalize_delete_action("NO ACTION"),
            ForeignKeyDeleteAction::NoAction
        );
        assert_eq!(
            normalize_delete_action("SET NULL"),
            ForeignKeyDeleteAction::SetNull
        );
        assert_eq!(
            normalize_postgres_delete_action("c"),
            ForeignKeyDeleteAction::Cascade
        );
        assert_eq!(
            normalize_postgres_delete_action("r"),
            ForeignKeyDeleteAction::Restrict
        );
    }

    #[test]
    fn predicate_normalization_collapses_pg_syntax_to_logical_shape() {
        assert_eq!(
            normalize_predicate("((token_hash <> ''::text))"),
            "token_hash != ''"
        );
        assert_eq!(
            normalize_predicate("(expires_at IS NOT NULL)"),
            "expires_at IS NOT NULL"
        );
    }

    #[test]
    fn extract_check_expressions_handles_inline_and_multiline_checks() {
        let sql = r#"
            CREATE TABLE instances (
                kind TEXT CHECK (kind IN ('root', 'managed')),
                placement_mode TEXT CHECK (placement_mode IN ('global', 'regional')),
                CHECK (
                    (parent_instance_id IS NULL AND owner_org_id IS NULL AND kind = 'root')
                    OR (parent_instance_id IS NOT NULL AND owner_org_id IS NOT NULL)
                )
            )
        "#;
        let checks = extract_check_expressions(sql);
        assert_eq!(checks.len(), 3);
        assert!(checks[0].contains("kind IN"));
        assert!(checks[1].contains("placement_mode IN"));
        assert!(checks[2].contains("parent_instance_id IS NULL"));
    }

    #[test]
    fn canonical_manifest_is_available() {
        assert!(!canonical_manifest().tables.is_empty());
    }

    #[test]
    fn spanner_partial_helper_names_start_with_a_letter() {
        let name = spanner_partial_marker_name("idx_auth_states_instance_state");
        let first = name
            .chars()
            .next()
            .expect("helper marker names should not be empty");

        assert!(first.is_ascii_alphabetic(), "{name}");
        assert!(name.starts_with(SPANNER_HELPER_PREFIX), "{name}");
    }
}
