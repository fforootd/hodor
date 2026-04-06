mod auth;
mod authz;
mod instances;
mod orgs;
mod schemas;
mod users;

use std::collections::BTreeMap;

use anyhow::Context;
use google_cloud_spanner::{row::Row, statement::Statement};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ─── Re-exports from submodules ─────────────────────────────

pub use auth::*;
pub use authz::*;
pub use instances::*;
pub use orgs::*;
pub use schemas::*;
pub use users::*;

// ─── Shared type definitions ────────────────────────────────

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
pub struct OrgRoleMembershipRecord {
    pub org_id: String,
    pub user_id: String,
    pub role: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildInstanceOwnershipRecord {
    pub instance_id: String,
    pub owner_org_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OidcClientRecord {
    pub app_id: String,
    pub client_secret: String,
    pub redirect_uris_json: String,
    pub post_logout_redirect_uris_json: String,
    pub grant_types_json: String,
    pub response_types_json: String,
    pub state: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OidcAuthRequestRecord {
    pub auth_request_id: String,
    pub user_id: String,
    pub session_id: String,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub nonce: String,
    pub response_type: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub prompt_json: String,
    pub login_hint: String,
    pub max_age: Option<i64>,
    pub auth_time: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserClaimsRecord {
    pub identifier: String,
    pub display_name: String,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MembershipRow {
    pub user_id: String,
    pub display_name: Option<String>,
    pub role: String,
    pub added_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveRoleBindingRecord {
    pub principal_ref: String,
    pub role_key: String,
}

// ─── Shared helper functions (used across submodules) ───────

pub(super) fn metadata_has_capability(metadata_json: &str, capability: &str) -> bool {
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

#[allow(clippy::type_complexity)]
pub(super) fn instance_from_sql_row(
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

pub(super) fn instance_from_spanner_row(row: Row) -> ManagedInstanceRecord {
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

pub(super) fn action_from_sql_row(
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

pub(super) fn action_from_spanner_row(row: Row) -> ActionRecord {
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

pub(super) fn login_flow_from_sql_row(
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

pub(super) fn login_flow_from_spanner_row(row: Row) -> LoginFlowRecord {
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

pub(super) async fn spanner_query_all(
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

pub(super) async fn spanner_query_optional(
    spanner: &crate::SpannerDb,
    stmt: Statement,
) -> anyhow::Result<Option<Row>> {
    let mut tx = spanner.client().single().await?;
    let mut rows = tx.query(stmt).await?;
    Ok(rows.next().await?)
}

pub(super) async fn spanner_query_scalar_i64(
    spanner: &crate::SpannerDb,
    stmt: Statement,
) -> anyhow::Result<i64> {
    let row = spanner_query_optional(spanner, stmt)
        .await?
        .context("spanner scalar query returned no row")?;
    row.column_by_name::<i64>("total")
        .map_err(anyhow::Error::from)
}
