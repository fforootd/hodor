//! Repository port traits.
//!
//! Use cases depend on these traits, not on sqlx, Db, or backend-specific types.
//! Implementations are provided by storage drivers (SQLite, Postgres, Spanner)
//! and selected at startup based on the storage runtime configuration.

use crate::event::DomainEvent;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Type alias for boxed futures (object-safe async return type).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Container for all repository implementations, selected at startup.
pub struct Repositories {
    pub users: Arc<dyn UserRepository>,
    pub orgs: Arc<dyn OrgRepository>,
    pub credentials: Arc<dyn CredentialRepository>,
    pub sessions: Arc<dyn SessionRepository>,
    pub instances: Arc<dyn InstanceRepository>,
    pub providers: Arc<dyn ProviderRepository>,
    pub login_flows: Arc<dyn LoginFlowRepository>,
    pub oidc: Arc<dyn OidcRepository>,
    pub events: Arc<dyn EventRepository>,
    pub settings: Arc<dyn SettingsRepository>,
    pub fga: Arc<dyn FgaRepository>,
    pub schemas: Arc<dyn SchemaRepository>,
    pub groups: Arc<dyn GroupRepository>,
    pub pats: Arc<dyn PatRepository>,
    pub search: Arc<dyn SearchRepository>,
    pub actions: Arc<dyn ActionRepository>,
}

// ─── Shared record types ───
// These are the domain-facing record types that repository traits return.
// They are intentionally separate from DB row types to allow the driver
// to map between SQL rows and these structs.

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserRecord {
    pub id: String,
    pub org_id: String,
    pub identifier: String,
    pub display_name: String,
    pub user_type: String,
    pub state: String,
    pub schema_id: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrgRecord {
    pub id: String,
    pub name: String,
    pub state: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroupRecord {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub state: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppRecord {
    pub id: String,
    pub group_id: String,
    pub name: String,
    pub protocol: String,
    pub state: String,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceRecord {
    pub instance_id: String,
    pub state: String,
    pub kind: String,
    pub placement_mode: String,
    pub region_key: Option<String>,
    pub owner_org_id: String,
    pub feature_overrides: serde_json::Value,
    pub primary_domain: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DomainRecord {
    pub domain: String,
    pub is_primary: bool,
    pub state: String,
    pub verified: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderRecord {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub state: String,
    pub config: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LinkedIdentityRecord {
    pub id: String,
    pub user_id: String,
    pub provider_id: String,
    pub external_sub: String,
    pub external_email: Option<String>,
    pub raw_claims: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchemaRecord {
    pub id: String,
    pub schema_type: String,
    pub schema_json: serde_json::Value,
    pub version: i64,
    pub is_default: bool,
    pub visibility: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SettingsRecord {
    pub settings_type: String,
    pub scope: String,
    pub data: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginFlowRecord {
    pub id: String,
    pub name: String,
    pub strategy: String,
    pub state: String,
    pub is_default: bool,
    pub enabled: bool,
    pub priority: i32,
    pub config: serde_json::Value,
    pub audience: serde_json::Value,
    pub auth_methods: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PatRecord {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionRecord {
    pub id: String,
    pub org_id: String,
    pub name: String,
    pub hook: String,
    pub action_type: String,
    pub trigger_expr: String,
    pub config: serde_json::Value,
    pub priority: i32,
    pub enabled: bool,
    pub fail_open: bool,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: String,
    pub event_type: String,
    pub category: String,
    pub org_id: String,
    pub actor_id: Option<String>,
    pub aggregate_id: Option<String>,
    pub aggregate_type: Option<String>,
    pub resource_type: Option<String>,
    pub payload: serde_json::Value,
    pub metadata: serde_json::Value,
    pub request_id: Option<String>,
    pub session_id: Option<String>,
    pub flow_id: Option<String>,
    pub sequence: Option<i64>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub resource_type: String,
    pub id: String,
    pub title: String,
    pub subtitle: Option<String>,
}

// ─── Pagination ───

#[derive(Clone, Debug, Default)]
pub struct ListParams {
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    pub search: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ListResult<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub total_count: Option<u64>,
}

// ─── Repository Traits ───

pub trait UserRepository: Send + Sync {
    fn create(
        &self,
        instance_id: &str,
        user: &UserRecord,
    ) -> BoxFuture<'_, anyhow::Result<UserRecord>>;

    fn get(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserRecord>>>;

    fn find_by_identifier(
        &self,
        instance_id: &str,
        identifier: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserRecord>>>;

    fn list(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<UserRecord>>>;

    fn update(
        &self,
        instance_id: &str,
        user: &UserRecord,
    ) -> BoxFuture<'_, anyhow::Result<UserRecord>>;

    fn deactivate(&self, instance_id: &str, user_id: &str) -> BoxFuture<'_, anyhow::Result<()>>;
}

pub trait OrgRepository: Send + Sync {
    fn create(
        &self,
        instance_id: &str,
        org: &OrgRecord,
    ) -> BoxFuture<'_, anyhow::Result<OrgRecord>>;

    fn get(
        &self,
        instance_id: &str,
        org_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OrgRecord>>>;

    fn list(
        &self,
        instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<OrgRecord>>>;

    fn update(
        &self,
        instance_id: &str,
        org: &OrgRecord,
    ) -> BoxFuture<'_, anyhow::Result<OrgRecord>>;

    fn first_org_id(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Option<String>>>;
}

pub trait CredentialRepository: Send + Sync {
    fn set_password(
        &self,
        instance_id: &str,
        user_id: &str,
        password_hash: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn get_password_hash(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<String>>>;

    fn link_identity(
        &self,
        instance_id: &str,
        link: &LinkedIdentityRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn unlink_identity(
        &self,
        instance_id: &str,
        user_id: &str,
        provider_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn list_linked_identities(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<LinkedIdentityRecord>>>;

    fn find_by_external_sub(
        &self,
        instance_id: &str,
        provider_id: &str,
        external_sub: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<LinkedIdentityRecord>>>;
}

pub trait SessionRepository: Send + Sync {
    fn create(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        auth_method: &str,
    ) -> BoxFuture<'_, anyhow::Result<CreatedSession>>;

    fn find_by_token(
        &self,
        instance_id: &str,
        token: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SessionInfo>>>;

    fn revoke(&self, instance_id: &str, session_id: &str) -> BoxFuture<'_, anyhow::Result<bool>>;
}

#[derive(Clone, Debug)]
pub struct CreatedSession {
    pub session_id: String,
    pub token: String,
}

#[derive(Clone, Debug)]
pub struct SessionInfo {
    pub session_id: String,
    pub user_id: String,
    pub org_id: String,
    pub token_type: String,
}

pub trait InstanceRepository: Send + Sync {
    fn create(
        &self,
        root_instance_id: &str,
        instance: &InstanceRecord,
    ) -> BoxFuture<'_, anyhow::Result<InstanceRecord>>;

    fn get(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Option<InstanceRecord>>>;

    fn list(
        &self,
        root_instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<InstanceRecord>>>;

    fn update(&self, instance: &InstanceRecord) -> BoxFuture<'_, anyhow::Result<InstanceRecord>>;

    fn deprovision(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<()>>;

    fn resolve_domain(
        &self,
        domain: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<RouteResolution>>>;

    fn list_domains(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Vec<DomainRecord>>>;

    fn set_domain(
        &self,
        instance_id: &str,
        domain: &DomainRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>>;
}

#[derive(Clone, Debug)]
pub struct RouteResolution {
    pub instance_id: String,
    pub resolved_org_id: Option<String>,
    pub placement_mode: String,
    pub region_key: Option<String>,
}

pub trait ProviderRepository: Send + Sync {
    fn create(
        &self,
        instance_id: &str,
        provider: &ProviderRecord,
    ) -> BoxFuture<'_, anyhow::Result<ProviderRecord>>;

    fn get(
        &self,
        instance_id: &str,
        provider_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<ProviderRecord>>>;

    fn list(
        &self,
        instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<ProviderRecord>>>;

    fn update(
        &self,
        instance_id: &str,
        provider: &ProviderRecord,
    ) -> BoxFuture<'_, anyhow::Result<ProviderRecord>>;

    fn delete(&self, instance_id: &str, provider_id: &str) -> BoxFuture<'_, anyhow::Result<()>>;
}

pub trait LoginFlowRepository: Send + Sync {
    fn get_flow(
        &self,
        instance_id: &str,
        flow_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<LoginFlowRecord>>>;

    fn list_flows(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Vec<LoginFlowRecord>>>;

    fn upsert_flow(
        &self,
        instance_id: &str,
        flow: &LoginFlowRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn delete_flow(&self, instance_id: &str, flow_id: &str) -> BoxFuture<'_, anyhow::Result<()>>;
}

pub trait OidcRepository: Send + Sync {
    fn find_client(
        &self,
        instance_id: &str,
        client_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OidcClientInfo>>>;

    fn create_auth_request(
        &self,
        instance_id: &str,
        request: &OidcAuthRequest,
    ) -> BoxFuture<'_, anyhow::Result<String>>;

    fn consume_auth_code(
        &self,
        instance_id: &str,
        code: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OidcAuthRequest>>>;

    fn load_user_claims(
        &self,
        instance_id: &str,
        subject: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserClaims>>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OidcClientInfo {
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OidcAuthRequest {
    pub id: String,
    pub user_id: Option<String>,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub nonce: Option<String>,
    pub code_challenge: Option<String>,
    pub auth_time: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserClaims {
    pub sub: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
}

pub trait EventRepository: Send + Sync {
    /// Append a domain event. In repository implementations backed by SQL,
    /// this MUST be called within the same transaction as the state change.
    fn append(
        &self,
        instance_id: &str,
        event: &DomainEvent,
        request_id: Option<&str>,
        session_id: Option<&str>,
        flow_id: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn list(
        &self,
        instance_id: &str,
        params: &EventQueryParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<EventRecord>>>;
}

#[derive(Clone, Debug, Default)]
pub struct EventQueryParams {
    pub event_type: Option<String>,
    pub category: Option<String>,
    pub aggregate_id: Option<String>,
    pub session_id: Option<String>,
    pub limit: Option<u32>,
    pub cursor: Option<String>,
}

pub trait SettingsRepository: Send + Sync {
    fn get(
        &self,
        instance_id: &str,
        settings_type: &str,
        scope: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SettingsRecord>>>;

    fn set(
        &self,
        instance_id: &str,
        settings: &SettingsRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    /// Resolve settings with cascade: instance defaults → org overrides → app overrides.
    fn resolve(
        &self,
        instance_id: &str,
        settings_type: &str,
        org_id: Option<&str>,
        app_id: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<SettingsRecord>>;
}

pub trait FgaRepository: Send + Sync {
    fn check(
        &self,
        instance_id: &str,
        user: &str,
        relation: &str,
        object: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>>;

    fn write_tuple(
        &self,
        instance_id: &str,
        user: &str,
        relation: &str,
        object: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn delete_tuple(
        &self,
        instance_id: &str,
        user: &str,
        relation: &str,
        object: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn list_relations(
        &self,
        instance_id: &str,
        user: &str,
        object_type: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<FgaRelation>>>;
}

#[derive(Clone, Debug)]
pub struct FgaRelation {
    pub user: String,
    pub relation: String,
    pub object: String,
}

pub trait SchemaRepository: Send + Sync {
    fn register(
        &self,
        instance_id: &str,
        schema: &SchemaRecord,
    ) -> BoxFuture<'_, anyhow::Result<SchemaRecord>>;

    fn get(
        &self,
        instance_id: &str,
        schema_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SchemaRecord>>>;

    fn get_by_type(
        &self,
        instance_id: &str,
        schema_type: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SchemaRecord>>>;

    fn list(
        &self,
        instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<SchemaRecord>>>;

    fn update(
        &self,
        instance_id: &str,
        schema: &SchemaRecord,
    ) -> BoxFuture<'_, anyhow::Result<SchemaRecord>>;
}

pub trait GroupRepository: Send + Sync {
    fn create(
        &self,
        instance_id: &str,
        group: &GroupRecord,
    ) -> BoxFuture<'_, anyhow::Result<GroupRecord>>;

    fn get(
        &self,
        instance_id: &str,
        group_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<GroupRecord>>>;

    fn list(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<GroupRecord>>>;

    fn update(
        &self,
        instance_id: &str,
        group: &GroupRecord,
    ) -> BoxFuture<'_, anyhow::Result<GroupRecord>>;

    fn delete(&self, instance_id: &str, group_id: &str) -> BoxFuture<'_, anyhow::Result<()>>;

    fn add_member(
        &self,
        instance_id: &str,
        group_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn remove_member(
        &self,
        instance_id: &str,
        group_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;
}

pub trait PatRepository: Send + Sync {
    fn create(
        &self,
        instance_id: &str,
        pat: &PatRecord,
        token_hash: &str,
    ) -> BoxFuture<'_, anyhow::Result<String>>;

    fn get(
        &self,
        instance_id: &str,
        pat_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<PatRecord>>>;

    fn list(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<PatRecord>>>;

    fn revoke(&self, instance_id: &str, pat_id: &str) -> BoxFuture<'_, anyhow::Result<()>>;

    fn resolve_token(
        &self,
        instance_id: &str,
        raw_token: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<ResolvedPat>>>;
}

#[derive(Clone, Debug)]
pub struct ResolvedPat {
    pub user_id: String,
    pub session_id: String,
    pub org_id: String,
}

pub trait SearchRepository: Send + Sync {
    fn search(
        &self,
        instance_id: &str,
        query: &str,
        resource_types: Option<&[&str]>,
        limit: Option<u32>,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SearchResult>>>;
}

pub trait ActionRepository: Send + Sync {
    fn list(
        &self,
        instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<ActionRecord>>>;

    fn get(
        &self,
        instance_id: &str,
        action_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<ActionRecord>>>;

    fn create(
        &self,
        instance_id: &str,
        action: &ActionRecord,
    ) -> BoxFuture<'_, anyhow::Result<ActionRecord>>;

    fn update(
        &self,
        instance_id: &str,
        action: &ActionRecord,
    ) -> BoxFuture<'_, anyhow::Result<ActionRecord>>;

    fn delete(&self, instance_id: &str, action_id: &str) -> BoxFuture<'_, anyhow::Result<()>>;
}
