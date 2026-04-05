use std::collections::BTreeMap;

use anyhow::Context;
use google_cloud_spanner::{
    client::Error as SpannerError, mutation::insert, row::Row, statement::Statement,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Db;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityMetadata {
    pub org_id: String,
    pub metadata_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgSummary {
    pub id: String,
    pub name: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceMetadata {
    pub instance_id: String,
    pub kind: String,
    pub parent_instance_id: Option<String>,
    pub feature_overrides_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleBootstrapData {
    pub counts: BTreeMap<String, i64>,
    pub orgs: Vec<OrgSummary>,
    pub instance: InstanceMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManagedInstanceRecord {
    pub instance_id: String,
    pub state: String,
    pub kind: String,
    pub placement_mode: String,
    pub region_key: Option<String>,
    pub owner_org_id: String,
    pub feature_overrides_json: String,
    pub created_at: String,
    pub updated_at: String,
    pub primary_domain: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainRecord {
    pub domain: String,
    pub is_primary: bool,
    pub state: String,
    pub verified: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateManagedInstanceInput {
    pub instance_id: String,
    pub root_instance_id: String,
    pub owner_org_id: String,
    pub primary_domain: String,
    pub kind: String,
    pub placement_mode: String,
    pub region_key: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ManagedInstancePatch {
    pub state: Option<String>,
    pub placement_mode: Option<String>,
    pub region_key: Option<String>,
    pub feature_overrides_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DomainDeleteOutcome {
    Deleted,
    NotFound,
    PrimaryDomain,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedQueryRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub sql: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedResourceRecord {
    pub id: String,
    pub name: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrgRecord {
    pub id: String,
    pub name: String,
    pub state: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserRecord {
    pub id: String,
    pub org_id: String,
    pub identifier: String,
    pub display_name: String,
    pub user_type: String,
    pub state: String,
    pub schema_id: String,
    pub metadata_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsRecord {
    pub type_: String,
    pub scope: String,
    pub data_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatRecord {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaRegistryRecord {
    pub id: String,
    pub type_: String,
    pub schema_json: String,
    pub version: i64,
    pub is_default: bool,
    pub visibility: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoginFlowRecord {
    pub id: String,
    pub name: String,
    pub strategy: String,
    pub state: String,
    pub is_default: bool,
    pub enabled: bool,
    pub priority: i64,
    pub config_json: String,
    pub audience_json: String,
    pub auth_methods_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRecord {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub hook: String,
    pub action_type: String,
    pub trigger_expr: String,
    pub config_json: String,
    pub priority: i64,
    pub enabled: bool,
    pub fail_open: bool,
    pub metadata_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FingerprintRecord {
    pub id: String,
    pub type_: String,
    pub raw_data_json: String,
    pub created_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRecord {
    pub name: String,
    pub display_name: String,
    pub description: String,
    pub cron: String,
    pub enabled: bool,
    pub last_status: String,
    pub last_error: String,
    pub run_count: i64,
    pub last_rows_removed: i64,
    pub config_json: String,
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub lease_expires_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchRecord {
    pub resource_type: String,
    pub id: String,
    pub title: String,
    pub subtitle: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkedIdentityRecord {
    pub id: String,
    pub user_id: String,
    pub provider_id: String,
    pub external_sub: String,
    pub external_email: String,
    pub raw_claims_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteResolutionRecord {
    pub instance_id: String,
    pub resolved_org_id: Option<String>,
    pub placement_mode: String,
    pub region_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrgUserLinkRecord {
    pub org_id: String,
    pub user_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildInstanceOwnershipRecord {
    pub instance_id: String,
    pub owner_org_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OidcClientRecord {
    pub client_secret: String,
    pub redirect_uris_json: String,
    pub grant_types_json: String,
    pub response_types_json: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OidcAuthRequestRecord {
    pub auth_request_id: String,
    pub user_id: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub nonce: String,
    pub code_challenge: String,
    pub auth_time: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserClaimsRecord {
    pub identifier: String,
    pub display_name: String,
}

pub async fn load_identity_metadata(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Option<IdentityMetadata>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let sql = format!(
                "SELECT org_id, COALESCE({metadata}, '{{}}') FROM users WHERE instance_id = $1 AND id = $2"
            );
            let row: Option<(String, String)> = sqlx::query_as(&sql)
                .bind(scoped.instance_id())
                .bind(user_id)
                .fetch_optional(scoped.pool())
                .await?;
            Ok(row.map(|(org_id, metadata_json)| IdentityMetadata {
                org_id,
                metadata_json,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT org_id, IFNULL(metadata, '{}') AS metadata \
                 FROM users WHERE instance_id = @instance_id AND id = @user_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| IdentityMetadata {
                    org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
                    metadata_json: row.column_by_name::<String>("metadata").unwrap_or_default(),
                }))
        }
    }
}

pub async fn list_active_org_users(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Vec<OrgUserLinkRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let rows = sqlx::query_as::<_, (String, String)>(
                "SELECT org_id, id FROM users \
                 WHERE instance_id = $1 AND state = 'active' \
                 ORDER BY org_id, id",
            )
            .bind(instance_id)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|(org_id, user_id)| OrgUserLinkRecord { org_id, user_id })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT org_id, id FROM users \
                 WHERE instance_id = @instance_id AND state = 'active' \
                 ORDER BY org_id, id",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| OrgUserLinkRecord {
                    org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
                    user_id: row.column_by_name::<String>("id").unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn list_active_child_instance_ownerships(
    db: &Db,
    root_instance_id: &str,
) -> anyhow::Result<Vec<ChildInstanceOwnershipRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(root_instance_id.to_string());
            let rows = sqlx::query_as::<_, (String, String)>(
                "SELECT instance_id, owner_org_id FROM instances \
                 WHERE parent_instance_id = $1 AND state = 'active' \
                 ORDER BY instance_id",
            )
            .bind(root_instance_id)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|(instance_id, owner_org_id)| ChildInstanceOwnershipRecord {
                    instance_id,
                    owner_org_id,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT instance_id, owner_org_id FROM instances \
                 WHERE parent_instance_id = @root_instance_id AND state = 'active' \
                 ORDER BY instance_id",
            );
            stmt.add_param("root_instance_id", &root_instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| ChildInstanceOwnershipRecord {
                    instance_id: row
                        .column_by_name::<String>("instance_id")
                        .unwrap_or_default(),
                    owner_org_id: row
                        .column_by_name::<String>("owner_org_id")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn load_session_user_profile(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Option<(String, String)>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let row: Option<(String, String)> = sqlx::query_as(
                "SELECT identifier, display_name FROM users WHERE instance_id = $1 AND id = $2",
            )
            .bind(scoped.instance_id())
            .bind(user_id)
            .fetch_optional(scoped.pool())
            .await?;
            Ok(row)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT identifier, display_name \
                 FROM users WHERE instance_id = @instance_id AND id = @user_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            Ok(spanner_query_optional(spanner, stmt).await?.map(|row| {
                (
                    row.column_by_name::<String>("identifier")
                        .unwrap_or_default(),
                    row.column_by_name::<String>("display_name")
                        .unwrap_or_default(),
                )
            }))
        }
    }
}

pub async fn update_password_hash(
    db: &Db,
    instance_id: &str,
    user_id: &str,
    credential_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "UPDATE credentials SET data = {} WHERE instance_id = $1 AND user_id = $2 AND type = 'password'",
                scoped.json_bind(3),
            );
            sqlx::query(&sql)
                .bind(scoped.instance_id())
                .bind(user_id)
                .bind(credential_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE credentials SET data = @data \
                 WHERE instance_id = @instance_id AND user_id = @user_id AND type = 'password'",
            );
            stmt.add_param("data", &credential_json);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
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
        }
    }
    Ok(())
}

pub async fn load_instance_metadata(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Option<InstanceMetadata>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let feature_overrides = scoped.as_text("feature_overrides");
            let sql = format!(
                "SELECT instance_id, kind, parent_instance_id, COALESCE({feature_overrides}, '{{}}') \
                 FROM instances WHERE instance_id = $1"
            );
            let row: Option<(String, String, Option<String>, String)> = sqlx::query_as(&sql)
                .bind(scoped.instance_id())
                .fetch_optional(scoped.pool())
                .await?;
            Ok(row.map(
                |(instance_id, kind, parent_instance_id, feature_overrides_json)| {
                    InstanceMetadata {
                        instance_id,
                        kind,
                        parent_instance_id,
                        feature_overrides_json,
                    }
                },
            ))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT instance_id, kind, parent_instance_id, IFNULL(feature_overrides, '{}') AS feature_overrides \
                 FROM instances WHERE instance_id = @instance_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| InstanceMetadata {
                    instance_id: row
                        .column_by_name::<String>("instance_id")
                        .unwrap_or_default(),
                    kind: row.column_by_name::<String>("kind").unwrap_or_default(),
                    parent_instance_id: row
                        .column_by_name::<Option<String>>("parent_instance_id")
                        .unwrap_or(None),
                    feature_overrides_json: row
                        .column_by_name::<String>("feature_overrides")
                        .unwrap_or_else(|_| "{}".to_string()),
                }))
        }
    }
}

pub async fn load_console_bootstrap_data(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<ConsoleBootstrapData> {
    let mut counts = load_entity_counts(db, instance_id).await?;
    let child_count = count_child_instances(db, instance_id).await?;
    counts.insert("instance".to_string(), child_count);

    let orgs = list_orgs(db, instance_id, 50).await?;
    let instance = load_instance_metadata(db, instance_id)
        .await?
        .unwrap_or(InstanceMetadata {
            instance_id: instance_id.to_string(),
            kind: "managed".to_string(),
            parent_instance_id: None,
            feature_overrides_json: "{}".to_string(),
        });

    Ok(ConsoleBootstrapData {
        counts,
        orgs,
        instance,
    })
}

pub async fn load_entity_counts(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<BTreeMap<String, i64>> {
    let queries = [
        (
            "human_user",
            "SELECT COUNT(*) AS total FROM users WHERE instance_id = @instance_id AND user_type = 'human'",
            "SELECT COUNT(*) FROM users WHERE instance_id = $1 AND user_type = 'human'",
        ),
        (
            "service_user",
            "SELECT COUNT(*) AS total FROM users WHERE instance_id = @instance_id AND user_type = 'service'",
            "SELECT COUNT(*) FROM users WHERE instance_id = $1 AND user_type = 'service'",
        ),
        (
            "ai_agent",
            "SELECT COUNT(*) AS total FROM users WHERE instance_id = @instance_id AND user_type = 'ai_agent'",
            "SELECT COUNT(*) FROM users WHERE instance_id = $1 AND user_type = 'ai_agent'",
        ),
        (
            "org",
            "SELECT COUNT(*) AS total FROM orgs WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM orgs WHERE instance_id = $1",
        ),
        (
            "group",
            "SELECT COUNT(*) AS total FROM groups WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM groups WHERE instance_id = $1",
        ),
        (
            "project",
            "SELECT COUNT(*) AS total FROM projects WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM projects WHERE instance_id = $1",
        ),
        (
            "app",
            "SELECT COUNT(*) AS total FROM apps WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM apps WHERE instance_id = $1",
        ),
        (
            "provider",
            "SELECT COUNT(*) AS total FROM providers WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM providers WHERE instance_id = $1",
        ),
        (
            "login_flow",
            "SELECT COUNT(*) AS total FROM login_flows WHERE instance_id = @instance_id",
            "SELECT COUNT(*) FROM login_flows WHERE instance_id = $1",
        ),
    ];

    let mut counts = BTreeMap::new();
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            for (name, _, sql) in queries {
                let count: i64 = sqlx::query_as::<_, (i64,)>(sql)
                    .bind(scoped.instance_id())
                    .fetch_one(scoped.pool())
                    .await
                    .map(|row| row.0)
                    .unwrap_or(0);
                counts.insert(name.to_string(), count);
            }
        }
        Db::Spanner(spanner) => {
            for (name, sql, _) in queries {
                let mut stmt = Statement::new(sql);
                stmt.add_param("instance_id", &instance_id);
                let count = spanner_query_scalar_i64(spanner, stmt).await.unwrap_or(0);
                counts.insert(name.to_string(), count);
            }
        }
    }
    Ok(counts)
}

pub async fn list_admin_instances(
    db: &Db,
    root_instance_id: &str,
    after_instance_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<ManagedInstanceRecord>> {
    list_managed_instances(db, root_instance_id, None, after_instance_id, limit).await
}

pub async fn list_managed_instances(
    db: &Db,
    root_instance_id: &str,
    owner_filter: Option<&str>,
    after_instance_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<ManagedInstanceRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(root_instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let feature_overrides = scoped.as_text("i.feature_overrides");
            let base = format!(
                "SELECT i.instance_id, i.state, i.kind, i.placement_mode, i.region_key, \
                 i.owner_org_id, COALESCE({feature_overrides}, '{{}}'), \
                 {created_at}, {updated_at}, d.domain AS primary_domain \
                 FROM instances i \
                 LEFT JOIN domains d ON d.instance_id = i.instance_id AND d.org_id IS NULL AND d.is_primary = 1 \
                 WHERE i.parent_instance_id = $1",
                created_at = created_at.replace("created_at", "i.created_at"),
                updated_at = updated_at.replace("updated_at", "i.updated_at"),
            );
            let sql = if owner_filter.is_some() {
                format!(
                    "{base} AND i.owner_org_id = $2 AND i.instance_id > $3 ORDER BY i.instance_id LIMIT $4"
                )
            } else {
                format!("{base} AND i.instance_id > $2 ORDER BY i.instance_id LIMIT $3")
            };

            let rows = if let Some(owner_org_id) = owner_filter {
                sqlx::query_as::<
                    _,
                    (
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                    ),
                >(&sql)
                .bind(root_instance_id)
                .bind(owner_org_id)
                .bind(after_instance_id)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?
            } else {
                sqlx::query_as::<
                    _,
                    (
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                        String,
                        String,
                        String,
                        String,
                        Option<String>,
                    ),
                >(&sql)
                .bind(root_instance_id)
                .bind(after_instance_id)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?
            };

            Ok(rows.into_iter().map(instance_from_sql_row).collect())
        }
        Db::Spanner(spanner) => {
            let sql = if owner_filter.is_some() {
                "SELECT i.instance_id, i.state, i.kind, i.placement_mode, i.region_key, \
                        i.owner_org_id, IFNULL(i.feature_overrides, '{}') AS feature_overrides, \
                        CAST(i.created_at AS STRING) AS created_at, CAST(i.updated_at AS STRING) AS updated_at, \
                        d.domain AS primary_domain \
                 FROM instances i \
                 LEFT JOIN domains d ON d.instance_id = i.instance_id AND d.org_id IS NULL AND d.is_primary = TRUE \
                 WHERE i.parent_instance_id = @root_instance_id AND i.owner_org_id = @owner_org_id AND i.instance_id > @after_instance_id \
                 ORDER BY i.instance_id LIMIT @limit"
            } else {
                "SELECT i.instance_id, i.state, i.kind, i.placement_mode, i.region_key, \
                        i.owner_org_id, IFNULL(i.feature_overrides, '{}') AS feature_overrides, \
                        CAST(i.created_at AS STRING) AS created_at, CAST(i.updated_at AS STRING) AS updated_at, \
                        d.domain AS primary_domain \
                 FROM instances i \
                 LEFT JOIN domains d ON d.instance_id = i.instance_id AND d.org_id IS NULL AND d.is_primary = TRUE \
                 WHERE i.parent_instance_id = @root_instance_id AND i.instance_id > @after_instance_id \
                 ORDER BY i.instance_id LIMIT @limit"
            };
            let mut stmt = Statement::new(sql);
            stmt.add_param("root_instance_id", &root_instance_id);
            stmt.add_param("after_instance_id", &after_instance_id);
            stmt.add_param("limit", &limit);
            if let Some(owner_org_id) = owner_filter {
                stmt.add_param("owner_org_id", &owner_org_id);
            }
            let rows = spanner_query_all(spanner, stmt).await?;
            Ok(rows.into_iter().map(instance_from_spanner_row).collect())
        }
    }
}

pub async fn get_managed_instance(
    db: &Db,
    instance_id: &str,
    root_instance_id: &str,
    owner_filter: Option<&str>,
) -> anyhow::Result<Option<ManagedInstanceRecord>> {
    let mut rows = list_managed_instances(db, root_instance_id, owner_filter, "", i64::MAX).await?;
    Ok(rows
        .drain(..)
        .find(|record| record.instance_id == instance_id))
}

pub async fn create_managed_instance(
    db: &Db,
    input: &CreateManagedInstanceInput,
) -> anyhow::Result<ManagedInstanceRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(input.root_instance_id.clone());
            let mut tx = scoped.pool().begin().await?;
            sqlx::query(
                "INSERT INTO instances (instance_id, parent_instance_id, owner_org_id, kind, state, placement_mode, region_key, feature_overrides) \
                 VALUES ($1, $2, $3, $4, 'active', $5, $6, '{}')",
            )
            .bind(&input.instance_id)
            .bind(&input.root_instance_id)
            .bind(&input.owner_org_id)
            .bind(&input.kind)
            .bind(&input.placement_mode)
            .bind(&input.region_key)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                 VALUES ($1, $2, 1, 'active', 0)",
            )
            .bind(&input.primary_domain)
            .bind(&input.instance_id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }
        Db::Spanner(spanner) => {
            let region_key = input.region_key.clone();
            let mutations = vec![
                insert(
                    "instances",
                    &[
                        "instance_id",
                        "parent_instance_id",
                        "owner_org_id",
                        "kind",
                        "state",
                        "placement_mode",
                        "region_key",
                        "feature_overrides",
                    ],
                    &[
                        &input.instance_id,
                        &input.root_instance_id,
                        &input.owner_org_id,
                        &input.kind,
                        &"active",
                        &input.placement_mode,
                        &region_key,
                        &"{}",
                    ],
                ),
                insert(
                    "domains",
                    &["domain", "instance_id", "is_primary", "state", "verified"],
                    &[
                        &input.primary_domain,
                        &input.instance_id,
                        &true,
                        &"active",
                        &false,
                    ],
                ),
            ];
            spanner.client().apply(mutations).await?;
        }
    }

    get_managed_instance(
        db,
        &input.instance_id,
        &input.root_instance_id,
        Some(&input.owner_org_id),
    )
    .await?
    .ok_or_else(|| anyhow::anyhow!("created instance but could not reload it"))
}

pub async fn update_managed_instance(
    db: &Db,
    instance_id: &str,
    root_instance_id: &str,
    owner_filter: Option<&str>,
    patch: &ManagedInstancePatch,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(root_instance_id.to_string());
            let mut sets = Vec::new();
            let mut binds: Vec<String> = Vec::new();
            let mut next = 1usize;

            if let Some(state) = &patch.state {
                sets.push(format!("state = ${next}"));
                binds.push(state.clone());
                next += 1;
            }
            if let Some(placement_mode) = &patch.placement_mode {
                sets.push(format!("placement_mode = ${next}"));
                binds.push(placement_mode.clone());
                next += 1;
            }
            if let Some(region_key) = &patch.region_key {
                sets.push(format!("region_key = ${next}"));
                binds.push(region_key.clone());
                next += 1;
            }
            if let Some(feature_overrides_json) = &patch.feature_overrides_json {
                sets.push(format!("feature_overrides = {}", scoped.json_bind(next)));
                binds.push(feature_overrides_json.clone());
                next += 1;
            }
            if sets.is_empty() {
                return Ok(false);
            }
            sets.push("updated_at = CURRENT_TIMESTAMP".to_string());

            let sql = if owner_filter.is_some() {
                format!(
                    "UPDATE instances SET {} WHERE instance_id = ${} AND parent_instance_id = ${} AND owner_org_id = ${}",
                    sets.join(", "),
                    next,
                    next + 1,
                    next + 2,
                )
            } else {
                format!(
                    "UPDATE instances SET {} WHERE instance_id = ${} AND parent_instance_id = ${}",
                    sets.join(", "),
                    next,
                    next + 1,
                )
            };

            let mut query = sqlx::query(&sql);
            for bind in &binds {
                query = query.bind(bind);
            }
            query = query.bind(instance_id).bind(root_instance_id);
            if let Some(owner_org_id) = owner_filter {
                query = query.bind(owner_org_id);
            }
            Ok(query.execute(scoped.pool()).await?.rows_affected() > 0)
        }
        Db::Spanner(spanner) => {
            let mut sets = Vec::new();
            let mut stmt = Statement::new("");
            if let Some(state) = &patch.state {
                sets.push("state = @state");
                stmt.add_param("state", state);
            }
            if let Some(placement_mode) = &patch.placement_mode {
                sets.push("placement_mode = @placement_mode");
                stmt.add_param("placement_mode", placement_mode);
            }
            if let Some(region_key) = &patch.region_key {
                sets.push("region_key = @region_key");
                stmt.add_param("region_key", region_key);
            }
            if let Some(feature_overrides_json) = &patch.feature_overrides_json {
                sets.push("feature_overrides = @feature_overrides");
                stmt.add_param("feature_overrides", feature_overrides_json);
            }
            if sets.is_empty() {
                return Ok(false);
            }
            sets.push("updated_at = CURRENT_TIMESTAMP()");
            let mut sql = format!(
                "UPDATE instances SET {} WHERE instance_id = @instance_id AND parent_instance_id = @root_instance_id",
                sets.join(", ")
            );
            if owner_filter.is_some() {
                sql.push_str(" AND owner_org_id = @owner_org_id");
            }
            stmt = Statement::new(sql);
            if let Some(state) = &patch.state {
                stmt.add_param("state", state);
            }
            if let Some(placement_mode) = &patch.placement_mode {
                stmt.add_param("placement_mode", placement_mode);
            }
            if let Some(region_key) = &patch.region_key {
                stmt.add_param("region_key", region_key);
            }
            if let Some(feature_overrides_json) = &patch.feature_overrides_json {
                stmt.add_param("feature_overrides", feature_overrides_json);
            }
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("root_instance_id", &root_instance_id);
            if let Some(owner_org_id) = owner_filter {
                stmt.add_param("owner_org_id", &owner_org_id);
            }

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

pub async fn deprovision_managed_instance(
    db: &Db,
    instance_id: &str,
    root_instance_id: &str,
    owner_filter: Option<&str>,
) -> anyhow::Result<bool> {
    let patch = ManagedInstancePatch {
        state: Some("deprovisioning".to_string()),
        ..ManagedInstancePatch::default()
    };
    update_managed_instance(db, instance_id, root_instance_id, owner_filter, &patch).await
}

pub async fn instance_visible(
    db: &Db,
    instance_id: &str,
    root_instance_id: &str,
    owner_filter: Option<&str>,
) -> anyhow::Result<bool> {
    Ok(
        get_managed_instance(db, instance_id, root_instance_id, owner_filter)
            .await?
            .is_some(),
    )
}

pub async fn list_instance_domains(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Vec<DomainRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT domain, {}, state, {}, {created_at}, {updated_at} \
                 FROM domains WHERE instance_id = $1 AND org_id IS NULL \
                 ORDER BY is_primary DESC, domain",
                scoped.bool_as_int("is_primary"),
                scoped.bool_as_int("verified"),
            );
            let rows: Vec<(String, i32, String, i32, String, String)> = sqlx::query_as(&sql)
                .bind(instance_id)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| DomainRecord {
                    domain: row.0,
                    is_primary: row.1 != 0,
                    state: row.2,
                    verified: row.3 != 0,
                    created_at: row.4,
                    updated_at: row.5,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT domain, is_primary, state, verified, CAST(created_at AS STRING) AS created_at, \
                        CAST(updated_at AS STRING) AS updated_at \
                 FROM domains WHERE instance_id = @instance_id AND org_id IS NULL \
                 ORDER BY is_primary DESC, domain",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| DomainRecord {
                    domain: row.column_by_name::<String>("domain").unwrap_or_default(),
                    is_primary: row.column_by_name::<bool>("is_primary").unwrap_or(false),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    verified: row.column_by_name::<bool>("verified").unwrap_or(false),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                    updated_at: row
                        .column_by_name::<String>("updated_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn add_instance_domain(
    db: &Db,
    instance_id: &str,
    domain: &str,
) -> anyhow::Result<DomainRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "INSERT INTO domains (domain, instance_id, is_primary, state, verified) \
                 VALUES ($1, $2, 0, 'active', 0)",
            )
            .bind(domain)
            .bind(instance_id)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mutation = insert(
                "domains",
                &["domain", "instance_id", "is_primary", "state", "verified"],
                &[&domain, &instance_id, &false, &"active", &false],
            );
            spanner.client().apply(vec![mutation]).await?;
        }
    }

    let items = list_instance_domains(db, instance_id).await?;
    items
        .into_iter()
        .find(|item| item.domain == domain)
        .ok_or_else(|| anyhow::anyhow!("created domain but could not reload it"))
}

pub async fn delete_instance_domain(
    db: &Db,
    instance_id: &str,
    domain: &str,
) -> anyhow::Result<DomainDeleteOutcome> {
    let current = match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "SELECT {}, state FROM domains WHERE domain = $1 AND instance_id = $2 AND org_id IS NULL",
                scoped.bool_as_int("is_primary"),
            );
            let row: Option<(i32, String)> = sqlx::query_as(&sql)
                .bind(domain)
                .bind(instance_id)
                .fetch_optional(scoped.pool())
                .await?;
            row.map(|(is_primary, state)| (is_primary != 0, state))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT is_primary, state FROM domains \
                 WHERE domain = @domain AND instance_id = @instance_id AND org_id IS NULL LIMIT 1",
            );
            stmt.add_param("domain", &domain);
            stmt.add_param("instance_id", &instance_id);
            spanner_query_optional(spanner, stmt).await?.map(|row| {
                (
                    row.column_by_name::<bool>("is_primary").unwrap_or(false),
                    row.column_by_name::<String>("state").unwrap_or_default(),
                )
            })
        }
    };

    let Some((is_primary, _)) = current else {
        return Ok(DomainDeleteOutcome::NotFound);
    };
    if is_primary {
        return Ok(DomainDeleteOutcome::PrimaryDomain);
    }

    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "DELETE FROM domains WHERE domain = $1 AND instance_id = $2 AND org_id IS NULL",
            )
            .bind(domain)
            .bind(instance_id)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "DELETE FROM domains WHERE domain = @domain AND instance_id = @instance_id AND org_id IS NULL",
            );
            stmt.add_param("domain", &domain);
            stmt.add_param("instance_id", &instance_id);
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
        }
    }
    Ok(DomainDeleteOutcome::Deleted)
}

pub async fn list_saved_queries(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Vec<SavedQueryRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let created_at = scoped.as_text("created_at");
            let sql = format!(
                "SELECT id, name, COALESCE(description, ''), sql_text, {created_at} \
                 FROM saved_queries WHERE instance_id = $1 ORDER BY name"
            );
            let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(&sql)
                .bind(instance_id)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| SavedQueryRecord {
                    id: row.0,
                    name: row.1,
                    description: row.2,
                    sql: row.3,
                    created_at: row.4,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name, IFNULL(description, '') AS description, sql_text, \
                        CAST(created_at AS STRING) AS created_at \
                 FROM saved_queries WHERE instance_id = @instance_id ORDER BY name",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| SavedQueryRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    description: row
                        .column_by_name::<String>("description")
                        .unwrap_or_default(),
                    sql: row.column_by_name::<String>("sql_text").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn create_saved_query(
    db: &Db,
    instance_id: &str,
    id: &str,
    name: &str,
    description: &str,
    sql_text: &str,
) -> anyhow::Result<SavedQueryRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "INSERT INTO saved_queries (id, instance_id, name, description, sql_text) \
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(id)
            .bind(instance_id)
            .bind(name)
            .bind(description)
            .bind(sql_text)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mutation = insert(
                "saved_queries",
                &["id", "instance_id", "name", "description", "sql_text"],
                &[&id, &instance_id, &name, &description, &sql_text],
            );
            spanner.client().apply(vec![mutation]).await?;
        }
    }

    list_saved_queries(db, instance_id)
        .await?
        .into_iter()
        .find(|query| query.id == id)
        .ok_or_else(|| anyhow::anyhow!("saved query created but could not be reloaded"))
}

pub async fn delete_saved_query(db: &Db, instance_id: &str, id: &str) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(
                sqlx::query("DELETE FROM saved_queries WHERE instance_id = $1 AND id = $2")
                    .bind(instance_id)
                    .bind(id)
                    .execute(scoped.pool())
                    .await?
                    .rows_affected()
                    > 0,
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id FROM saved_queries WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            if spanner_query_optional(spanner, stmt).await?.is_none() {
                return Ok(false);
            }
            let mut delete_stmt = Statement::new(
                "DELETE FROM saved_queries WHERE instance_id = @instance_id AND id = @id",
            );
            delete_stmt.add_param("instance_id", &instance_id);
            delete_stmt.add_param("id", &id);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = delete_stmt.clone();
                    Box::pin(async move {
                        tx.update(stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
            Ok(true)
        }
    }
}

pub async fn first_org_id(db: &Db, instance_id: &str) -> anyhow::Result<Option<String>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query_as::<_, (String,)>(
                "SELECT id FROM orgs WHERE instance_id = $1 ORDER BY created_at ASC LIMIT 1",
            )
            .bind(instance_id)
            .fetch_optional(scoped.pool())
            .await?
            .map(|row| row.0))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id FROM orgs WHERE instance_id = @instance_id ORDER BY created_at ASC LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| row.column_by_name::<String>("id").unwrap_or_default()))
        }
    }
}

pub async fn delete_provider(db: &Db, instance_id: &str, id: &str) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(
                sqlx::query("DELETE FROM providers WHERE instance_id = $1 AND id = $2")
                    .bind(instance_id)
                    .bind(id)
                    .execute(scoped.pool())
                    .await?
                    .rows_affected()
                    > 0,
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id FROM providers WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            if spanner_query_optional(spanner, stmt).await?.is_none() {
                return Ok(false);
            }
            let mut delete_stmt = Statement::new(
                "DELETE FROM providers WHERE instance_id = @instance_id AND id = @id",
            );
            delete_stmt.add_param("instance_id", &instance_id);
            delete_stmt.add_param("id", &id);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = delete_stmt.clone();
                    Box::pin(async move {
                        tx.update(stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
            Ok(true)
        }
    }
}

pub async fn create_named_resource(
    db: &Db,
    instance_id: &str,
    table: &'static str,
    id: &str,
    name: &str,
) -> anyhow::Result<NamedResourceRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO {table} (id, instance_id, name, state) VALUES ($1, $2, $3, 'active')"
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(name)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(&format!(
                "INSERT INTO {table} (id, instance_id, name, state) \
                 VALUES (@id, @instance_id, @name, 'active')"
            ));
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("name", &name);
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
        }
    }

    get_named_resource(db, instance_id, table, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("created {table} row but could not reload it"))
}

pub async fn get_named_resource(
    db: &Db,
    instance_id: &str,
    table: &'static str,
    id: &str,
) -> anyhow::Result<Option<NamedResourceRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, name, state, {created_at}, {updated_at} \
                 FROM {table} WHERE instance_id = $1 AND id = $2"
            );
            Ok(
                sqlx::query_as::<_, (String, String, String, String, String)>(&sql)
                    .bind(instance_id)
                    .bind(id)
                    .fetch_optional(scoped.pool())
                    .await?
                    .map(|row| NamedResourceRecord {
                        id: row.0,
                        name: row.1,
                        state: row.2,
                        created_at: row.3,
                        updated_at: row.4,
                    }),
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(&format!(
                "SELECT id, name, state, CAST(created_at AS STRING) AS created_at, \
                        CAST(updated_at AS STRING) AS updated_at \
                 FROM {table} WHERE instance_id = @instance_id AND id = @id LIMIT 1"
            ));
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| NamedResourceRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
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

pub async fn list_named_resources(
    db: &Db,
    instance_id: &str,
    table: &'static str,
    after_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<NamedResourceRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, name, state, {created_at}, {updated_at} \
                 FROM {table} WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
            );
            let rows = sqlx::query_as::<_, (String, String, String, String, String)>(&sql)
                .bind(instance_id)
                .bind(after_id)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| NamedResourceRecord {
                    id: row.0,
                    name: row.1,
                    state: row.2,
                    created_at: row.3,
                    updated_at: row.4,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(&format!(
                "SELECT id, name, state, CAST(created_at AS STRING) AS created_at, \
                        CAST(updated_at AS STRING) AS updated_at \
                 FROM {table} WHERE instance_id = @instance_id AND id > @after_id \
                 ORDER BY id LIMIT @limit"
            ));
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| NamedResourceRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                    updated_at: row
                        .column_by_name::<String>("updated_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn update_named_resource_name(
    db: &Db,
    instance_id: &str,
    table: &'static str,
    id: &str,
    name: &str,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "UPDATE {table} SET name = $1, updated_at = CURRENT_TIMESTAMP \
                 WHERE instance_id = $2 AND id = $3"
            );
            Ok(sqlx::query(&sql)
                .bind(name)
                .bind(instance_id)
                .bind(id)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(&format!(
                "UPDATE {table} SET name = @name, updated_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND id = @id"
            ));
            stmt.add_param("name", &name);
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

pub async fn delete_instance_row(
    db: &Db,
    instance_id: &str,
    table: &'static str,
    id: &str,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!("DELETE FROM {table} WHERE instance_id = $1 AND id = $2");
            Ok(sqlx::query(&sql)
                .bind(instance_id)
                .bind(id)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut exists_stmt = Statement::new(&format!(
                "SELECT id FROM {table} WHERE instance_id = @instance_id AND id = @id LIMIT 1"
            ));
            exists_stmt.add_param("instance_id", &instance_id);
            exists_stmt.add_param("id", &id);
            if spanner_query_optional(spanner, exists_stmt)
                .await?
                .is_none()
            {
                return Ok(false);
            }
            let mut delete_stmt = Statement::new(&format!(
                "DELETE FROM {table} WHERE instance_id = @instance_id AND id = @id"
            ));
            delete_stmt.add_param("instance_id", &instance_id);
            delete_stmt.add_param("id", &id);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = delete_stmt.clone();
                    Box::pin(async move {
                        tx.update(stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
            Ok(true)
        }
    }
}

pub async fn create_org(
    db: &Db,
    instance_id: &str,
    id: &str,
    name: &str,
    metadata_json: &str,
) -> anyhow::Result<OrgRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO orgs (id, instance_id, name, state, metadata) \
                 VALUES ($1, $2, $3, 'active', {})",
                scoped.json_bind(4),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(name)
                .bind(metadata_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO orgs (id, instance_id, name, state, metadata) \
                 VALUES (@id, @instance_id, @name, 'active', @metadata)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("name", &name);
            stmt.add_param("metadata", &metadata_json);
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
        }
    }
    get_org(db, instance_id, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("created org but could not reload it"))
}

pub async fn get_org(db: &Db, instance_id: &str, id: &str) -> anyhow::Result<Option<OrgRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                 FROM orgs WHERE instance_id = $1 AND id = $2"
            );
            Ok(
                sqlx::query_as::<_, (String, String, String, String, String, String)>(&sql)
                    .bind(instance_id)
                    .bind(id)
                    .fetch_optional(scoped.pool())
                    .await?
                    .map(|row| OrgRecord {
                        id: row.0,
                        name: row.1,
                        state: row.2,
                        metadata_json: row.3,
                        created_at: row.4,
                        updated_at: row.5,
                    }),
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name, state, IFNULL(metadata, '{}') AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM orgs WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| OrgRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    metadata_json: row.column_by_name::<String>("metadata").unwrap_or_default(),
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

pub async fn list_org_records(
    db: &Db,
    instance_id: &str,
    after_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<OrgRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, name, state, COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                 FROM orgs WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
            );
            let rows = sqlx::query_as::<_, (String, String, String, String, String, String)>(&sql)
                .bind(instance_id)
                .bind(after_id)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| OrgRecord {
                    id: row.0,
                    name: row.1,
                    state: row.2,
                    metadata_json: row.3,
                    created_at: row.4,
                    updated_at: row.5,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name, state, IFNULL(metadata, '{}') AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM orgs WHERE instance_id = @instance_id AND id > @after_id ORDER BY id LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| OrgRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                    metadata_json: row.column_by_name::<String>("metadata").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                    updated_at: row
                        .column_by_name::<String>("updated_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn update_org(
    db: &Db,
    instance_id: &str,
    id: &str,
    name: Option<&str>,
    state: Option<&str>,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let mut sets = Vec::new();
            let mut binds: Vec<String> = Vec::new();
            let mut next = 1usize;
            if let Some(name) = name {
                sets.push(format!("name = {}", scoped.placeholder(next)));
                binds.push(name.to_string());
                next += 1;
            }
            if let Some(state) = state {
                sets.push(format!("state = {}", scoped.placeholder(next)));
                binds.push(state.to_string());
                next += 1;
            }
            if sets.is_empty() {
                return Ok(false);
            }
            sets.push("updated_at = CURRENT_TIMESTAMP".to_string());
            let sql = format!(
                "UPDATE orgs SET {} WHERE instance_id = {} AND id = {}",
                sets.join(", "),
                scoped.placeholder(next),
                scoped.placeholder(next + 1),
            );
            let mut query = sqlx::query(&sql);
            for bind in &binds {
                query = query.bind(bind);
            }
            Ok(query
                .bind(instance_id)
                .bind(id)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut sets = Vec::new();
            let mut stmt = Statement::new("");
            if let Some(name) = name {
                sets.push("name = @name");
                stmt.add_param("name", &name);
            }
            if let Some(state) = state {
                sets.push("state = @state");
                stmt.add_param("state", &state);
            }
            if sets.is_empty() {
                return Ok(false);
            }
            let sql = format!(
                "UPDATE orgs SET {}, updated_at = CURRENT_TIMESTAMP() WHERE instance_id = @instance_id AND id = @id",
                sets.join(", ")
            );
            stmt = Statement::new(sql);
            if let Some(name) = name {
                stmt.add_param("name", &name);
            }
            if let Some(state) = state {
                stmt.add_param("state", &state);
            }
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

pub async fn create_user(
    db: &Db,
    instance_id: &str,
    id: &str,
    org_id: &str,
    identifier: &str,
    display_name: &str,
    schema_id: &str,
    metadata_json: &str,
) -> anyhow::Result<UserRecord> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, schema_id, metadata) \
                 VALUES ($1, $2, $3, $4, $5, 'human', 'active', $6, {})",
                scoped.json_bind(7),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(org_id)
                .bind(identifier)
                .bind(display_name)
                .bind(schema_id)
                .bind(metadata_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO users (id, instance_id, org_id, identifier, display_name, user_type, state, schema_id, metadata) \
                 VALUES (@id, @instance_id, @org_id, @identifier, @display_name, 'human', 'active', @schema_id, @metadata)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("org_id", &org_id);
            stmt.add_param("identifier", &identifier);
            stmt.add_param("display_name", &display_name);
            stmt.add_param("schema_id", &schema_id);
            stmt.add_param("metadata", &metadata_json);
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
        }
    }
    get_user(db, instance_id, id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("created user but could not reload it"))
}

pub async fn get_user(db: &Db, instance_id: &str, id: &str) -> anyhow::Result<Option<UserRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, \
                        COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                 FROM users WHERE instance_id = $1 AND id = $2"
            );
            Ok(sqlx::query_as::<
                _,
                (
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
                ),
            >(&sql)
            .bind(instance_id)
            .bind(id)
            .fetch_optional(scoped.pool())
            .await?
            .map(|row| UserRecord {
                id: row.0,
                org_id: row.1,
                identifier: row.2,
                display_name: row.3,
                user_type: row.4,
                state: row.5,
                schema_id: row.6,
                metadata_json: row.7,
                created_at: row.8,
                updated_at: row.9,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, \
                        IFNULL(metadata, '{}') AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM users WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| UserRecord {
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
                    metadata_json: row.column_by_name::<String>("metadata").unwrap_or_default(),
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

pub async fn list_users(
    db: &Db,
    instance_id: &str,
    after_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<UserRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, '{{}}', {created_at}, {updated_at} \
                 FROM users WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
            );
            let rows = sqlx::query_as::<
                _,
                (
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
                ),
            >(&sql)
            .bind(instance_id)
            .bind(after_id)
            .bind(limit)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|row| UserRecord {
                    id: row.0,
                    org_id: row.1,
                    identifier: row.2,
                    display_name: row.3,
                    user_type: row.4,
                    state: row.5,
                    schema_id: row.6,
                    metadata_json: row.7,
                    created_at: row.8,
                    updated_at: row.9,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, '{}' AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM users WHERE instance_id = @instance_id AND id > @after_id ORDER BY id LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| UserRecord {
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
                    metadata_json: "{}".to_string(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                    updated_at: row
                        .column_by_name::<String>("updated_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn update_user(
    db: &Db,
    instance_id: &str,
    id: &str,
    display_name: Option<&str>,
    state: Option<&str>,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let mut sets = Vec::new();
            let mut binds: Vec<String> = Vec::new();
            let mut next = 1usize;
            if let Some(display_name) = display_name {
                sets.push(format!("display_name = {}", scoped.placeholder(next)));
                binds.push(display_name.to_string());
                next += 1;
            }
            if let Some(state) = state {
                sets.push(format!("state = {}", scoped.placeholder(next)));
                binds.push(state.to_string());
                next += 1;
            }
            if sets.is_empty() {
                return Ok(false);
            }
            sets.push("updated_at = CURRENT_TIMESTAMP".to_string());
            let sql = format!(
                "UPDATE users SET {} WHERE instance_id = {} AND id = {}",
                sets.join(", "),
                scoped.placeholder(next),
                scoped.placeholder(next + 1),
            );
            let mut query = sqlx::query(&sql);
            for bind in &binds {
                query = query.bind(bind);
            }
            Ok(query
                .bind(instance_id)
                .bind(id)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut sets = Vec::new();
            let mut stmt = Statement::new("");
            if let Some(display_name) = display_name {
                sets.push("display_name = @display_name");
                stmt.add_param("display_name", &display_name);
            }
            if let Some(state) = state {
                sets.push("state = @state");
                stmt.add_param("state", &state);
            }
            if sets.is_empty() {
                return Ok(false);
            }
            let sql = format!(
                "UPDATE users SET {}, updated_at = CURRENT_TIMESTAMP() WHERE instance_id = @instance_id AND id = @id",
                sets.join(", ")
            );
            stmt = Statement::new(sql);
            if let Some(display_name) = display_name {
                stmt.add_param("display_name", &display_name);
            }
            if let Some(state) = state {
                stmt.add_param("state", &state);
            }
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

pub async fn replace_password_credential(
    db: &Db,
    instance_id: &str,
    user_id: &str,
    credential_id: &str,
    credential_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query(
                "DELETE FROM credentials WHERE instance_id = $1 AND user_id = $2 AND type = 'password'",
            )
            .bind(instance_id)
            .bind(user_id)
            .execute(scoped.pool())
            .await?;
            let sql = format!(
                "INSERT INTO credentials (id, instance_id, user_id, type, data) VALUES ($1, $2, $3, 'password', {})",
                scoped.json_bind(4),
            );
            sqlx::query(&sql)
                .bind(credential_id)
                .bind(instance_id)
                .bind(user_id)
                .bind(credential_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut delete_stmt = Statement::new(
                "DELETE FROM credentials WHERE instance_id = @instance_id AND user_id = @user_id AND type = 'password'",
            );
            delete_stmt.add_param("instance_id", &instance_id);
            delete_stmt.add_param("user_id", &user_id);
            let mut insert_stmt = Statement::new(
                "INSERT INTO credentials (id, instance_id, user_id, type, data) \
                 VALUES (@id, @instance_id, @user_id, 'password', @data)",
            );
            insert_stmt.add_param("id", &credential_id);
            insert_stmt.add_param("instance_id", &instance_id);
            insert_stmt.add_param("user_id", &user_id);
            insert_stmt.add_param("data", &credential_json);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let delete_stmt = delete_stmt.clone();
                    let insert_stmt = insert_stmt.clone();
                    Box::pin(async move {
                        tx.update(delete_stmt).await?;
                        tx.update(insert_stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
        }
    }
    Ok(())
}

pub async fn get_settings_record(
    db: &Db,
    instance_id: &str,
    type_: &str,
) -> anyhow::Result<Option<SettingsRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "SELECT type, scope, {} FROM settings \
                 WHERE instance_id = $1 AND type = $2 \
                 ORDER BY CASE scope WHEN 'app' THEN 1 WHEN 'org' THEN 2 ELSE 3 END LIMIT 1",
                scoped.as_text("data"),
            );
            Ok(sqlx::query_as::<_, (String, String, String)>(&sql)
                .bind(instance_id)
                .bind(type_)
                .fetch_optional(scoped.pool())
                .await?
                .map(|row| SettingsRecord {
                    type_: row.0,
                    scope: row.1,
                    data_json: row.2,
                }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT type, scope, IFNULL(data, '{}') AS data \
                 FROM settings WHERE instance_id = @instance_id AND type = @type \
                 ORDER BY CASE scope WHEN 'app' THEN 1 WHEN 'org' THEN 2 ELSE 3 END LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("type", &type_);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| SettingsRecord {
                    type_: row.column_by_name::<String>("type").unwrap_or_default(),
                    scope: row.column_by_name::<String>("scope").unwrap_or_default(),
                    data_json: row.column_by_name::<String>("data").unwrap_or_default(),
                }))
        }
    }
}

pub async fn put_instance_settings(
    db: &Db,
    instance_id: &str,
    id: &str,
    type_: &str,
    data_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) \
                 VALUES ($1, $2, $3, 'instance', '', {}) \
                 ON CONFLICT(instance_id, type, scope, scope_id) DO UPDATE SET data = {}, updated_at = CURRENT_TIMESTAMP",
                scoped.json_bind(4),
                scoped.json_bind(4),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(type_)
                .bind(data_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut exists_stmt = Statement::new(
                "SELECT id FROM settings WHERE instance_id = @instance_id AND type = @type AND scope = 'instance' AND scope_id = '' LIMIT 1",
            );
            exists_stmt.add_param("instance_id", &instance_id);
            exists_stmt.add_param("type", &type_);
            let existing = spanner_query_optional(spanner, exists_stmt).await?;
            if let Some(row) = existing {
                let existing_id = row.column_by_name::<String>("id").unwrap_or_default();
                let mut update_stmt = Statement::new(
                    "UPDATE settings SET data = @data, updated_at = CURRENT_TIMESTAMP() \
                     WHERE instance_id = @instance_id AND id = @id",
                );
                update_stmt.add_param("data", &data_json);
                update_stmt.add_param("instance_id", &instance_id);
                update_stmt.add_param("id", &existing_id);
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = update_stmt.clone();
                        Box::pin(async move {
                            tx.update(stmt).await?;
                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await?;
            } else {
                let mut insert_stmt = Statement::new(
                    "INSERT INTO settings (id, instance_id, type, scope, scope_id, data) \
                     VALUES (@id, @instance_id, @type, 'instance', '', @data)",
                );
                insert_stmt.add_param("id", &id);
                insert_stmt.add_param("instance_id", &instance_id);
                insert_stmt.add_param("type", &type_);
                insert_stmt.add_param("data", &data_json);
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let stmt = insert_stmt.clone();
                        Box::pin(async move {
                            tx.update(stmt).await?;
                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await?;
            }
        }
    }
    Ok(())
}

pub async fn delete_settings_record(db: &Db, instance_id: &str, type_: &str) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            sqlx::query("DELETE FROM settings WHERE instance_id = $1 AND type = $2")
                .bind(instance_id)
                .bind(type_)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "DELETE FROM settings WHERE instance_id = @instance_id AND type = @type",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("type", &type_);
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
        }
    }
    Ok(())
}

pub async fn create_pat(
    db: &Db,
    instance_id: &str,
    id: &str,
    user_id: &str,
    name: &str,
    token_hash: &str,
    scopes_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) \
                 VALUES ($1, $2, 'pat', $3, $4, $5, {})",
                scoped.json_bind(6),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(token_hash)
                .bind(user_id)
                .bind(name)
                .bind(scopes_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO tokens (id, instance_id, type, token_hash, user_id, name, scopes) \
                 VALUES (@id, @instance_id, 'pat', @token_hash, @user_id, @name, @scopes)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("token_hash", &token_hash);
            stmt.add_param("user_id", &user_id);
            stmt.add_param("name", &name);
            stmt.add_param("scopes", &scopes_json);
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
        }
    }
    Ok(())
}

pub async fn list_pats_for_instance(db: &Db, instance_id: &str) -> anyhow::Result<Vec<PatRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let created_at = scoped.as_text("created_at");
            let sql = format!(
                "SELECT id, user_id, COALESCE(name,''), {created_at} \
                 FROM tokens WHERE instance_id = $1 AND type = 'pat' AND revoked_at IS NULL \
                 ORDER BY created_at DESC"
            );
            let rows = sqlx::query_as::<_, (String, String, String, String)>(&sql)
                .bind(instance_id)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| PatRecord {
                    id: row.0,
                    user_id: row.1,
                    name: row.2,
                    created_at: row.3,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, user_id, IFNULL(name, '') AS name, CAST(created_at AS STRING) AS created_at \
                 FROM tokens WHERE instance_id = @instance_id AND type = 'pat' AND revoked_at IS NULL \
                 ORDER BY created_at DESC",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| PatRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    user_id: row.column_by_name::<String>("user_id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn revoke_pat(db: &Db, instance_id: &str, id: &str) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query(
                "UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP WHERE instance_id = $1 AND id = $2 AND type = 'pat'",
            )
            .bind(instance_id)
            .bind(id)
            .execute(scoped.pool())
            .await?
            .rows_affected()
            > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE tokens SET revoked_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND id = @id AND type = 'pat'",
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

pub async fn list_actions(db: &Db, instance_id: &str) -> anyhow::Result<Vec<ActionRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let config = scoped.as_text("config");
            let metadata = scoped.as_text("metadata");
            let created_at = scoped.as_text("created_at");
            let enabled = scoped.bool_as_int("enabled");
            let fail_open = scoped.bool_as_int("fail_open");
            let sql = format!(
                "SELECT id, org_id, name, hook, action_type, COALESCE(trigger_expr, 'true'), \
                        COALESCE({config}, '{{}}'), priority, {enabled}, {fail_open}, \
                        COALESCE({metadata}, '{{}}'), {created_at} \
                 FROM actions WHERE instance_id = $1 ORDER BY priority, name"
            );
            let rows = sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    i64,
                    i64,
                    i64,
                    String,
                    String,
                ),
            >(&sql)
            .bind(instance_id)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows.into_iter().map(action_from_sql_row).collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, org_id, name, hook, action_type, IFNULL(trigger_expr, 'true') AS trigger_expr, \
                        IFNULL(config, '{}') AS config, priority, enabled, fail_open, \
                        IFNULL(metadata, '{}') AS metadata, CAST(created_at AS STRING) AS created_at \
                 FROM actions WHERE instance_id = @instance_id ORDER BY priority, name",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(action_from_spanner_row)
                .collect())
        }
    }
}

pub async fn get_action(
    db: &Db,
    instance_id: &str,
    id: &str,
) -> anyhow::Result<Option<ActionRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let config = scoped.as_text("config");
            let metadata = scoped.as_text("metadata");
            let created_at = scoped.as_text("created_at");
            let enabled = scoped.bool_as_int("enabled");
            let fail_open = scoped.bool_as_int("fail_open");
            let sql = format!(
                "SELECT id, org_id, name, hook, action_type, COALESCE(trigger_expr, 'true'), \
                        COALESCE({config}, '{{}}'), priority, {enabled}, {fail_open}, \
                        COALESCE({metadata}, '{{}}'), {created_at} \
                 FROM actions WHERE instance_id = $1 AND id = $2"
            );
            Ok(sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    String,
                    i64,
                    i64,
                    i64,
                    String,
                    String,
                ),
            >(&sql)
            .bind(instance_id)
            .bind(id)
            .fetch_optional(scoped.pool())
            .await?
            .map(action_from_sql_row))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, org_id, name, hook, action_type, IFNULL(trigger_expr, 'true') AS trigger_expr, \
                        IFNULL(config, '{}') AS config, priority, enabled, fail_open, \
                        IFNULL(metadata, '{}') AS metadata, CAST(created_at AS STRING) AS created_at \
                 FROM actions WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(action_from_spanner_row))
        }
    }
}

pub async fn upsert_catalog_action(
    db: &Db,
    instance_id: &str,
    action_id: &str,
    org_id: &str,
    name: &str,
    hook: &str,
    action_type: &str,
    trigger_expr: &str,
    config_json: &str,
    priority: i64,
    enabled: bool,
    fail_open: bool,
    metadata_json: &str,
) -> anyhow::Result<String> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let existing: Option<(String,)> = sqlx::query_as(
                "SELECT id FROM actions WHERE instance_id = $1 AND org_id = $2 AND name = $3",
            )
            .bind(instance_id)
            .bind(org_id)
            .bind(name)
            .fetch_optional(scoped.pool())
            .await?;
            let id = if let Some((existing_id,)) = existing {
                let sql = format!(
                    "UPDATE actions SET hook = $1, action_type = $2, trigger_expr = $3, config = {}, \
                     priority = $4, enabled = $5, fail_open = $6, metadata = {}, updated_at = CURRENT_TIMESTAMP \
                     WHERE id = $7 AND instance_id = $8",
                    scoped.json_bind(8),
                    scoped.json_bind(9),
                );
                sqlx::query(&sql)
                    .bind(hook)
                    .bind(action_type)
                    .bind(trigger_expr)
                    .bind(priority)
                    .bind(enabled)
                    .bind(fail_open)
                    .bind(&existing_id)
                    .bind(instance_id)
                    .bind(config_json)
                    .bind(metadata_json)
                    .execute(scoped.pool())
                    .await?;
                existing_id
            } else {
                let sql = format!(
                    "INSERT INTO actions (id, instance_id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, metadata) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, {}, $8, $9, $10, {})",
                    scoped.json_bind(11),
                    scoped.json_bind(12),
                );
                sqlx::query(&sql)
                    .bind(action_id)
                    .bind(instance_id)
                    .bind(org_id)
                    .bind(name)
                    .bind(hook)
                    .bind(action_type)
                    .bind(trigger_expr)
                    .bind(priority)
                    .bind(enabled)
                    .bind(fail_open)
                    .bind(config_json)
                    .bind(metadata_json)
                    .execute(scoped.pool())
                    .await?;
                action_id.to_string()
            };
            Ok(id)
        }
        Db::Spanner(spanner) => {
            let mut find_stmt = Statement::new(
                "SELECT id FROM actions WHERE instance_id = @instance_id AND org_id = @org_id AND name = @name LIMIT 1",
            );
            find_stmt.add_param("instance_id", &instance_id);
            find_stmt.add_param("org_id", &org_id);
            find_stmt.add_param("name", &name);
            if let Some(row) = spanner_query_optional(spanner, find_stmt).await? {
                let existing_id = row.column_by_name::<String>("id").unwrap_or_default();
                let mut stmt = Statement::new(
                    "UPDATE actions SET hook = @hook, action_type = @action_type, trigger_expr = @trigger_expr, \
                     config = @config, priority = @priority, enabled = @enabled, fail_open = @fail_open, \
                     metadata = @metadata, updated_at = CURRENT_TIMESTAMP() \
                     WHERE instance_id = @instance_id AND id = @id",
                );
                stmt.add_param("hook", &hook);
                stmt.add_param("action_type", &action_type);
                stmt.add_param("trigger_expr", &trigger_expr);
                stmt.add_param("config", &config_json);
                stmt.add_param("priority", &priority);
                stmt.add_param("enabled", &enabled);
                stmt.add_param("fail_open", &fail_open);
                stmt.add_param("metadata", &metadata_json);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("id", &existing_id);
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
                Ok(existing_id)
            } else {
                let mut stmt = Statement::new(
                    "INSERT INTO actions (id, instance_id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, metadata) \
                     VALUES (@id, @instance_id, @org_id, @name, @hook, @action_type, @trigger_expr, @config, @priority, @enabled, @fail_open, @metadata)",
                );
                stmt.add_param("id", &action_id);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("org_id", &org_id);
                stmt.add_param("name", &name);
                stmt.add_param("hook", &hook);
                stmt.add_param("action_type", &action_type);
                stmt.add_param("trigger_expr", &trigger_expr);
                stmt.add_param("config", &config_json);
                stmt.add_param("priority", &priority);
                stmt.add_param("enabled", &enabled);
                stmt.add_param("fail_open", &fail_open);
                stmt.add_param("metadata", &metadata_json);
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
                Ok(action_id.to_string())
            }
        }
    }
}

pub async fn list_fingerprints(
    db: &Db,
    instance_id: &str,
    after_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<FingerprintRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let created_at = scoped.as_text("created_at");
            let sql = format!(
                "SELECT id, type, raw_data, {created_at} \
                 FROM fingerprints WHERE instance_id = $1 AND id > $2 ORDER BY id LIMIT $3"
            );
            let rows = sqlx::query_as::<_, (String, String, String, String)>(&sql)
                .bind(instance_id)
                .bind(after_id)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await?;
            Ok(rows
                .into_iter()
                .map(|row| FingerprintRecord {
                    id: row.0,
                    type_: row.1,
                    raw_data_json: row.2,
                    created_at: row.3,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, type, raw_data, CAST(created_at AS STRING) AS created_at \
                 FROM fingerprints WHERE instance_id = @instance_id AND id > @after_id ORDER BY id LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| FingerprintRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    type_: row.column_by_name::<String>("type").unwrap_or_default(),
                    raw_data_json: row.column_by_name::<String>("raw_data").unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn upsert_fingerprint(
    db: &Db,
    instance_id: &str,
    id: &str,
    type_: &str,
    raw_data_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO fingerprints (id, instance_id, type, raw_data, created_at) \
                 VALUES ($1, $2, $3, {}, {}) \
                 ON CONFLICT (id) DO UPDATE SET raw_data = excluded.raw_data, type = excluded.type",
                scoped.json_bind(4),
                scoped.timestamp_now(),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(type_)
                .bind(raw_data_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut exists_stmt =
                Statement::new("SELECT id FROM fingerprints WHERE id = @id LIMIT 1");
            exists_stmt.add_param("id", &id);
            if spanner_query_optional(spanner, exists_stmt)
                .await?
                .is_some()
            {
                let mut stmt = Statement::new(
                    "UPDATE fingerprints SET raw_data = @raw_data, type = @type WHERE id = @id",
                );
                stmt.add_param("raw_data", &raw_data_json);
                stmt.add_param("type", &type_);
                stmt.add_param("id", &id);
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
            } else {
                let mut stmt = Statement::new(
                    "INSERT INTO fingerprints (id, instance_id, type, raw_data, created_at) \
                     VALUES (@id, @instance_id, @type, @raw_data, CURRENT_TIMESTAMP())",
                );
                stmt.add_param("id", &id);
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("type", &type_);
                stmt.add_param("raw_data", &raw_data_json);
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
            }
        }
    }
    Ok(())
}

pub async fn list_jobs_for_instance(db: &Db, instance_id: &str) -> anyhow::Result<Vec<JobRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let config_json = scoped.as_text("config_json");
            let last_run_at = scoped.as_text("last_run_at");
            let next_run_at = scoped.as_text("next_run_at");
            let lease_expires_at = scoped.as_text("lease_expires_at");
            let created_at = scoped.as_text("created_at");
            let updated_at = scoped.as_text("updated_at");
            let enabled = scoped.bool_as_int("enabled");
            let sql = format!(
                "SELECT name, display_name, description, cron, {enabled}, last_status, last_error, \
                        run_count, last_rows_removed, {config_json}, {last_run_at}, {next_run_at}, \
                        {lease_expires_at}, {created_at}, {updated_at} \
                 FROM jobs WHERE instance_id = $1 ORDER BY name ASC"
            );
            let rows = sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    i64,
                    String,
                    String,
                    i64,
                    i64,
                    String,
                    Option<String>,
                    Option<String>,
                    Option<String>,
                    String,
                    String,
                ),
            >(&sql)
            .bind(instance_id)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|row| JobRecord {
                    name: row.0,
                    display_name: row.1,
                    description: row.2,
                    cron: row.3,
                    enabled: row.4 != 0,
                    last_status: row.5,
                    last_error: row.6,
                    run_count: row.7,
                    last_rows_removed: row.8,
                    config_json: row.9,
                    last_run_at: row.10,
                    next_run_at: row.11,
                    lease_expires_at: row.12,
                    created_at: row.13,
                    updated_at: row.14,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT name, display_name, description, cron, enabled, last_status, last_error, run_count, \
                        last_rows_removed, IFNULL(config_json, '{}') AS config_json, \
                        CAST(last_run_at AS STRING) AS last_run_at, CAST(next_run_at AS STRING) AS next_run_at, \
                        CAST(lease_expires_at AS STRING) AS lease_expires_at, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM jobs WHERE instance_id = @instance_id ORDER BY name ASC",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| JobRecord {
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    display_name: row
                        .column_by_name::<String>("display_name")
                        .unwrap_or_default(),
                    description: row
                        .column_by_name::<String>("description")
                        .unwrap_or_default(),
                    cron: row.column_by_name::<String>("cron").unwrap_or_default(),
                    enabled: row.column_by_name::<bool>("enabled").unwrap_or(false),
                    last_status: row
                        .column_by_name::<String>("last_status")
                        .unwrap_or_default(),
                    last_error: row
                        .column_by_name::<String>("last_error")
                        .unwrap_or_default(),
                    run_count: row.column_by_name::<i64>("run_count").unwrap_or(0),
                    last_rows_removed: row.column_by_name::<i64>("last_rows_removed").unwrap_or(0),
                    config_json: row
                        .column_by_name::<String>("config_json")
                        .unwrap_or_else(|_| "{}".to_string()),
                    last_run_at: row
                        .column_by_name::<Option<String>>("last_run_at")
                        .unwrap_or(None),
                    next_run_at: row
                        .column_by_name::<Option<String>>("next_run_at")
                        .unwrap_or(None),
                    lease_expires_at: row
                        .column_by_name::<Option<String>>("lease_expires_at")
                        .unwrap_or(None),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                    updated_at: row
                        .column_by_name::<String>("updated_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn search_records(
    db: &Db,
    instance_id: &str,
    q: &str,
    limit: i64,
) -> anyhow::Result<Vec<SearchRecord>> {
    let pattern = format!("%{q}%");
    let mut results = Vec::new();
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let user_rows = sqlx::query_as::<_, (String, String, String)>(
                "SELECT id, identifier, display_name FROM users \
                 WHERE instance_id = $1 AND (identifier LIKE $2 OR display_name LIKE $3) LIMIT $4",
            )
            .bind(instance_id)
            .bind(&pattern)
            .bind(&pattern)
            .bind(limit)
            .fetch_all(scoped.pool())
            .await?;
            for row in user_rows {
                results.push(SearchRecord {
                    resource_type: "user".to_string(),
                    id: row.0,
                    title: row.2.clone(),
                    subtitle: row.1,
                });
            }
            let org_rows = sqlx::query_as::<_, (String, String)>(
                "SELECT id, name FROM orgs WHERE instance_id = $1 AND name LIKE $2 LIMIT $3",
            )
            .bind(instance_id)
            .bind(&pattern)
            .bind(limit)
            .fetch_all(scoped.pool())
            .await?;
            for row in org_rows {
                results.push(SearchRecord {
                    resource_type: "org".to_string(),
                    id: row.0.clone(),
                    title: row.1,
                    subtitle: format!("Organization {}", row.0),
                });
            }
        }
        Db::Spanner(spanner) => {
            let mut user_stmt = Statement::new(
                "SELECT id, identifier, display_name FROM users \
                 WHERE instance_id = @instance_id AND (identifier LIKE @pattern OR display_name LIKE @pattern) LIMIT @limit",
            );
            user_stmt.add_param("instance_id", &instance_id);
            user_stmt.add_param("pattern", &pattern);
            user_stmt.add_param("limit", &limit);
            for row in spanner_query_all(spanner, user_stmt).await? {
                let id = row.column_by_name::<String>("id").unwrap_or_default();
                let identifier = row
                    .column_by_name::<String>("identifier")
                    .unwrap_or_default();
                let display_name = row
                    .column_by_name::<String>("display_name")
                    .unwrap_or_default();
                results.push(SearchRecord {
                    resource_type: "user".to_string(),
                    id,
                    title: display_name.clone(),
                    subtitle: identifier,
                });
            }
            let mut org_stmt = Statement::new(
                "SELECT id, name FROM orgs WHERE instance_id = @instance_id AND name LIKE @pattern LIMIT @limit",
            );
            org_stmt.add_param("instance_id", &instance_id);
            org_stmt.add_param("pattern", &pattern);
            org_stmt.add_param("limit", &limit);
            for row in spanner_query_all(spanner, org_stmt).await? {
                let id = row.column_by_name::<String>("id").unwrap_or_default();
                results.push(SearchRecord {
                    resource_type: "org".to_string(),
                    id: id.clone(),
                    title: row.column_by_name::<String>("name").unwrap_or_default(),
                    subtitle: format!("Organization {id}"),
                });
            }
        }
    }
    Ok(results)
}

pub async fn list_schema_registry(
    db: &Db,
    after_id: &str,
    type_filter: Option<&str>,
    limit: i64,
) -> anyhow::Result<Vec<SchemaRegistryRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped_default();
            let is_default = scoped.bool_as_int("is_default");
            let created_at = scoped.as_text("created_at");
            let (sql, bind_type) = if type_filter.is_some() {
                (
                    format!(
                        "SELECT id, type, {}, version, {is_default}, visibility, {created_at} \
                         FROM schemas WHERE id > $1 AND type = $2 ORDER BY type, version DESC LIMIT $3",
                        scoped.as_text("schema"),
                    ),
                    type_filter,
                )
            } else {
                (
                    format!(
                        "SELECT id, type, {}, version, {is_default}, visibility, {created_at} \
                         FROM schemas WHERE id > $1 ORDER BY type, version DESC LIMIT $2",
                        scoped.as_text("schema"),
                    ),
                    None,
                )
            };
            let rows = if let Some(type_filter) = bind_type {
                sqlx::query_as::<_, (String, String, String, i64, i64, String, String)>(&sql)
                    .bind(after_id)
                    .bind(type_filter)
                    .bind(limit)
                    .fetch_all(scoped.pool())
                    .await?
            } else {
                sqlx::query_as::<_, (String, String, String, i64, i64, String, String)>(&sql)
                    .bind(after_id)
                    .bind(limit)
                    .fetch_all(scoped.pool())
                    .await?
            };
            Ok(rows
                .into_iter()
                .map(|row| SchemaRegistryRecord {
                    id: row.0,
                    type_: row.1,
                    schema_json: row.2,
                    version: row.3,
                    is_default: row.4 != 0,
                    visibility: row.5,
                    created_at: row.6,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let sql = if type_filter.is_some() {
                "SELECT id, type, schema, version, is_default, visibility, CAST(created_at AS STRING) AS created_at \
                 FROM schemas WHERE id > @after_id AND type = @type ORDER BY type, version DESC LIMIT @limit"
            } else {
                "SELECT id, type, schema, version, is_default, visibility, CAST(created_at AS STRING) AS created_at \
                 FROM schemas WHERE id > @after_id ORDER BY type, version DESC LIMIT @limit"
            };
            let mut stmt = Statement::new(sql);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            if let Some(type_filter) = type_filter {
                stmt.add_param("type", &type_filter);
            }
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| SchemaRegistryRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    type_: row.column_by_name::<String>("type").unwrap_or_default(),
                    schema_json: row.column_by_name::<String>("schema").unwrap_or_default(),
                    version: row.column_by_name::<i64>("version").unwrap_or(1),
                    is_default: row.column_by_name::<bool>("is_default").unwrap_or(false),
                    visibility: row
                        .column_by_name::<String>("visibility")
                        .unwrap_or_default(),
                    created_at: row
                        .column_by_name::<String>("created_at")
                        .unwrap_or_default(),
                })
                .collect())
        }
    }
}

pub async fn get_schema_record(db: &Db, id: &str) -> anyhow::Result<Option<SchemaRegistryRecord>> {
    let items = list_schema_registry(db, "", None, i64::MAX).await?;
    Ok(items.into_iter().find(|item| item.id == id))
}

pub async fn create_schema_record(
    db: &Db,
    id: &str,
    type_: &str,
    schema_json: &str,
    visibility: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped_default();
            let sql = format!(
                "INSERT INTO schemas (id, type, schema, visibility) VALUES ($1, $2, {}, $3)",
                scoped.json_bind(4),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(type_)
                .bind(visibility)
                .bind(schema_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO schemas (id, type, schema, visibility) VALUES (@id, @type, @schema, @visibility)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("type", &type_);
            stmt.add_param("schema", &schema_json);
            stmt.add_param("visibility", &visibility);
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
        }
    }
    Ok(())
}

pub async fn update_schema_record(db: &Db, id: &str, schema_json: &str) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped_default();
            let sql = format!(
                "UPDATE schemas SET schema = {}, version = version + 1 WHERE id = $1",
                scoped.json_bind(2),
            );
            Ok(sqlx::query(&sql)
                .bind(id)
                .bind(schema_json)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE schemas SET schema = @schema, version = version + 1 WHERE id = @id",
            );
            stmt.add_param("schema", &schema_json);
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

pub async fn promote_schema_record(db: &Db, id: &str) -> anyhow::Result<bool> {
    let Some(record) = get_schema_record(db, id).await? else {
        return Ok(false);
    };
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped_default();
            sqlx::query("UPDATE schemas SET is_default = FALSE WHERE type = $1")
                .bind(&record.type_)
                .execute(scoped.pool())
                .await?;
            sqlx::query(
                "UPDATE schemas SET is_default = TRUE, visibility = 'public' WHERE id = $1",
            )
            .bind(id)
            .execute(scoped.pool())
            .await?;
        }
        Db::Spanner(spanner) => {
            let mut reset_stmt =
                Statement::new("UPDATE schemas SET is_default = FALSE WHERE type = @type");
            reset_stmt.add_param("type", &record.type_);
            let mut promote_stmt = Statement::new(
                "UPDATE schemas SET is_default = TRUE, visibility = 'public' WHERE id = @id",
            );
            promote_stmt.add_param("id", &id);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let reset_stmt = reset_stmt.clone();
                    let promote_stmt = promote_stmt.clone();
                    Box::pin(async move {
                        tx.update(reset_stmt).await?;
                        tx.update(promote_stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
        }
    }
    Ok(true)
}

pub async fn count_users_for_schema(
    db: &Db,
    instance_id: &str,
    schema_id: &str,
) -> anyhow::Result<i64> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM users WHERE instance_id = $1 AND schema_id = $2",
            )
            .bind(instance_id)
            .bind(schema_id)
            .fetch_one(scoped.pool())
            .await?
            .0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT COUNT(*) AS total FROM users WHERE instance_id = @instance_id AND schema_id = @schema_id",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("schema_id", &schema_id);
            spanner_query_scalar_i64(spanner, stmt).await
        }
    }
}

pub async fn create_login_flow(
    db: &Db,
    instance_id: &str,
    id: &str,
    name: &str,
    strategy: &str,
    config_json: &str,
    audience_json: &str,
    auth_methods_json: &str,
    is_default: bool,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO login_flows (id, instance_id, name, strategy, config, audience, auth_methods, is_default) \
                 VALUES ($1, $2, $3, $4, {}, {}, {}, $8)",
                scoped.json_bind(5),
                scoped.json_bind(6),
                scoped.json_bind(7),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(name)
                .bind(strategy)
                .bind(config_json)
                .bind(audience_json)
                .bind(auth_methods_json)
                .bind(is_default)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO login_flows (id, instance_id, name, strategy, config, audience, auth_methods, is_default) \
                 VALUES (@id, @instance_id, @name, @strategy, @config, @audience, @auth_methods, @is_default)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("name", &name);
            stmt.add_param("strategy", &strategy);
            stmt.add_param("config", &config_json);
            stmt.add_param("audience", &audience_json);
            stmt.add_param("auth_methods", &auth_methods_json);
            stmt.add_param("is_default", &is_default);
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
        }
    }
    Ok(())
}

pub async fn list_login_flow_records(
    db: &Db,
    instance_id: &str,
    after_id: &str,
    limit: i64,
) -> anyhow::Result<Vec<LoginFlowRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let is_default = scoped.bool_as_int("is_default");
            let enabled = scoped.bool_as_int("enabled");
            let config = scoped.as_text("config");
            let audience = scoped.as_text("audience");
            let auth_methods = scoped.as_text("auth_methods");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, name, strategy, state, {is_default}, {enabled}, priority, \
                        COALESCE({config}, '{{}}'), COALESCE({audience}, '{{}}'), COALESCE({auth_methods}, '{{}}'), \
                        {created_at}, {updated_at} \
                 FROM login_flows WHERE instance_id = $1 AND id > $2 ORDER BY priority DESC, name LIMIT $3"
            );
            let rows = sqlx::query_as::<
                _,
                (
                    String,
                    String,
                    String,
                    String,
                    i64,
                    i64,
                    i64,
                    String,
                    String,
                    String,
                    String,
                    String,
                ),
            >(&sql)
            .bind(instance_id)
            .bind(after_id)
            .bind(limit)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows.into_iter().map(login_flow_from_sql_row).collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name, strategy, state, is_default, enabled, priority, \
                        IFNULL(config, '{}') AS config, IFNULL(audience, '{}') AS audience, \
                        IFNULL(auth_methods, '{}') AS auth_methods, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM login_flows WHERE instance_id = @instance_id AND id > @after_id \
                 ORDER BY priority DESC, name LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("after_id", &after_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(login_flow_from_spanner_row)
                .collect())
        }
    }
}

pub async fn get_login_flow_record(
    db: &Db,
    instance_id: &str,
    id: &str,
) -> anyhow::Result<Option<LoginFlowRecord>> {
    let rows = list_login_flow_records(db, instance_id, "", i64::MAX).await?;
    Ok(rows.into_iter().find(|row| row.id == id))
}

pub async fn update_login_flow(
    db: &Db,
    instance_id: &str,
    id: &str,
    name: &str,
    strategy: &str,
    config_json: &str,
    auth_methods_json: &str,
    is_default: bool,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "UPDATE login_flows SET name = $1, strategy = $2, config = {}, auth_methods = {}, is_default = $5, updated_at = CURRENT_TIMESTAMP \
                 WHERE instance_id = $6 AND id = $7",
                scoped.json_bind(3),
                scoped.json_bind(4),
            );
            Ok(sqlx::query(&sql)
                .bind(name)
                .bind(strategy)
                .bind(config_json)
                .bind(auth_methods_json)
                .bind(is_default)
                .bind(instance_id)
                .bind(id)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE login_flows SET name = @name, strategy = @strategy, config = @config, \
                 auth_methods = @auth_methods, is_default = @is_default, updated_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND id = @id",
            );
            stmt.add_param("name", &name);
            stmt.add_param("strategy", &strategy);
            stmt.add_param("config", &config_json);
            stmt.add_param("auth_methods", &auth_methods_json);
            stmt.add_param("is_default", &is_default);
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

pub async fn set_login_flow_state(
    db: &Db,
    instance_id: &str,
    id: &str,
    state: &str,
    enabled: bool,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query(
                "UPDATE login_flows SET state = $1, enabled = $2, updated_at = CURRENT_TIMESTAMP WHERE instance_id = $3 AND id = $4",
            )
            .bind(state)
            .bind(enabled)
            .bind(instance_id)
            .bind(id)
            .execute(scoped.pool())
            .await?
            .rows_affected()
            > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE login_flows SET state = @state, enabled = @enabled, updated_at = CURRENT_TIMESTAMP() \
                 WHERE instance_id = @instance_id AND id = @id",
            );
            stmt.add_param("state", &state);
            stmt.add_param("enabled", &enabled);
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

pub async fn resolve_login_flow(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Option<(String, String)>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query_as::<_, (String, String)>(
                "SELECT id, name FROM login_flows WHERE instance_id = $1 AND enabled = TRUE ORDER BY is_default DESC, priority DESC LIMIT 1",
            )
            .bind(instance_id)
            .fetch_optional(scoped.pool())
            .await?)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name FROM login_flows WHERE instance_id = @instance_id AND enabled = TRUE ORDER BY is_default DESC, priority DESC LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt).await?.map(|row| {
                (
                    row.column_by_name::<String>("id").unwrap_or_default(),
                    row.column_by_name::<String>("name").unwrap_or_default(),
                )
            }))
        }
    }
}

async fn count_child_instances(db: &Db, instance_id: &str) -> anyhow::Result<i64> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query_as::<_, (i64,)>(
                "SELECT COUNT(*) FROM instances WHERE parent_instance_id = $1",
            )
            .bind(instance_id)
            .fetch_one(scoped.pool())
            .await?
            .0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT COUNT(*) AS total FROM instances WHERE parent_instance_id = @instance_id",
            );
            stmt.add_param("instance_id", &instance_id);
            spanner_query_scalar_i64(spanner, stmt).await
        }
    }
}

async fn list_orgs(db: &Db, instance_id: &str, limit: i64) -> anyhow::Result<Vec<OrgSummary>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let rows: Vec<(String, String, String)> = sqlx::query_as(
                "SELECT id, name, state FROM orgs WHERE instance_id = $1 ORDER BY name LIMIT $2",
            )
            .bind(instance_id)
            .bind(limit)
            .fetch_all(scoped.pool())
            .await?;
            Ok(rows
                .into_iter()
                .map(|row| OrgSummary {
                    id: row.0,
                    name: row.1,
                    state: row.2,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, name, state FROM orgs \
                 WHERE instance_id = @instance_id ORDER BY name LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("limit", &limit);
            Ok(spanner_query_all(spanner, stmt)
                .await?
                .into_iter()
                .map(|row| OrgSummary {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    name: row.column_by_name::<String>("name").unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                })
                .collect())
        }
    }
}

fn metadata_has_capability(metadata_json: &str, capability: &str) -> bool {
    serde_json::from_str::<Value>(metadata_json)
        .ok()
        .and_then(|value| value.get("capabilities").and_then(Value::as_array).cloned())
        .map(|entries| {
            entries
                .iter()
                .any(|entry| entry.as_str().is_some_and(|item| item == capability))
        })
        .unwrap_or(false)
}

pub async fn user_has_capability(
    db: &Db,
    instance_id: &str,
    user_id: &str,
    capability: &str,
) -> anyhow::Result<bool> {
    Ok(load_identity_metadata(db, instance_id, user_id)
        .await?
        .map(|identity| metadata_has_capability(&identity.metadata_json, capability))
        .unwrap_or(false))
}

pub async fn find_active_user_by_identifier(
    db: &Db,
    instance_id: &str,
    identifier: &str,
) -> anyhow::Result<Option<UserRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let metadata = scoped.as_text("metadata");
            let (created_at, updated_at) = scoped.select_timestamps();
            let sql = format!(
                "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, \
                        COALESCE({metadata}, '{{}}'), {created_at}, {updated_at} \
                 FROM users \
                 WHERE instance_id = $1 AND identifier = $2 AND state = 'active' \
                 LIMIT 1"
            );
            Ok(sqlx::query_as::<
                _,
                (
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
                ),
            >(&sql)
            .bind(instance_id)
            .bind(identifier)
            .fetch_optional(scoped.pool())
            .await?
            .map(|row| UserRecord {
                id: row.0,
                org_id: row.1,
                identifier: row.2,
                display_name: row.3,
                user_type: row.4,
                state: row.5,
                schema_id: row.6,
                metadata_json: row.7,
                created_at: row.8,
                updated_at: row.9,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, org_id, identifier, display_name, user_type, state, schema_id, \
                        IFNULL(metadata, '{}') AS metadata, \
                        CAST(created_at AS STRING) AS created_at, CAST(updated_at AS STRING) AS updated_at \
                 FROM users \
                 WHERE instance_id = @instance_id AND identifier = @identifier AND state = 'active' \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("identifier", &identifier);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| UserRecord {
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
                    metadata_json: row.column_by_name::<String>("metadata").unwrap_or_default(),
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

pub async fn find_linked_identity(
    db: &Db,
    instance_id: &str,
    provider_id: &str,
    external_sub: &str,
) -> anyhow::Result<Option<LinkedIdentityRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let raw_claims = scoped.as_text("raw_claims");
            let sql = format!(
                "SELECT id, user_id, provider_id, external_sub, COALESCE(external_email, ''), \
                        COALESCE({raw_claims}, '{{}}') \
                 FROM linked_identities \
                 WHERE instance_id = $1 AND provider_id = $2 AND external_sub = $3 \
                 LIMIT 1"
            );
            Ok(
                sqlx::query_as::<_, (String, String, String, String, String, String)>(&sql)
                    .bind(instance_id)
                    .bind(provider_id)
                    .bind(external_sub)
                    .fetch_optional(scoped.pool())
                    .await?
                    .map(|row| LinkedIdentityRecord {
                        id: row.0,
                        user_id: row.1,
                        provider_id: row.2,
                        external_sub: row.3,
                        external_email: row.4,
                        raw_claims_json: row.5,
                    }),
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, user_id, provider_id, external_sub, IFNULL(external_email, '') AS external_email, \
                        IFNULL(raw_claims, '{}') AS raw_claims \
                 FROM linked_identities \
                 WHERE instance_id = @instance_id AND provider_id = @provider_id AND external_sub = @external_sub \
                 LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("provider_id", &provider_id);
            stmt.add_param("external_sub", &external_sub);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| LinkedIdentityRecord {
                    id: row.column_by_name::<String>("id").unwrap_or_default(),
                    user_id: row.column_by_name::<String>("user_id").unwrap_or_default(),
                    provider_id: row
                        .column_by_name::<String>("provider_id")
                        .unwrap_or_default(),
                    external_sub: row
                        .column_by_name::<String>("external_sub")
                        .unwrap_or_default(),
                    external_email: row
                        .column_by_name::<String>("external_email")
                        .unwrap_or_default(),
                    raw_claims_json: row
                        .column_by_name::<String>("raw_claims")
                        .unwrap_or_default(),
                }))
        }
    }
}

pub async fn touch_linked_identity(
    db: &Db,
    instance_id: &str,
    provider_id: &str,
    external_sub: &str,
    external_email: &str,
    raw_claims_json: &str,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "UPDATE linked_identities \
                 SET last_used_at = CURRENT_TIMESTAMP, external_email = $1, raw_claims = {} \
                 WHERE instance_id = $2 AND provider_id = $3 AND external_sub = $4",
                scoped.json_bind(5),
            );
            Ok(sqlx::query(&sql)
                .bind(external_email)
                .bind(instance_id)
                .bind(provider_id)
                .bind(external_sub)
                .bind(raw_claims_json)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE linked_identities \
                 SET last_used_at = CURRENT_TIMESTAMP(), external_email = @external_email, raw_claims = @raw_claims \
                 WHERE instance_id = @instance_id AND provider_id = @provider_id AND external_sub = @external_sub",
            );
            stmt.add_param("external_email", &external_email);
            stmt.add_param("raw_claims", &raw_claims_json);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("provider_id", &provider_id);
            stmt.add_param("external_sub", &external_sub);
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

pub async fn create_linked_identity_record(
    db: &Db,
    instance_id: &str,
    id: &str,
    user_id: &str,
    provider_id: &str,
    external_sub: &str,
    external_email: &str,
    raw_claims_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO linked_identities \
                 (id, instance_id, user_id, provider_id, external_sub, external_email, raw_claims) \
                 VALUES ($1, $2, $3, $4, $5, $6, {})",
                scoped.json_bind(7),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(user_id)
                .bind(provider_id)
                .bind(external_sub)
                .bind(external_email)
                .bind(raw_claims_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO linked_identities \
                 (id, instance_id, user_id, provider_id, external_sub, external_email, raw_claims) \
                 VALUES (@id, @instance_id, @user_id, @provider_id, @external_sub, @external_email, @raw_claims)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("user_id", &user_id);
            stmt.add_param("provider_id", &provider_id);
            stmt.add_param("external_sub", &external_sub);
            stmt.add_param("external_email", &external_email);
            stmt.add_param("raw_claims", &raw_claims_json);
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
        }
    }
    Ok(())
}

pub async fn update_session_metadata(
    db: &Db,
    instance_id: &str,
    session_id: &str,
    metadata_json: &str,
) -> anyhow::Result<bool> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "UPDATE sessions SET metadata = {} WHERE instance_id = $1 AND id = $2",
                scoped.json_bind(3),
            );
            Ok(sqlx::query(&sql)
                .bind(instance_id)
                .bind(session_id)
                .bind(metadata_json)
                .execute(scoped.pool())
                .await?
                .rows_affected()
                > 0)
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "UPDATE sessions SET metadata = @metadata WHERE instance_id = @instance_id AND id = @id",
            );
            stmt.add_param("metadata", &metadata_json);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &session_id);
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

pub async fn append_event(
    db: &Db,
    instance_id: &str,
    id: &str,
    event_type: &str,
    category: &str,
    flow_id: &str,
    fingerprint: &str,
    payload_json: &str,
    metadata_json: &str,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO events \
                 (id, instance_id, event_type, category, flow_id, fingerprint, payload, metadata) \
                 VALUES ($1, $2, $3, $4, $5, $6, {}, {})",
                scoped.json_bind(7),
                scoped.json_bind(8),
            );
            sqlx::query(&sql)
                .bind(id)
                .bind(instance_id)
                .bind(event_type)
                .bind(category)
                .bind(flow_id)
                .bind(fingerprint)
                .bind(payload_json)
                .bind(metadata_json)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO events \
                 (id, instance_id, event_type, category, flow_id, fingerprint, payload, metadata) \
                 VALUES (@id, @instance_id, @event_type, @category, @flow_id, @fingerprint, @payload, @metadata)",
            );
            stmt.add_param("id", &id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("event_type", &event_type);
            stmt.add_param("category", &category);
            stmt.add_param("flow_id", &flow_id);
            stmt.add_param("fingerprint", &fingerprint);
            stmt.add_param("payload", &payload_json);
            stmt.add_param("metadata", &metadata_json);
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
        }
    }
    Ok(())
}

pub async fn resolve_domain_route(
    db: &Db,
    host: &str,
) -> anyhow::Result<Option<RouteResolutionRecord>> {
    match db {
        Db::Sql(_) => {
            let row = sqlx::query_as::<_, (String, Option<String>, String, Option<String>)>(
                "SELECT d.instance_id, d.org_id AS resolved_org_id, i.placement_mode, \
                        NULLIF(i.region_key, '') AS region_key \
                 FROM domains d \
                 JOIN instances i ON i.instance_id = d.instance_id \
                 WHERE d.domain = $1 AND d.state = 'active' AND i.state = 'active' \
                 ORDER BY d.is_primary DESC, d.updated_at DESC LIMIT 1",
            )
            .bind(host)
            .fetch_optional(db.pool())
            .await?;
            Ok(row.map(|row| RouteResolutionRecord {
                instance_id: row.0,
                resolved_org_id: row.1,
                placement_mode: row.2,
                region_key: row.3,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT d.instance_id, d.org_id AS resolved_org_id, i.placement_mode, i.region_key \
                 FROM domains d \
                 JOIN instances i ON i.instance_id = d.instance_id \
                 WHERE d.domain = @host AND d.state = 'active' AND i.state = 'active' \
                 ORDER BY d.is_primary DESC, d.updated_at DESC LIMIT 1",
            );
            stmt.add_param("host", &host);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| RouteResolutionRecord {
                    instance_id: row
                        .column_by_name::<String>("instance_id")
                        .unwrap_or_default(),
                    resolved_org_id: row
                        .column_by_name::<Option<String>>("resolved_org_id")
                        .unwrap_or(None),
                    placement_mode: row
                        .column_by_name::<String>("placement_mode")
                        .unwrap_or_default(),
                    region_key: row
                        .column_by_name::<Option<String>>("region_key")
                        .unwrap_or(None)
                        .filter(|value| !value.is_empty()),
                }))
        }
    }
}

pub async fn resolve_instance_route(
    db: &Db,
    instance_id: &str,
) -> anyhow::Result<Option<RouteResolutionRecord>> {
    match db {
        Db::Sql(_) => {
            let row = sqlx::query_as::<_, (String, String, Option<String>)>(
                "SELECT i.instance_id, i.placement_mode, NULLIF(i.region_key, '') AS region_key \
                 FROM instances i \
                 WHERE i.instance_id = $1 AND i.state = 'active' LIMIT 1",
            )
            .bind(instance_id)
            .fetch_optional(db.pool())
            .await?;
            Ok(row.map(|row| RouteResolutionRecord {
                instance_id: row.0,
                resolved_org_id: None,
                placement_mode: row.1,
                region_key: row.2,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT i.instance_id, i.placement_mode, i.region_key \
                 FROM instances i \
                 WHERE i.instance_id = @instance_id AND i.state = 'active' LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| RouteResolutionRecord {
                    instance_id: row
                        .column_by_name::<String>("instance_id")
                        .unwrap_or_default(),
                    resolved_org_id: None,
                    placement_mode: row
                        .column_by_name::<String>("placement_mode")
                        .unwrap_or_default(),
                    region_key: row
                        .column_by_name::<Option<String>>("region_key")
                        .unwrap_or(None)
                        .filter(|value| !value.is_empty()),
                }))
        }
    }
}

pub async fn get_oidc_client_record(
    db: &Db,
    instance_id: &str,
    client_id: &str,
) -> anyhow::Result<Option<OidcClientRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "SELECT COALESCE(client_secret, ''), COALESCE({}, '[]'), COALESCE({}, '[]'), \
                        COALESCE({}, '[]'), COALESCE(state, 'active') \
                 FROM apps WHERE instance_id = $1 AND client_id = $2",
                scoped.as_text("redirect_uris"),
                scoped.as_text("grant_types"),
                scoped.as_text("response_types"),
            );
            Ok(
                sqlx::query_as::<_, (String, String, String, String, String)>(&sql)
                    .bind(instance_id)
                    .bind(client_id)
                    .fetch_optional(scoped.pool())
                    .await?
                    .map(|row| OidcClientRecord {
                        client_secret: row.0,
                        redirect_uris_json: row.1,
                        grant_types_json: row.2,
                        response_types_json: row.3,
                        state: row.4,
                    }),
            )
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT IFNULL(client_secret, '') AS client_secret, \
                        IFNULL(redirect_uris, '[]') AS redirect_uris, \
                        IFNULL(grant_types, '[]') AS grant_types, \
                        IFNULL(response_types, '[]') AS response_types, \
                        IFNULL(state, 'active') AS state \
                 FROM apps WHERE instance_id = @instance_id AND client_id = @client_id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("client_id", &client_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| OidcClientRecord {
                    client_secret: row
                        .column_by_name::<String>("client_secret")
                        .unwrap_or_default(),
                    redirect_uris_json: row
                        .column_by_name::<String>("redirect_uris")
                        .unwrap_or_default(),
                    grant_types_json: row
                        .column_by_name::<String>("grant_types")
                        .unwrap_or_default(),
                    response_types_json: row
                        .column_by_name::<String>("response_types")
                        .unwrap_or_default(),
                    state: row.column_by_name::<String>("state").unwrap_or_default(),
                }))
        }
    }
}

pub async fn create_oidc_auth_request_record(
    db: &Db,
    instance_id: &str,
    auth_request_id: &str,
    client_id: &str,
    redirect_uri: &str,
    scope: &str,
    state_value: &str,
    nonce: &str,
    response_type: &str,
    code_challenge: &str,
    code_challenge_method: &str,
    prompt_json: &str,
    login_hint: &str,
    max_age: Option<i64>,
) -> anyhow::Result<()> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let sql = format!(
                "INSERT INTO oidc_auth_requests \
                 (id, instance_id, client_id, redirect_uri, scope, state, nonce, response_type, code_challenge, code_challenge_method, prompt, login_hint, max_age) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, {}, $12, $13)",
                scoped.json_bind(11),
            );
            sqlx::query(&sql)
                .bind(auth_request_id)
                .bind(instance_id)
                .bind(client_id)
                .bind(redirect_uri)
                .bind(scope)
                .bind(state_value)
                .bind(nonce)
                .bind(response_type)
                .bind(code_challenge)
                .bind(code_challenge_method)
                .bind(prompt_json)
                .bind(login_hint)
                .bind(max_age)
                .execute(scoped.pool())
                .await?;
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "INSERT INTO oidc_auth_requests \
                 (id, instance_id, client_id, redirect_uri, scope, state, nonce, response_type, code_challenge, code_challenge_method, prompt, login_hint, max_age) \
                 VALUES \
                 (@id, @instance_id, @client_id, @redirect_uri, @scope, @state, @nonce, @response_type, @code_challenge, @code_challenge_method, @prompt, @login_hint, @max_age)",
            );
            stmt.add_param("id", &auth_request_id);
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("client_id", &client_id);
            stmt.add_param("redirect_uri", &redirect_uri);
            stmt.add_param("scope", &scope);
            stmt.add_param("state", &state_value);
            stmt.add_param("nonce", &nonce);
            stmt.add_param("response_type", &response_type);
            stmt.add_param("code_challenge", &code_challenge);
            stmt.add_param("code_challenge_method", &code_challenge_method);
            stmt.add_param("prompt", &prompt_json);
            stmt.add_param("login_hint", &login_hint);
            stmt.add_param("max_age", &max_age);
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
        }
    }
    Ok(())
}

pub async fn consume_oidc_auth_code_record(
    db: &Db,
    instance_id: &str,
    code: &str,
) -> anyhow::Result<Option<OidcAuthRequestRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let mut tx = scoped.pool().begin().await?;
            let auth_time = scoped.epoch_seconds("auth_time");
            let row: Option<(String, String, String, String, String, String, String, Option<i64>)> =
                sqlx::query_as(&format!(
                    "SELECT id, user_id, client_id, redirect_uri, scope, nonce, code_challenge, {auth_time} \
                     FROM oidc_auth_requests WHERE instance_id = $1 AND code = $2 AND done = 1"
                ))
                .bind(instance_id)
                .bind(code)
                .fetch_optional(&mut *tx)
                .await?;
            let Some(row) = row else {
                tx.rollback().await?;
                return Ok(None);
            };
            sqlx::query("DELETE FROM oidc_auth_requests WHERE instance_id = $1 AND id = $2")
                .bind(instance_id)
                .bind(&row.0)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            Ok(Some(OidcAuthRequestRecord {
                auth_request_id: row.0,
                user_id: row.1,
                client_id: row.2,
                redirect_uri: row.3,
                scope: row.4,
                nonce: row.5,
                code_challenge: row.6,
                auth_time: row.7,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, user_id, client_id, redirect_uri, scope, nonce, code_challenge, \
                        UNIX_SECONDS(auth_time) AS auth_time \
                 FROM oidc_auth_requests \
                 WHERE instance_id = @instance_id AND code = @code AND done = TRUE LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("code", &code);
            let Some(row) = spanner_query_optional(spanner, stmt).await? else {
                return Ok(None);
            };
            let auth_request_id = row.column_by_name::<String>("id")?;
            let record = OidcAuthRequestRecord {
                auth_request_id: auth_request_id.clone(),
                user_id: row.column_by_name::<String>("user_id")?,
                client_id: row.column_by_name::<String>("client_id")?,
                redirect_uri: row.column_by_name::<String>("redirect_uri")?,
                scope: row.column_by_name::<String>("scope")?,
                nonce: row.column_by_name::<String>("nonce")?,
                code_challenge: row.column_by_name::<String>("code_challenge")?,
                auth_time: row.column_by_name::<Option<i64>>("auth_time")?,
            };
            let mut delete_stmt = Statement::new(
                "DELETE FROM oidc_auth_requests WHERE instance_id = @instance_id AND id = @id",
            );
            delete_stmt.add_param("instance_id", &instance_id);
            delete_stmt.add_param("id", &auth_request_id);
            let _ = spanner
                .client()
                .read_write_transaction(|tx| {
                    let stmt = delete_stmt.clone();
                    Box::pin(async move {
                        tx.update(stmt).await?;
                        Ok::<(), SpannerError>(())
                    })
                })
                .await?;
            Ok(Some(record))
        }
    }
}

pub async fn load_user_claims_record(
    db: &Db,
    instance_id: &str,
    user_id: &str,
) -> anyhow::Result<Option<UserClaimsRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            Ok(sqlx::query_as::<_, (String, String)>(
                "SELECT identifier, display_name FROM users WHERE instance_id = $1 AND id = $2",
            )
            .bind(instance_id)
            .bind(user_id)
            .fetch_optional(scoped.pool())
            .await?
            .map(|row| UserClaimsRecord {
                identifier: row.0,
                display_name: row.1,
            }))
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT identifier, display_name FROM users \
                 WHERE instance_id = @instance_id AND id = @id LIMIT 1",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("id", &user_id);
            Ok(spanner_query_optional(spanner, stmt)
                .await?
                .map(|row| UserClaimsRecord {
                    identifier: row
                        .column_by_name::<String>("identifier")
                        .unwrap_or_default(),
                    display_name: row
                        .column_by_name::<String>("display_name")
                        .unwrap_or_default(),
                }))
        }
    }
}

fn instance_from_sql_row(
    row: (
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        Option<String>,
    ),
) -> ManagedInstanceRecord {
    ManagedInstanceRecord {
        instance_id: row.0,
        state: row.1,
        kind: row.2,
        placement_mode: row.3,
        region_key: row.4,
        owner_org_id: row.5,
        feature_overrides_json: row.6,
        created_at: row.7,
        updated_at: row.8,
        primary_domain: row.9,
    }
}

fn action_from_sql_row(
    row: (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        i64,
        i64,
        String,
        String,
    ),
) -> ActionRecord {
    ActionRecord {
        id: row.0,
        org_id: row.1,
        name: row.2,
        hook: row.3,
        action_type: row.4,
        trigger_expr: row.5,
        config_json: row.6,
        priority: row.7,
        enabled: row.8 != 0,
        fail_open: row.9 != 0,
        metadata_json: row.10,
        created_at: row.11,
    }
}

fn login_flow_from_sql_row(
    row: (
        String,
        String,
        String,
        String,
        i64,
        i64,
        i64,
        String,
        String,
        String,
        String,
        String,
    ),
) -> LoginFlowRecord {
    LoginFlowRecord {
        id: row.0,
        name: row.1,
        strategy: row.2,
        state: row.3,
        is_default: row.4 != 0,
        enabled: row.5 != 0,
        priority: row.6,
        config_json: row.7,
        audience_json: row.8,
        auth_methods_json: row.9,
        created_at: row.10,
        updated_at: row.11,
    }
}

fn instance_from_spanner_row(row: Row) -> ManagedInstanceRecord {
    ManagedInstanceRecord {
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
            .unwrap_or(None),
        owner_org_id: row
            .column_by_name::<String>("owner_org_id")
            .unwrap_or_default(),
        feature_overrides_json: row
            .column_by_name::<String>("feature_overrides")
            .unwrap_or_else(|_| "{}".to_string()),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
        updated_at: row
            .column_by_name::<String>("updated_at")
            .unwrap_or_default(),
        primary_domain: row
            .column_by_name::<Option<String>>("primary_domain")
            .unwrap_or(None),
    }
}

fn action_from_spanner_row(row: Row) -> ActionRecord {
    ActionRecord {
        id: row.column_by_name::<String>("id").unwrap_or_default(),
        org_id: row.column_by_name::<String>("org_id").unwrap_or_default(),
        name: row.column_by_name::<String>("name").unwrap_or_default(),
        hook: row.column_by_name::<String>("hook").unwrap_or_default(),
        action_type: row
            .column_by_name::<String>("action_type")
            .unwrap_or_default(),
        trigger_expr: row
            .column_by_name::<String>("trigger_expr")
            .unwrap_or_else(|_| "true".to_string()),
        config_json: row
            .column_by_name::<String>("config")
            .unwrap_or_else(|_| "{}".to_string()),
        priority: row.column_by_name::<i64>("priority").unwrap_or(0),
        enabled: row.column_by_name::<bool>("enabled").unwrap_or(false),
        fail_open: row.column_by_name::<bool>("fail_open").unwrap_or(false),
        metadata_json: row
            .column_by_name::<String>("metadata")
            .unwrap_or_else(|_| "{}".to_string()),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
    }
}

fn login_flow_from_spanner_row(row: Row) -> LoginFlowRecord {
    LoginFlowRecord {
        id: row.column_by_name::<String>("id").unwrap_or_default(),
        name: row.column_by_name::<String>("name").unwrap_or_default(),
        strategy: row.column_by_name::<String>("strategy").unwrap_or_default(),
        state: row.column_by_name::<String>("state").unwrap_or_default(),
        is_default: row.column_by_name::<bool>("is_default").unwrap_or(false),
        enabled: row.column_by_name::<bool>("enabled").unwrap_or(false),
        priority: row.column_by_name::<i64>("priority").unwrap_or(0),
        config_json: row
            .column_by_name::<String>("config")
            .unwrap_or_else(|_| "{}".to_string()),
        audience_json: row
            .column_by_name::<String>("audience")
            .unwrap_or_else(|_| "{}".to_string()),
        auth_methods_json: row
            .column_by_name::<String>("auth_methods")
            .unwrap_or_else(|_| "{}".to_string()),
        created_at: row
            .column_by_name::<String>("created_at")
            .unwrap_or_default(),
        updated_at: row
            .column_by_name::<String>("updated_at")
            .unwrap_or_default(),
    }
}

async fn spanner_query_all(
    spanner: &crate::SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Vec<Row>> {
    let mut tx = spanner.client().single().await?;
    let mut rows = tx.query(stmt).await?;
    let mut result = Vec::new();
    while let Some(row) = rows.next().await? {
        result.push(row);
    }
    Ok(result)
}

async fn spanner_query_optional(
    spanner: &crate::SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Option<Row>> {
    let mut tx = spanner.client().single().await?;
    let mut rows = tx.query(stmt).await?;
    Ok(rows.next().await?)
}

async fn spanner_query_scalar_i64(
    spanner: &crate::SpannerDb,
    stmt: Statement,
) -> anyhow::Result<i64> {
    let row = spanner_query_optional(spanner, stmt)
        .await?
        .context("spanner scalar query returned no row")?;
    row.column_by_name::<i64>("total")
        .map_err(anyhow::Error::from)
}

// ─── Event consumption ───────────────────────────────────────

/// Row returned by `fetch_unshipped_events`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnshippedEventRecord {
    pub id: String,
    pub instance_id: String,
    pub event_type: String,
    pub category: String,
    pub payload: String,
    pub metadata: String,
    pub created_at: String,
}

/// Fetch up to `limit` events that have not yet been shipped (shipped_at IS NULL),
/// ordered by created_at ASC, id ASC. Used by the event consumer job.
pub async fn fetch_unshipped_events(
    db: &Db,
    instance_id: &str,
    limit: u32,
) -> anyhow::Result<Vec<UnshippedEventRecord>> {
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let rows =
                sqlx::query_as::<_, (String, String, String, String, String, String, String)>(
                    "SELECT id, instance_id, event_type, category, \
                        COALESCE(payload, '{}'), COALESCE(metadata, '{}'), created_at \
                 FROM events \
                 WHERE instance_id = $1 AND shipped_at IS NULL \
                 ORDER BY created_at ASC, id ASC \
                 LIMIT $2",
                )
                .bind(instance_id)
                .bind(limit as i64)
                .fetch_all(scoped.pool())
                .await?;

            Ok(rows
                .into_iter()
                .map(|r| UnshippedEventRecord {
                    id: r.0,
                    instance_id: r.1,
                    event_type: r.2,
                    category: r.3,
                    payload: r.4,
                    metadata: r.5,
                    created_at: r.6,
                })
                .collect())
        }
        Db::Spanner(spanner) => {
            let mut stmt = Statement::new(
                "SELECT id, instance_id, event_type, category, \
                        COALESCE(payload, '{}'), COALESCE(metadata, '{}'), \
                        CAST(created_at AS STRING) AS created_at \
                 FROM events \
                 WHERE instance_id = @instance_id AND shipped_at IS NULL \
                 ORDER BY created_at ASC, id ASC \
                 LIMIT @limit",
            );
            stmt.add_param("instance_id", &instance_id);
            stmt.add_param("limit", &(limit as i64));

            let mut tx = spanner.client().single().await?;
            let mut result_set = tx.query(stmt).await?;
            let mut records = Vec::new();
            while let Some(row) = result_set.next().await? {
                records.push(UnshippedEventRecord {
                    id: row.column_by_name::<String>("id")?,
                    instance_id: row.column_by_name::<String>("instance_id")?,
                    event_type: row.column_by_name::<String>("event_type")?,
                    category: row.column_by_name::<String>("category")?,
                    payload: row.column_by_name::<String>("payload").unwrap_or_default(),
                    metadata: row.column_by_name::<String>("metadata").unwrap_or_default(),
                    created_at: row.column_by_name::<String>("created_at")?,
                });
            }
            Ok(records)
        }
    }
}

/// Mark events as shipped by setting shipped_at to the current timestamp.
pub async fn mark_events_shipped(
    db: &Db,
    instance_id: &str,
    event_ids: &[String],
) -> anyhow::Result<u64> {
    if event_ids.is_empty() {
        return Ok(0);
    }
    match db {
        Db::Sql(_) => {
            let scoped = db.scoped(instance_id.to_string());
            let current_ts = match db.dialect() {
                crate::Dialect::Sqlite => "datetime('now')",
                crate::Dialect::Postgres => "NOW()",
                crate::Dialect::Spanner => unreachable!(),
            };
            // Build IN list with numbered params starting at $2
            let placeholders: Vec<String> = (0..event_ids.len())
                .map(|i| format!("${}", i + 2))
                .collect();
            let sql = format!(
                "UPDATE events SET shipped_at = {current_ts} \
                 WHERE instance_id = $1 AND id IN ({}) AND shipped_at IS NULL",
                placeholders.join(", ")
            );
            let mut query = sqlx::query(&sql).bind(instance_id);
            for eid in event_ids {
                query = query.bind(eid);
            }
            let result = query.execute(scoped.pool()).await?;
            Ok(result.rows_affected())
        }
        Db::Spanner(spanner) => {
            // Spanner: batch update in a read-write transaction
            let ids = event_ids.to_vec();
            let iid = instance_id.to_string();
            let (_, affected) = spanner
                .client()
                .read_write_transaction(|tx| {
                    let ids = ids.clone();
                    let iid = iid.clone();
                    Box::pin(async move {
                        let mut total = 0i64;
                        for chunk in ids.chunks(100) {
                            let placeholders: Vec<String> = chunk
                                .iter()
                                .enumerate()
                                .map(|(i, _)| format!("@id{i}"))
                                .collect();
                            let sql = format!(
                                "UPDATE events SET shipped_at = CURRENT_TIMESTAMP() \
                                 WHERE instance_id = @iid AND id IN ({}) AND shipped_at IS NULL",
                                placeholders.join(", ")
                            );
                            let mut stmt = Statement::new(&sql);
                            stmt.add_param("iid", &iid);
                            for (i, id) in chunk.iter().enumerate() {
                                stmt.add_param(&format!("id{i}"), id);
                            }
                            total += tx.update(stmt).await?;
                        }
                        Ok::<i64, SpannerError>(total)
                    })
                })
                .await?;
            Ok(affected as u64)
        }
    }
}
