use anyhow::Context;
use google_cloud_spanner::{
    client::Error as SpannerError, row::Row as SpannerRow, statement::Statement,
};
use serde_json::{Map, Value};
use std::collections::HashSet;

use crate::{
    Db, SpannerDb, add_instance_domain, get_org, get_schema_record, get_user,
    list_instance_domains, list_schema_registry, provider, resolve_domain_route,
};
use zitadel_app::repo::{
    DomainRecord, GroupRecord, InstanceRecord, ListParams, OrgRecord, ProviderRecord,
    SchemaRecord, SettingsRecord, UserRecord,
};

pub(super) const DEFAULT_LIST_LIMIT: i64 = 50;
pub(super) const MAX_LIST_LIMIT: i64 = 200;

#[derive(Clone)]
pub struct SqlUserRepository {
    pub(super) db: Db,
}

#[derive(Clone)]
pub struct SqlOrgRepository {
    pub(super) db: Db,
}

#[derive(Clone)]
pub struct SqlGroupRepository {
    pub(super) db: Db,
}

#[derive(Clone)]
pub struct SqlInstanceRepository {
    pub(super) db: Db,
}

#[derive(Clone)]
pub struct SqlProviderRepository {
    pub(super) db: Db,
}

#[derive(Clone)]
pub struct SqlSchemaRepository {
    pub(super) db: Db,
}

#[derive(Clone)]
pub struct SqlSettingsRepository {
    pub(super) db: Db,
}

#[derive(Clone)]
pub struct SqlSearchRepository {
    pub(super) db: Db,
}

impl SqlUserRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlOrgRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlGroupRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlInstanceRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlProviderRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlSchemaRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlSettingsRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

impl SqlSearchRepository {
    pub fn new(db: Db) -> Self {
        Self { db }
    }
}

// ---------------------------------------------------------------------------
// SQL row type aliases
// ---------------------------------------------------------------------------

pub(super) type UserSqlRow = (
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);
pub(super) type OrgSqlRow = (String, String, String, String, String, String);
pub(super) type GroupSqlRow = (String, String, String, String, String, String, String);

// ---------------------------------------------------------------------------
// Row conversion helpers
// ---------------------------------------------------------------------------

pub(super) fn user_from_sql_row(row: UserSqlRow) -> UserRecord {
    UserRecord {
        id: row.0,
        org_id: row.1,
        identifier: row.2,
        display_name: row.3,
        user_type: row.4,
        state: row.5,
        schema_id: row.6,
        metadata: json_value(&row.7),
        created_at: row.8,
        updated_at: row.9,
    }
}

pub(super) fn user_from_spanner_row(row: SpannerRow) -> UserRecord {
    UserRecord {
        id: row.column_by_name::<String>("id").unwrap_or_default(),
        org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
        identifier: row
            .column_by_name::<String>("identifier")
            .unwrap_or_default(),
        display_name: row
            .column_by_name::<String>("display_name")
            .unwrap_or_default(),
        user_type: row
            .column_by_name::<String>("user_type")
            .unwrap_or_default(),
        state: row.column_by_name::<String>("state").unwrap_or_default(),
        schema_id: row
            .column_by_name::<String>("schema_id")
            .unwrap_or_default(),
        metadata: json_value(
            &row.column_by_name::<String>("metadata")
                .unwrap_or_else(|_| "{}".to_string()),
        ),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
        updated_at: row
            .column_by_name::<String>("updated_at")
            .unwrap_or_default(),
    }
}

pub(super) fn org_from_sql_row(row: OrgSqlRow) -> OrgRecord {
    OrgRecord {
        id: row.0,
        name: row.1,
        state: row.2,
        metadata: json_value(&row.3),
        created_at: row.4,
        updated_at: row.5,
    }
}

pub(super) fn org_from_spanner_row(row: SpannerRow) -> OrgRecord {
    OrgRecord {
        id: row.column_by_name::<String>("id").unwrap_or_default(),
        name: row.column_by_name::<String>("name").unwrap_or_default(),
        state: row.column_by_name::<String>("state").unwrap_or_default(),
        metadata: json_value(
            &row.column_by_name::<String>("metadata")
                .unwrap_or_else(|_| "{}".to_string()),
        ),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
        updated_at: row
            .column_by_name::<String>("updated_at")
            .unwrap_or_default(),
    }
}

pub(super) fn group_from_sql_row(row: GroupSqlRow) -> GroupRecord {
    GroupRecord {
        id: row.0,
        org_id: row.1,
        name: row.2,
        state: row.3,
        metadata: json_value(&row.4),
        created_at: row.5,
        updated_at: row.6,
    }
}

pub(super) fn group_from_spanner_row(row: SpannerRow) -> GroupRecord {
    GroupRecord {
        id: row.column_by_name::<String>("id").unwrap_or_default(),
        org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
        name: row.column_by_name::<String>("name").unwrap_or_default(),
        state: row.column_by_name::<String>("state").unwrap_or_default(),
        metadata: json_value(
            &row.column_by_name::<String>("metadata")
                .unwrap_or_else(|_| "{}".to_string()),
        ),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
        updated_at: row
            .column_by_name::<String>("updated_at")
            .unwrap_or_default(),
    }
}

pub(super) fn instance_from_retained(record: crate::ManagedInstanceRecord) -> InstanceRecord {
    InstanceRecord {
        instance_id: record.instance_id,
        state: record.state,
        kind: record.kind,
        placement_mode: record.placement_mode,
        region_key: record.region_key,
        owner_org_id: Some(record.owner_org_id),
        feature_overrides: json_value(&record.feature_overrides_json),
        primary_domain: record.primary_domain,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

pub(super) fn domain_from_retained(record: crate::DomainRecord) -> DomainRecord {
    DomainRecord {
        domain: record.domain,
        is_primary: record.is_primary,
        state: record.state,
        verified: record.verified,
        created_at: record.created_at,
        updated_at: record.updated_at,
    }
}

pub(super) fn schema_from_retained(record: crate::SchemaRegistryRecord) -> SchemaRecord {
    SchemaRecord {
        id: record.id,
        schema_type: record.type_,
        schema_json: json_value(&record.schema_json),
        version: record.version,
        is_default: record.is_default,
        visibility: record.visibility,
        created_at: record.created_at,
    }
}

pub(super) fn provider_from_storage(record: provider::ProviderRecord) -> anyhow::Result<ProviderRecord> {
    let mut config = serde_json::to_value(&record.payload)?;
    if let Value::Object(map) = &mut config {
        map.remove("display_name");
        map.remove("protocol");
        map.insert("org_id".to_string(), Value::String(record.org_id));
    }
    Ok(ProviderRecord {
        id: record.id,
        name: record.payload.display_name,
        protocol: record.payload.protocol,
        state: if record.payload.enabled {
            "active".to_string()
        } else {
            "inactive".to_string()
        },
        config,
        created_at: record.created_at,
        updated_at: record.updated_at,
    })
}

pub(super) fn provider_payload_from_record(
    record: &ProviderRecord,
) -> anyhow::Result<provider::ProviderPayload> {
    let mut payload = if has_provider_payload_shape(&record.config) {
        serde_json::from_value::<provider::ProviderPayload>(record.config.clone())
            .unwrap_or_default()
    } else {
        let connection =
            serde_json::from_value::<provider::ProviderConnection>(record.config.clone())
                .unwrap_or_default();
        provider::ProviderPayload {
            connection,
            ..provider::ProviderPayload::default()
        }
    };
    payload.display_name = record.name.clone();
    payload.protocol = record.protocol.clone();
    payload.enabled = record.state == "active";
    Ok(payload)
}

pub(super) fn has_provider_payload_shape(config: &Value) -> bool {
    config
        .as_object()
        .is_some_and(|map| map.contains_key("connection") || map.contains_key("mapping"))
}

pub(super) fn provider_org_id(config: &Value) -> Option<String> {
    config
        .get("org_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

pub(super) fn parse_scope(scope: &str) -> (String, String) {
    if let Some((scope_kind, scope_id)) = scope.split_once(':') {
        (scope_kind.to_string(), scope_id.to_string())
    } else {
        (scope.to_string(), String::new())
    }
}

pub(super) fn format_scope(scope: &str, scope_id: &str) -> String {
    if scope_id.is_empty() {
        scope.to_string()
    } else {
        format!("{scope}:{scope_id}")
    }
}

pub(super) fn limit_from_params(params: &ListParams) -> i64 {
    i64::from(params.limit.unwrap_or(DEFAULT_LIST_LIMIT as u32)).clamp(1, MAX_LIST_LIMIT)
}

pub(super) fn json_string(value: &Value) -> anyhow::Result<String> {
    serde_json::to_string(value).context("serialize json")
}

pub(super) fn json_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::Object(Map::new()))
}

pub(super) fn next_cursor<T, F>(items: &[T], limit: i64, f: F) -> Option<String>
where
    F: Fn(&T) -> &str,
{
    if items.len() < limit as usize {
        None
    } else {
        items.last().map(|item| f(item).to_string())
    }
}

pub(super) fn normalized_resource_types(resource_types: Option<&[&str]>) -> HashSet<String> {
    resource_types
        .into_iter()
        .flatten()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

// ---------------------------------------------------------------------------
// Async loader helpers
// ---------------------------------------------------------------------------

pub(super) async fn load_user(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Option<UserRecord>> {
    Ok(get_user(db, instance_id, user_id)
        .await?
        .map(|row| UserRecord {
            id: row.id,
            org_id: row.org_id,
            identifier: row.identifier,
            display_name: row.display_name,
            user_type: row.user_type,
            state: row.state,
            schema_id: row.schema_id,
            metadata: json_value(&row.metadata_json),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
}

pub(super) async fn load_org(db: &Db, instance_id: &str, org_id: &str) -> anyhow::Result<Option<OrgRecord>> {
    Ok(get_org(db, instance_id, org_id)
        .await?
        .map(|row| OrgRecord {
            id: row.id,
            name: row.name,
            state: row.state,
            metadata: json_value(&row.metadata_json),
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
}

pub(super) async fn load_group(
    db: &Db,
    instance_id: &str,
    group_id: &str,
) -> anyhow::Result<Option<GroupRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, COALESCE(org_id, ''), name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                 FROM groups WHERE instance_id = $1 AND id = $2"
            );
            Ok(sqlx::query_as::<_, GroupSqlRow>(&sql)
                .bind(instance_id)
                .bind(group_id)
                .fetch_optional(scoped.pool())
                .await?
                .map(group_from_sql_row))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, IFNULL(org_id, '') AS org_id, name, state, IFNULL(metadata, '{}') AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM groups WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &group_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(group_from_spanner_row))
        }
    }
}

pub(super) async fn load_instance(db: &Db, instance_id: &str) -> anyhow::Result<Option<InstanceRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let feature_overrides = scoped.as_text("i.feature_overrides");
            let created_at = scoped.as_text("i.created_at");
            let updated_at = scoped.as_text("i.updated_at");
            let sql = format!(
                "SELECT i.instance_id, i.state, i.kind, i.placement_mode, NULLIF(i.region_key, ''), i.owner_org_id, \
                        COALESCE({feature_overrides}, '{{}}'), d.domain, {created_at}, {updated_at} \
                 FROM instances i \
                 LEFT JOIN domains d ON d.instance_id = i.instance_id AND d.org_id IS NULL AND d.is_primary = TRUE \
                 WHERE i.instance_id = $1 LIMIT 1"
            );
            let row: Option<(
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                String,
                Option<String>,
                String,
                String,
            )> = sqlx::query_as(&sql)
                .bind(instance_id)
                .fetch_optional(scoped.pool())
                .await?;
            Ok(row.map(|row| InstanceRecord {
                instance_id: row.0,
                state: row.1,
                kind: row.2,
                placement_mode: row.3,
                region_key: row.4,
                owner_org_id: row.5,
                feature_overrides: json_value(&row.6),
                primary_domain: row.7,
                created_at: row.8,
                updated_at: row.9,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT i.instance_id, i.state, i.kind, i.placement_mode, i.region_key, i.owner_org_id, \
                        IFNULL(i.feature_overrides, '{}') AS feature_overrides, d.domain AS primary_domain, \
                        CAST(i.created_at AS STRING) AS created_at, CAST(i.updated_at AS STRING) AS updated_at \
                 FROM instances i \
                 LEFT JOIN domains d ON d.instance_id = i.instance_id AND d.org_id IS NULL AND d.is_primary = TRUE \
                 WHERE i.instance_id = @instance_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| InstanceRecord {
                    instance_id: row
                        .column_by_name::<String>("instance_id")
                        .unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    kind: row.column_by_name::<String>("kind").unwrap_or_default(),
                    placement_mode: row
                        .column_by_name::<String>("placement_mode")
                        .unwrap_or_default(),
                    region_key: row
                        .column_by_name::<Option<String>>("region_key")
                        .unwrap_or(None)
                        .filter(|value| !value.is_empty()),
                    owner_org_id: row
                        .column_by_name::<Option<String>>("owner_org_id")
                        .unwrap_or(None),
                    feature_overrides: json_value(
                        &row.column_by_name::<String>("feature_overrides")
                            .unwrap_or_else(|_| "{}".to_string()),
                    ),
                    primary_domain: row
                        .column_by_name::<Option<String>>("primary_domain")
                        .unwrap_or(None),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                    updated_at: row
                        .column_by_name::<String>("updated_at")
                        .unwrap_or_default(),
                }))
        }
    }
}

pub(super) async fn load_provider(
    db: &Db,
    instance_id: &str,
    provider_id: &str,
) -> anyhow::Result<Option<ProviderRecord>> {
    provider::get_provider_for(db, instance_id, provider_id)
        .await?
        .map(provider_from_storage)
        .transpose()
}

pub(super) async fn load_schema(db: &Db, schema_id: &str) -> anyhow::Result<Option<SchemaRecord>> {
    Ok(get_schema_record(db, schema_id)
        .await?
        .map(schema_from_retained))
}

pub(super) async fn load_settings_exact(
    db: &Db,
    instance_id: &str,
    settings_type: &str,
    scope: &str,
    scope_id: &str,
) -> anyhow::Result<Option<SettingsRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "SELECT type, scope, scope_id, {} FROM settings \
                 WHERE instance_id = $1 AND type = $2 AND scope = $3 AND scope_id = $4 LIMIT 1",
                scoped.as_text("data"),
            );
            let row: Option<(String, String, String, String)> = sqlx::query_as(&sql)
                .bind(instance_id)
                .bind(settings_type)
                .bind(scope)
                .bind(scope_id)
                .fetch_optional(scoped.pool())
                .await?;
            Ok(row.map(|row| SettingsRecord {
                settings_type: row.0,
                scope: format_scope(&row.1, &row.2),
                data: json_value(&row.3),
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT type, scope, scope_id, IFNULL(data, '{}') AS data FROM settings \
                 WHERE instance_id = @instance_id AND type = @type AND scope = @scope AND scope_id = @scope_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("type", &settings_type);
            stmt.add_param("scope", &scope);
            stmt.add_param("scope_id", &scope_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| SettingsRecord {
                    settings_type: row.column_by_name::<String>("type").unwrap_or_default(),
                    scope: format_scope(
                        &row.column_by_name::<String>("scope").unwrap_or_default(),
                        &row.column_by_name::<String>("scope_id").unwrap_or_default(),
                    ),
                    data: json_value(
                        &row.column_by_name::<String>("data")
                            .unwrap_or_else(|_| "{}".to_string()),
                    ),
                }))
        }
    }
}

pub(super) async fn upsert_domain(db: &Db, instance_id: &str, domain: &DomainRecord) -> anyhow::Result<()> {
    if domain.is_primary {
        let existing = list_instance_domains(db, instance_id).await?;
        for item in existing {
            if item.is_primary && item.domain != domain.domain {
                match db {
                    Db::Sql(_) => {
                        let scoped = db.scoped(instance_id.to_string());
                        sqlx::query(
                            "UPDATE domains SET is_primary = FALSE, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND domain = $2",
                        )
                        .bind(instance_id)
                        .bind(&item.domain)
                        .execute(scoped.pool())
                        .await?;
                    }
                    Db::Spanner(spanner) => {
                        let mut stmt = Statement::new(
                            "UPDATE domains SET is_primary = FALSE, updated_at = CURRENT_TIMESTAMP() \
                             WHERE instance_id = @instance_id AND domain = @domain",
                        );
                        stmt.add_param("instance_id", &instance_id);
                        stmt.add_param("domain", &item.domain);
                        let _ = write_spanner_count(spanner, stmt).await?;
                    }
                }
            }
        }
    }

    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                 VALUES ($1, $2, $3, $4, $5) \
                 ON CONFLICT(domain) DO UPDATE SET \
                     instance_id = excluded.instance_id, \
                     is_primary = excluded.is_primary, \
                     state = excluded.state, \
                     verified = excluded.verified, \
                     updated_at = CURRENT_TIMESTAMP",
            )
            .bind(&domain.domain)
            .bind(instance_id)
            .bind(domain.is_primary)
            .bind(&domain.state)
            .bind(domain.verified)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mut exists =
                Statement::new("SELECT domain FROM domains WHERE domain = @domain LIMIT 1");
            exists.add_param("domain", &domain.domain);
            if spanner_query_optional(spanner, exists).await?.is_some() {
                let mut stmt = Statement::new(
                    "UPDATE domains SET instance_id = @instance_id, is_primary = @is_primary, \
                         state = @state, verified = @verified, updated_at = CURRENT_TIMESTAMP() \
                     WHERE domain = @domain",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("is_primary", &domain.is_primary);
                stmt.add_param("state", &domain.state);
                stmt.add_param("verified", &domain.verified);
                stmt.add_param("domain", &domain.domain);
                let _ = write_spanner_count(spanner, stmt).await?;
            } else if !domain.is_primary && domain.state == "active" && !domain.verified {
                let _ = add_instance_domain(db, instance_id, &domain.domain).await?;
            } else {
                let mut stmt = Statement::new(
                    "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                     VALUES (@domain, @instance_id, @is_primary, @state, @verified)",
                );
                stmt.add_param("domain", &domain.domain);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("is_primary", &domain.is_primary);
                stmt.add_param("state", &domain.state);
                stmt.add_param("verified", &domain.verified);
                write_spanner_stmt(spanner, stmt).await?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Spanner helpers
// ---------------------------------------------------------------------------

pub(super) async fn write_spanner_stmt(spanner: &SpannerDb, stmt: Statement) -> anyhow::Result<()> {
    let _ = spanner
        .client()
        .read_write_transaction(|tx| {
            let stmt = stmt.clone();
            Box::pin(async move {
                tx.update(stmt).await?;
                Ok::<(), SpannerError>(())
            })
        })
        .await?;
    Ok(())
}

pub(super) async fn write_spanner_count(spanner: &SpannerDb, stmt: Statement) -> anyhow::Result<u64> {
    let (_, affected) = spanner
        .client()
        .read_write_transaction(|tx| {
            let stmt = stmt.clone();
            Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
        })
        .await?;
    Ok(affected as u64)
}

pub(super) async fn write_spanner_many(spanner: &SpannerDb, stmts: Vec<Statement>) -> anyhow::Result<()> {
    let _ = spanner
        .client()
        .read_write_transaction(|tx| {
            let stmts = stmts.clone();
            Box::pin(async move {
                for stmt in stmts {
                    tx.update(stmt).await?;
                }
                Ok::<(), SpannerError>(())
            })
        })
        .await?;
    Ok(())
}

pub(super) async fn spanner_query_all(
    spanner: &SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Vec<SpannerRow>> {
    let mut tx = spanner.client().single().await?;
    let mut rows = tx.query(stmt).await?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().await? {
        result.push(row);
    }
    Ok(result)
}

pub(super) async fn spanner_query_optional(
    spanner: &SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Option<SpannerRow>> {
    let mut tx = spanner.client().single().await?;
    let mut rows = tx.query(stmt).await?;
    Ok(rows.next().await?)
}
