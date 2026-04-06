use crate::{Db, scoped::ScopedDb};
use google_cloud_spanner::{client::Error as SpannerError, statement::Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ProviderConnection {
    #[serde(default)]
    pub issuer: String,
    #[serde(default)]
    pub authorization_url: String,
    #[serde(default)]
    pub token_url: String,
    #[serde(default)]
    pub userinfo_url: String,
    #[serde(default)]
    pub jwks_uri: String,
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default, deserialize_with = "deserialize_scopes")]
    pub scopes: Vec<String>,
    #[serde(default = "default_token_endpoint_auth_method")]
    pub token_endpoint_auth_method: String,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

fn default_token_endpoint_auth_method() -> String {
    "client_secret_post".to_string()
}

fn deserialize_scopes<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::Null => Ok(Vec::new()),
        Value::String(raw) => Ok(raw
            .split_whitespace()
            .filter(|scope| !scope.is_empty())
            .map(str::to_string)
            .collect()),
        Value::Array(items) => Ok(items
            .into_iter()
            .filter_map(|value| value.as_str().map(str::to_string))
            .collect()),
        _ => Err(serde::de::Error::custom("scopes must be a string or array")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProviderMapping {
    #[serde(default)]
    pub claims: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProviderTarget {
    #[serde(default)]
    pub schema_type: String,
    #[serde(default)]
    pub schema_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLinkingMode {
    #[default]
    CreateOrLink,
    LinkOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMatchBy {
    #[default]
    VerifiedEmail,
    Identifier,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProviderLinking {
    #[serde(default)]
    pub mode: ProviderLinkingMode,
    #[serde(default)]
    pub match_by: ProviderMatchBy,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ProviderUi {
    #[serde(default)]
    pub display_order: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct ProviderCatalogRef {
    #[serde(default)]
    pub template_id: String,
    #[serde(default)]
    pub template_version: String,
    #[serde(default)]
    pub official: bool,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub logo_url: String,
    #[serde(default)]
    pub docs_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderPayload {
    pub display_name: String,
    #[serde(default = "default_provider_kind")]
    pub kind: String,
    pub protocol: String,
    #[serde(default)]
    pub connection: ProviderConnection,
    #[serde(default)]
    pub mapping: ProviderMapping,
    #[serde(default)]
    pub target: ProviderTarget,
    #[serde(default)]
    pub linking: ProviderLinking,
    #[serde(default = "default_json_object")]
    pub session: Value,
    #[serde(default)]
    pub ui: ProviderUi,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub catalog_ref: ProviderCatalogRef,
}

fn default_provider_kind() -> String {
    "custom".to_string()
}

fn default_json_object() -> Value {
    Value::Object(Default::default())
}

fn default_enabled() -> bool {
    true
}

impl Default for ProviderPayload {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            kind: default_provider_kind(),
            protocol: "oidc".to_string(),
            connection: ProviderConnection::default(),
            mapping: ProviderMapping::default(),
            target: ProviderTarget::default(),
            linking: ProviderLinking::default(),
            session: default_json_object(),
            ui: ProviderUi::default(),
            enabled: true,
            catalog_ref: ProviderCatalogRef::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderRecord {
    pub id: String,
    pub org_id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(flatten)]
    pub payload: ProviderPayload,
}

fn parse_json_field<T: serde::de::DeserializeOwned>(label: &str, raw: &str) -> anyhow::Result<T> {
    serde_json::from_str(raw).map_err(|error| anyhow::anyhow!("parse provider {label}: {error}"))
}

type ProviderRow = (
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
    String,
    i64,
    String,
    String,
    String,
);

fn row_to_provider(row: ProviderRow) -> anyhow::Result<ProviderRecord> {
    Ok(ProviderRecord {
        id: row.0,
        org_id: row.1,
        payload: ProviderPayload {
            display_name: row.2,
            kind: row.3,
            protocol: row.4,
            connection: parse_json_field("connection", &row.5)?,
            mapping: parse_json_field("mapping", &row.6)?,
            target: parse_json_field("target", &row.7)?,
            linking: parse_json_field("linking", &row.8)?,
            session: parse_json_field("session", &row.9)?,
            ui: parse_json_field("ui", &row.10)?,
            enabled: row.11 != 0,
            catalog_ref: parse_json_field("catalog_ref", &row.12).unwrap_or_default(),
        },
        created_at: row.13,
        updated_at: row.14,
    })
}

fn spanner_row_to_provider(row: google_cloud_spanner::row::Row) -> anyhow::Result<ProviderRecord> {
    row_to_provider((
        row.column_by_name::<String>("id")?,
        row.column_by_name::<String>("org_id")?,
        row.column_by_name::<String>("display_name")?,
        row.column_by_name::<String>("kind")?,
        row.column_by_name::<String>("protocol")?,
        row.column_by_name::<String>("connection")?,
        row.column_by_name::<String>("mapping")?,
        row.column_by_name::<String>("target")?,
        row.column_by_name::<String>("linking")?,
        row.column_by_name::<String>("session")?,
        row.column_by_name::<String>("ui")?,
        if row.column_by_name::<bool>("enabled")? {
            1
        } else {
            0
        },
        row.column_by_name::<String>("catalog_ref")?,
        row.column_by_name::<String>("created_at")?,
        row.column_by_name::<String>("updated_at")?,
    ))
}

pub async fn list_providers(scoped: &ScopedDb) -> anyhow::Result<Vec<ProviderRecord>> {
    let enabled = scoped.bool_as_int("enabled");
    let connection = scoped.as_text("connection");
    let mapping = scoped.as_text("mapping");
    let target = scoped.as_text("target");
    let linking = scoped.as_text("linking");
    let session = scoped.as_text("session");
    let ui = scoped.as_text("ui");
    let catalog_ref = scoped.as_text("catalog_ref");
    let created_at = scoped.as_text("created_at");
    let updated_at = scoped.as_text("updated_at");
    let sql = format!(
        "SELECT id, COALESCE(org_id, ''), display_name, kind, protocol, {connection}, {mapping}, {target}, {linking}, {session}, {ui}, {enabled}, {catalog_ref}, {created_at}, {updated_at} \
         FROM providers WHERE instance_id = $1 ORDER BY display_order, display_name"
    );

    let rows: Vec<ProviderRow> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .fetch_all(scoped.pool())
        .await?;

    rows.into_iter().map(row_to_provider).collect()
}

pub async fn list_providers_for(db: &Db, instance_id: &str) -> anyhow::Result<Vec<ProviderRecord>> {
    match db {
        Db::Sql(_) => list_providers(&db.scoped(instance_id.to_string())).await,
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, IFNULL(org_id, '') AS org_id, display_name, kind, protocol, connection, mapping, target, linking, \
                        session, ui, enabled, catalog_ref, CAST(created_at AS STRING) AS created_at, \
                        CAST(updated_at AS STRING) AS updated_at \
                 FROM providers WHERE instance_id = @instance_id ORDER BY display_order, display_name",
            );
            stmt.add_param("instance_id", &instance_id);
            let mut tx = spanner.client().single().await?;
            let mut rows = tx.query(stmt).await?;
            let mut providers = Vec::new();
            while let Some(row) = rows.next().await? {
                providers.push(spanner_row_to_provider(row)?);
            }
            Ok(providers)
        }
    }
}

pub async fn get_provider(scoped: &ScopedDb, id: &str) -> anyhow::Result<Option<ProviderRecord>> {
    let enabled = scoped.bool_as_int("enabled");
    let connection = scoped.as_text("connection");
    let mapping = scoped.as_text("mapping");
    let target = scoped.as_text("target");
    let linking = scoped.as_text("linking");
    let session = scoped.as_text("session");
    let ui = scoped.as_text("ui");
    let catalog_ref = scoped.as_text("catalog_ref");
    let created_at = scoped.as_text("created_at");
    let updated_at = scoped.as_text("updated_at");
    let sql = format!(
        "SELECT id, COALESCE(org_id, ''), display_name, kind, protocol, {connection}, {mapping}, {target}, {linking}, {session}, {ui}, {enabled}, {catalog_ref}, {created_at}, {updated_at} \
         FROM providers WHERE instance_id = $1 AND id = $2"
    );

    let row: Option<ProviderRow> = sqlx::query_as(&sql)
        .bind(scoped.instance_id())
        .bind(id)
        .fetch_optional(scoped.pool())
        .await?;

    row.map(row_to_provider).transpose()
}

pub async fn get_provider_for(
    db: &Db,
    instance_id: &str,
    id: &str,
) -> anyhow::Result<Option<ProviderRecord>> {
    match db {
        Db::Sql(_) => get_provider(&db.scoped(instance_id.to_string()), id).await,
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, IFNULL(org_id, '') AS org_id, display_name, kind, protocol, connection, mapping, target, linking, \
                        session, ui, enabled, catalog_ref, CAST(created_at AS STRING) AS created_at, \
                        CAST(updated_at AS STRING) AS updated_at \
                 FROM providers WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            let mut tx = spanner.client().single().await?;
            let mut rows = tx.query(stmt).await?;
            Ok(match rows.next().await? {
                Some(row) => Some(spanner_row_to_provider(row)?),
                None => None,
            })
        }
    }
}

pub async fn insert_provider(
    scoped: &ScopedDb,
    id: &str,
    org_id: &str,
    provider: &ProviderPayload,
) -> anyhow::Result<()> {
    let sql = format!(
        "INSERT INTO providers (id, instance_id, org_id, display_name, kind, protocol, connection, mapping, target, linking, session, ui, enabled, display_order, catalog_ref) \
         VALUES ($1, $2, $3, $4, $5, $6, {}, {}, {}, {}, {}, {}, $13, $14, {})",
        scoped.json_bind(7),
        scoped.json_bind(8),
        scoped.json_bind(9),
        scoped.json_bind(10),
        scoped.json_bind(11),
        scoped.json_bind(12),
        scoped.json_bind(15),
    );
    sqlx::query(&sql)
        .bind(id)
        .bind(scoped.instance_id())
        .bind(org_id)
        .bind(&provider.display_name)
        .bind(&provider.kind)
        .bind(&provider.protocol)
        .bind(serde_json::to_string(&provider.connection)?)
        .bind(serde_json::to_string(&provider.mapping)?)
        .bind(serde_json::to_string(&provider.target)?)
        .bind(serde_json::to_string(&provider.linking)?)
        .bind(serde_json::to_string(&provider.session)?)
        .bind(serde_json::to_string(&provider.ui)?)
        .bind(provider.enabled)
        .bind(provider.ui.display_order)
        .bind(serde_json::to_string(&provider.catalog_ref)?)
        .execute(scoped.pool())
        .await?;
    Ok(())
}

pub async fn insert_provider_for(
    db: &Db,
    instance_id: &str,
    id: &str,
    org_id: &str,
    provider: &ProviderPayload,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            insert_provider(&db.scoped(instance_id.to_string()), id, org_id, provider).await
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO providers \
                 (id, instance_id, org_id, display_name, kind, protocol, connection, mapping, target, linking, session, ui, enabled, display_order, catalog_ref) \
                 VALUES \
                 (@id, @instance_id, @org_id, @display_name, @kind, @protocol, @connection, @mapping, @target, @linking, @session, @ui, @enabled, @display_order, @catalog_ref)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("org_id", &org_id);
            stmt.add_param("display_name", &provider.display_name);
            stmt.add_param("kind", &provider.kind);
            stmt.add_param("protocol", &provider.protocol);
            stmt.add_param("connection", &serde_json::to_string(&provider.connection)?);
            stmt.add_param("mapping", &serde_json::to_string(&provider.mapping)?);
            stmt.add_param("target", &serde_json::to_string(&provider.target)?);
            stmt.add_param("linking", &serde_json::to_string(&provider.linking)?);
            stmt.add_param("session", &serde_json::to_string(&provider.session)?);
            stmt.add_param("ui", &serde_json::to_string(&provider.ui)?);
            stmt.add_param("enabled", &provider.enabled);
            stmt.add_param("display_order", &provider.ui.display_order);
            stmt.add_param(
                "catalog_ref",
                &serde_json::to_string(&provider.catalog_ref)?,
            );
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
    }
}

pub async fn update_provider(
    scoped: &ScopedDb,
    id: &str,
    provider: &ProviderPayload,
) -> anyhow::Result<bool> {
    let sql = format!(
        "UPDATE providers SET display_name = $1, kind = $2, protocol = $3, connection = {}, mapping = {}, target = {}, linking = {}, session = {}, ui = {}, enabled = $10, display_order = $11, catalog_ref = {}, updated_at = CURRENT_TIMESTAMP \
         WHERE instance_id = $12 AND id = $13",
        scoped.json_bind(4),
        scoped.json_bind(5),
        scoped.json_bind(6),
        scoped.json_bind(7),
        scoped.json_bind(8),
        scoped.json_bind(9),
        scoped.json_bind(12),
    );
    let result = sqlx::query(&sql)
        .bind(&provider.display_name)
        .bind(&provider.kind)
        .bind(&provider.protocol)
        .bind(serde_json::to_string(&provider.connection)?)
        .bind(serde_json::to_string(&provider.mapping)?)
        .bind(serde_json::to_string(&provider.target)?)
        .bind(serde_json::to_string(&provider.linking)?)
        .bind(serde_json::to_string(&provider.session)?)
        .bind(serde_json::to_string(&provider.ui)?)
        .bind(provider.enabled)
        .bind(provider.ui.display_order)
        .bind(serde_json::to_string(&provider.catalog_ref)?)
        .bind(scoped.instance_id())
        .bind(id)
        .execute(scoped.pool())
        .await?;
    Ok(result.rows_affected() > 0)
}

pub async fn update_provider_for(
    db: &Db,
    instance_id: &str,
    id: &str,
    provider: &ProviderPayload,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => update_provider(&db.scoped(instance_id.to_string()), id, provider).await,
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE providers \
                 SET display_name = @display_name, kind = @kind, protocol = @protocol, connection = @connection, \
                     mapping = @mapping, target = @target, linking = @linking, session = @session, ui = @ui, \
                     enabled = @enabled, display_order = @display_order, catalog_ref = @catalog_ref, \
                     updated_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND id = @id",
            );
            stmt.add_param("display_name", &provider.display_name);
            stmt.add_param("kind", &provider.kind);
            stmt.add_param("protocol", &provider.protocol);
            stmt.add_param("connection", &serde_json::to_string(&provider.connection)?);
            stmt.add_param("mapping", &serde_json::to_string(&provider.mapping)?);
            stmt.add_param("target", &serde_json::to_string(&provider.target)?);
            stmt.add_param("linking", &serde_json::to_string(&provider.linking)?);
            stmt.add_param("session", &serde_json::to_string(&provider.session)?);
            stmt.add_param("ui", &serde_json::to_string(&provider.ui)?);
            stmt.add_param("enabled", &provider.enabled);
            stmt.add_param("display_order", &provider.ui.display_order);
            stmt.add_param(
                "catalog_ref",
                &serde_json::to_string(&provider.catalog_ref)?,
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            let (_, affected) = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = stmt.clone();
                    Box::pin(async move { Ok::<i64, SpannerError>(tx.update(stmt).await?) })
                })
                .await?;
            Ok(affected > 0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Db, migrate};

    #[tokio::test]
    async fn provider_round_trip_uses_canonical_shape() {
        let db = Db::open("").await.unwrap();
        migrate::migrate(&db).await.unwrap();
        let scoped = db.scoped_default();
        sqlx::query("INSERT INTO orgs (instance_id, id, name) VALUES ($1, $2, $3)")
            .bind(scoped.instance_id())
            .bind("org-1")
            .bind("Test Org")
            .execute(scoped.pool())
            .await
            .unwrap();
        let provider = ProviderPayload {
            display_name: "Mock OIDC".to_string(),
            kind: "custom".to_string(),
            protocol: "oidc".to_string(),
            connection: ProviderConnection {
                issuer: "https://issuer.example".to_string(),
                client_id: "client".to_string(),
                client_secret: "secret".to_string(),
                scopes: vec!["openid".to_string(), "email".to_string()],
                ..ProviderConnection::default()
            },
            mapping: ProviderMapping {
                claims: HashMap::from([
                    ("email".to_string(), "claims.email".to_string()),
                    ("display_name".to_string(), "claims.name".to_string()),
                ]),
            },
            target: ProviderTarget {
                schema_type: "human_user".to_string(),
                schema_id: String::new(),
            },
            linking: ProviderLinking {
                mode: ProviderLinkingMode::CreateOrLink,
                match_by: ProviderMatchBy::VerifiedEmail,
            },
            ui: ProviderUi { display_order: 7 },
            enabled: true,
            ..ProviderPayload::default()
        };

        insert_provider(&scoped, "provider-1", "org-1", &provider)
            .await
            .unwrap();
        let stored = get_provider(&scoped, "provider-1").await.unwrap().unwrap();
        assert_eq!(stored.payload.display_name, "Mock OIDC");
        assert_eq!(stored.payload.connection.issuer, "https://issuer.example");
        assert_eq!(
            stored.payload.connection.scopes,
            vec!["openid".to_string(), "email".to_string()]
        );
        assert_eq!(stored.payload.ui.display_order, 7);
    }
}
