//! Repository port traits.
//!
//! Use cases depend on these traits, not on sqlx, Db, or backend-specific types.
//! Implementations are provided by storage drivers (SQLite, Postgres, Spanner)
//! and selected at startup based on the storage runtime configuration.

use crate::effect::Effect;
use crate::event::DomainEvent;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use zitadel_authz::RoleDefinition;

/// Type alias for boxed futures (object-safe async return type).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Container for all repository implementations, selected at startup.
pub struct Repositories {
    pub users: Arc<dyn UserRepository>,
    pub orgs: Arc<dyn OrgRepository>,
    pub apps: Arc<dyn AppRepository>,
    pub projects: Arc<dyn ProjectRepository>,
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
    pub memberships: Arc<dyn MembershipRepository>,
    pub console_queries: Arc<dyn ConsoleQueryRepository>,
    pub telemetry: Arc<dyn TelemetryRepository>,
    pub jobs: Arc<dyn JobRepository>,
    pub saved_queries: Arc<dyn SavedQueryRepository>,
    pub authorization: Arc<dyn AuthorizationRepository>,
    pub fga_admin: Arc<dyn FgaAdminRepository>,
    pub catalog: Arc<dyn CatalogRepository>,
    pub observability: Arc<dyn ObservabilityRepository>,
    pub schema_registry: Arc<dyn SchemaRegistryRepository>,
    pub oidc_tokens: Arc<dyn OidcTokenRepository>,
    pub oidc_keys: Arc<dyn OidcKeyRepository>,
    pub effects: Arc<dyn EffectRepository>,
    pub uow: Arc<dyn UnitOfWorkFactory>,
}

// ─── Unit of Work ──────────────────────────────────────────

/// Factory for creating transactional scopes (ADR-032 §5).
///
/// A `UnitOfWork` collects domain events and flushes them atomically
/// when `commit()` is called. State mutations go through the normal
/// repository methods; events are buffered and written in a single
/// transaction on commit.
pub trait UnitOfWorkFactory: Send + Sync {
    fn begin<'a>(
        &'a self,
        instance_id: &'a str,
    ) -> BoxFuture<'a, anyhow::Result<Box<dyn UnitOfWork>>>;
}

/// A transactional scope that buffers domain events and effects, then commits
/// them atomically. Use cases call `buffer_event()` and optionally
/// `buffer_effect()`, then `commit()` at the end.
pub trait UnitOfWork: Send {
    /// Buffer an event for atomic commit.
    fn buffer_event(
        &mut self,
        event: DomainEvent,
        request_id: Option<String>,
        session_id: Option<String>,
        flow_id: Option<String>,
    );

    /// Buffer a durable side-effect for atomic commit alongside events.
    /// Default implementation is a no-op for backward compatibility.
    fn buffer_effect(&mut self, _effect: Effect) {}

    /// Commit all buffered events and effects in a single transaction.
    /// After this call, the UnitOfWork is consumed.
    fn commit(self: Box<Self>) -> BoxFuture<'static, anyhow::Result<()>>;
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
    pub owner_org_id: Option<String>,
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
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
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default = "default_token_endpoint_auth_method")]
    pub token_endpoint_auth_method: String,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

fn default_token_endpoint_auth_method() -> String {
    "client_secret_post".to_string()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderMapping {
    #[serde(default)]
    pub claims: std::collections::HashMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderTarget {
    #[serde(default)]
    pub schema_type: String,
    #[serde(default)]
    pub schema_id: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderLinkingMode {
    #[default]
    CreateOrLink,
    LinkOnly,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMatchBy {
    #[default]
    VerifiedEmail,
    Identifier,
    None,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderLinking {
    #[serde(default)]
    pub mode: ProviderLinkingMode,
    #[serde(default)]
    pub match_by: ProviderMatchBy,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderUi {
    #[serde(default)]
    pub display_order: i64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
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
    pub session: serde_json::Value,
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

fn default_json_object() -> serde_json::Value {
    serde_json::Value::Object(Default::default())
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ProviderDefinitionRecord {
    pub id: String,
    pub org_id: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(flatten)]
    pub payload: ProviderPayload,
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
    fn delete(&self, instance_id: &str, user_id: &str) -> BoxFuture<'_, anyhow::Result<()>>;
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

    fn delete(&self, instance_id: &str, org_id: &str) -> BoxFuture<'_, anyhow::Result<bool>>;

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

    fn touch_linked_identity(
        &self,
        instance_id: &str,
        provider_id: &str,
        external_sub: &str,
        external_email: &str,
        raw_claims: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;
}

pub trait AppRepository: Send + Sync {
    fn create(
        &self,
        instance_id: &str,
        app: &AppRecord,
    ) -> BoxFuture<'_, anyhow::Result<AppRecord>>;

    fn get(
        &self,
        instance_id: &str,
        app_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<AppRecord>>>;

    fn list(
        &self,
        instance_id: &str,
        group_id: Option<&str>,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<AppRecord>>>;

    fn update_name(
        &self,
        instance_id: &str,
        app_id: &str,
        name: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>>;

    fn delete(&self, instance_id: &str, app_id: &str) -> BoxFuture<'_, anyhow::Result<bool>>;
}

pub trait ProjectRepository: Send + Sync {
    fn create(
        &self,
        instance_id: &str,
        project: &NamedResourceRecord,
        org_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<NamedResourceRecord>>;

    fn get(
        &self,
        instance_id: &str,
        project_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<NamedResourceRecord>>>;

    fn list(
        &self,
        instance_id: &str,
        params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<NamedResourceRecord>>>;

    fn update_name(
        &self,
        instance_id: &str,
        project_id: &str,
        name: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>>;

    fn delete(&self, instance_id: &str, project_id: &str) -> BoxFuture<'_, anyhow::Result<bool>>;
}

pub trait SessionRepository: Send + Sync {
    #[allow(clippy::too_many_arguments)]
    fn create(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        auth_method: &str,
        user_agent: &str,
        ip_address: &str,
        fingerprint: &str,
    ) -> BoxFuture<'_, anyhow::Result<CreatedSession>>;

    fn find_by_token(
        &self,
        instance_id: &str,
        token: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SessionInfo>>>;

    fn revoke(&self, instance_id: &str, session_id: &str) -> BoxFuture<'_, anyhow::Result<bool>>;

    fn update_metadata(
        &self,
        instance_id: &str,
        session_id: &str,
        metadata_json: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn list_by_instance(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SessionDetail>>>;

    fn get(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SessionDetail>>>;
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

#[derive(Clone, Debug)]
pub struct SessionDetail {
    pub id: String,
    pub user_id: String,
    pub org_id: String,
    pub user_agent: String,
    pub ip_address: String,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
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

    fn remove_domain(
        &self,
        instance_id: &str,
        domain: &str,
    ) -> BoxFuture<'_, anyhow::Result<DomainRemoveResult>>;
}

/// Outcome of a domain-removal attempt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DomainRemoveResult {
    Deleted,
    NotFound,
    PrimaryDomain,
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

    fn get_definition(
        &self,
        instance_id: &str,
        provider_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<ProviderDefinitionRecord>>>;

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

    /// Atomically set a flow's state and enabled flag (for promote/archive).
    fn set_state(
        &self,
        instance_id: &str,
        flow_id: &str,
        state: &str,
        enabled: bool,
    ) -> BoxFuture<'_, anyhow::Result<bool>>;
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
    pub app_id: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub redirect_uris: Vec<String>,
    pub post_logout_redirect_uris: Vec<String>,
    pub grant_types: Vec<String>,
    pub response_types: Vec<String>,
    pub state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OidcAuthRequest {
    pub id: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub client_id: String,
    pub redirect_uri: String,
    pub scope: String,
    pub state: String,
    pub nonce: Option<String>,
    pub response_type: String,
    pub code_challenge: Option<String>,
    pub code_challenge_method: Option<String>,
    pub prompt: Vec<String>,
    pub login_hint: Option<String>,
    pub max_age: Option<u64>,
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

    fn delete(&self, instance_id: &str, settings_type: &str) -> BoxFuture<'_, anyhow::Result<()>>;

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
    /// Check a relationship in the internal platform FGA store.
    fn check(
        &self,
        instance_id: &str,
        user: &str,
        relation: &str,
        object: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>>;

    /// Write relationship tuples (batch) in the internal platform FGA store.
    fn write(
        &self,
        instance_id: &str,
        writes: Vec<(String, String, String)>,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    /// Delete relationship tuples (batch) in the internal platform FGA store.
    fn delete(
        &self,
        instance_id: &str,
        deletes: Vec<(String, String, String)>,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    /// Read tuples in the internal platform FGA store matching an optional filter.
    fn read(
        &self,
        instance_id: &str,
        filter: Option<(String, String, String)>,
    ) -> BoxFuture<'_, anyhow::Result<Vec<FgaRelation>>>;
}

#[derive(Clone, Debug)]
pub struct FgaRelation {
    pub user: String,
    pub relation: String,
    pub object: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleAssignmentRecord {
    pub assignment_id: String,
    pub enforcement_instance_id: String,
    pub scope_kind: String,
    pub scope_id: String,
    pub principal_ref: String,
    pub role_key: String,
    pub source_kind: String,
    pub origin_instance_id: Option<String>,
    pub approved_by: Option<String>,
    pub reason: Option<String>,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RoleAssignmentFilter {
    pub enforcement_instance_id: Option<String>,
    pub scope_kind: Option<String>,
    pub scope_id: Option<String>,
    pub principal_ref: Option<String>,
    pub role_key: Option<String>,
    pub source_kind: Option<String>,
    pub include_revoked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InstanceTrustLinkRecord {
    pub child_instance_id: String,
    pub issuer: String,
    pub audience: String,
    pub allowed_scopes: Vec<String>,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
}

pub trait AuthorizationRepository: Send + Sync {
    fn list_role_definitions(&self) -> BoxFuture<'_, anyhow::Result<Vec<RoleDefinition>>>;

    fn create_role_assignment(
        &self,
        assignment: &RoleAssignmentRecord,
    ) -> BoxFuture<'_, anyhow::Result<RoleAssignmentRecord>>;

    fn get_role_assignment(
        &self,
        assignment_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<RoleAssignmentRecord>>>;

    fn list_role_assignments(
        &self,
        filter: &RoleAssignmentFilter,
    ) -> BoxFuture<'_, anyhow::Result<Vec<RoleAssignmentRecord>>>;

    fn revoke_role_assignment(
        &self,
        assignment_id: &str,
        revoked_at: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>>;

    fn get_instance_trust_link(
        &self,
        child_instance_id: &str,
        issuer: &str,
        audience: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<InstanceTrustLinkRecord>>>;
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

    /// Atomically promote a schema to `is_default = true`, demoting all other
    /// schemas of the same type.
    fn promote(&self, instance_id: &str, schema_id: &str) -> BoxFuture<'_, anyhow::Result<bool>>;

    /// Count users that reference this schema.
    fn count_by_schema(
        &self,
        instance_id: &str,
        schema_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<i64>>;
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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MembershipRecord {
    pub user_id: String,
    pub display_name: Option<String>,
    pub role: String,
    pub added_at: String,
}

pub trait MembershipRepository: Send + Sync {
    fn list(
        &self,
        instance_id: &str,
        entity_type: &str,
        entity_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<MembershipRecord>>>;

    fn add(
        &self,
        instance_id: &str,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
        role: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn remove(
        &self,
        instance_id: &str,
        entity_type: &str,
        entity_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NamedResourceRecord {
    pub id: String,
    pub name: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConsoleBootstrapData {
    pub counts: Vec<(String, i64)>,
    pub orgs: Vec<OrgSummary>,
    pub instance: InstanceInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrgSummary {
    pub id: String,
    pub name: String,
    pub state: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub instance_id: String,
    pub kind: String,
    pub feature_overrides_json: String,
    pub parent_instance_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FingerprintRecord {
    pub id: String,
    pub type_: String,
    pub raw_data_json: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
    pub last_run_at: Option<String>,
    pub next_run_at: Option<String>,
    pub lease_expires_at: Option<String>,
    pub config_json: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedQueryRecord {
    pub id: String,
    pub name: String,
    pub description: String,
    pub sql: String,
    pub created_at: String,
}

pub trait ConsoleQueryRepository: Send + Sync {
    fn load_console_bootstrap(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<ConsoleBootstrapData>>;

    fn load_entity_counts(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<(String, i64)>>>;
}

pub trait TelemetryRepository: Send + Sync {
    fn list_fingerprints(
        &self,
        instance_id: &str,
        cursor: &str,
        limit: i64,
    ) -> BoxFuture<'_, anyhow::Result<Vec<FingerprintRecord>>>;

    fn upsert_fingerprint(
        &self,
        instance_id: &str,
        id: &str,
        type_: &str,
        raw_data: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;
}

pub trait JobRepository: Send + Sync {
    fn list_jobs(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Vec<JobRecord>>>;
}

pub trait SavedQueryRepository: Send + Sync {
    fn list_saved_queries(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SavedQueryRecord>>>;

    fn create_saved_query(
        &self,
        instance_id: &str,
        id: &str,
        name: &str,
        description: &str,
        sql: &str,
    ) -> BoxFuture<'_, anyhow::Result<SavedQueryRecord>>;

    fn delete_saved_query(
        &self,
        instance_id: &str,
        id: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>>;
}

// ─── FGA Admin ─────────────────────────────────────────────

/// Minimal store info returned by FGA admin operations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FgaStoreInfo {
    pub id: String,
    pub name: String,
}

/// Error type for FGA admin operations, independent of the FGA engine.
#[derive(Debug, thiserror::Error)]
pub enum FgaAdminError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("forbidden: {0}")]
    Forbidden(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("internal: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Repository for FGA administrative operations exposed via the API.
/// Customer-facing FGA endpoints and internal platform FGA endpoints
/// delegate to this trait instead of calling FgaService directly.
pub trait FgaAdminRepository: Send + Sync {
    /// Discover the customer store for an instance.
    fn discover_store(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, Result<FgaStoreInfo, FgaAdminError>>;

    /// Discover the platform store.
    fn discover_platform_store(&self) -> BoxFuture<'_, Result<FgaStoreInfo, FgaAdminError>>;

    /// Check a single authorization tuple.
    fn check(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// Batch check multiple authorization tuples.
    fn batch_check(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// Read tuples from a store.
    fn read_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// Write tuples to a store.
    fn write_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<(), FgaAdminError>>;

    /// Expand a tuple.
    fn expand(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// List objects a user has access to.
    fn list_objects(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// List users that have access to an object.
    fn list_users(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// Read authorization model(s).
    fn read_model(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// List all authorization models.
    fn read_models(
        &self,
        instance_id: &str,
        store_id: &str,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// Write a new authorization model.
    fn write_model(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// Read tuple changes.
    fn read_changes(
        &self,
        instance_id: &str,
        store_id: &str,
        object_type: Option<&str>,
        page_size: u32,
        continuation_token: Option<&str>,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// Get legacy model view.
    fn legacy_model(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// Get legacy model graph view.
    fn legacy_model_graph(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>>;

    /// Rebuild the platform store (called after membership/user changes).
    fn rebuild_platform_store(&self) -> BoxFuture<'_, Result<(), FgaAdminError>>;
}

// ─── Catalog ──────────────────────────────────────────────

/// Repository for catalog template installation operations.
pub trait CatalogRepository: Send + Sync {
    /// Install a provider from a catalog template.
    /// Returns the provider ID.
    fn install_provider(
        &self,
        instance_id: &str,
        template_id: &str,
        variables: &serde_json::Value,
    ) -> BoxFuture<'_, anyhow::Result<String>>;

    /// Install an action from a catalog template.
    /// Returns the action ID.
    fn install_action(
        &self,
        instance_id: &str,
        template_id: &str,
        variables: &serde_json::Value,
    ) -> BoxFuture<'_, anyhow::Result<String>>;
}

// ─── Observability ────────────────────────────────────────

/// Observability overview data returned by the repository.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ObservabilityOverview {
    pub auth_current: i64,
    pub auth_previous: i64,
    pub tokens_current: i64,
    pub tokens_previous: i64,
    pub failures_current: i64,
    pub failures_previous: i64,
    pub sessions_current: i64,
    pub sessions_previous: i64,
    pub auth_timestamps: Vec<i64>,
    pub session_timestamps: Vec<i64>,
    pub token_timestamps: Vec<i64>,
    pub failure_timestamps: Vec<i64>,
    pub top_operations: Vec<(String, i64)>,
    pub top_users: Vec<(String, i64)>,
    pub top_ips: Vec<(String, i64)>,
    pub top_clients: Vec<(String, i64)>,
    pub top_sdks: Vec<(String, i64)>,
    pub delegation: Vec<(String, i64)>,
}

/// Repository for observability / analytics queries.
pub trait ObservabilityRepository: Send + Sync {
    /// Load the observability overview for an instance within a time range.
    fn load_overview(
        &self,
        instance_id: &str,
        range_hours: u64,
    ) -> BoxFuture<'_, anyhow::Result<ObservabilityOverview>>;
}

// ─── Schema Registry ──────────────────────────────────────

/// A schema registry entry for OpenAPI generation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SchemaRegistryEntry {
    pub id: String,
    pub type_name: String,
    pub version: i64,
    pub visibility: String,
    pub is_default: bool,
    pub schema_json: String,
}

/// Repository for schema registry lookups (used by OpenAPI generation).
pub trait SchemaRegistryRepository: Send + Sync {
    fn list_registry(
        &self,
        instance_id: &str,
        after_id: &str,
        type_filter: Option<&str>,
        limit: i64,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SchemaRegistryEntry>>>;
}

// ─── OIDC Token Storage ──────────────────────────────────

/// Stored token record for OIDC token persistence.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OidcStoredToken {
    pub token_id: String,
    pub token_type: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub client_id: String,
    pub application_id: String,
    pub scope: String,
    pub refresh_family_id: Option<String>,
}

/// New token to be persisted.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OidcNewToken {
    pub token_id: String,
    pub token_type: String,
    pub token_hash: String,
    pub user_id: Option<String>,
    pub session_id: Option<String>,
    pub client_id: String,
    pub application_id: String,
    pub scope_json: String,
    pub auth_method: String,
    pub refresh_family_id: Option<String>,
    pub expires_in_secs: u64,
}

/// Repository for OIDC token persistence.
pub trait OidcTokenRepository: Send + Sync {
    fn store_token(
        &self,
        instance_id: &str,
        token: &OidcNewToken,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn lookup_active_token(
        &self,
        instance_id: &str,
        token_hash: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OidcStoredToken>>>;

    fn revoke_token_by_id(
        &self,
        instance_id: &str,
        token_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn revoke_refresh_family(
        &self,
        instance_id: &str,
        refresh_family_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    fn revoke_session_tokens(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;
}

// ─── OIDC Key Storage ────────────────────────────────────

/// Stored signing key record.
#[derive(Clone, Debug)]
pub struct OidcSigningKeyRecord {
    pub kid: String,
    pub algorithm: String,
    pub encryption_key_id: String,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub public_key: Vec<u8>,
    pub created_at_epoch: u64,
}

/// New signing key to be persisted.
#[derive(Clone, Debug)]
pub struct OidcNewSigningKey {
    pub kid: String,
    pub algorithm: String,
    pub encryption_key_id: String,
    pub ciphertext: Vec<u8>,
    pub nonce: Vec<u8>,
    pub public_key: Vec<u8>,
    pub expires_in_secs: u64,
}

/// Repository for OIDC signing key persistence.
pub trait OidcKeyRepository: Send + Sync {
    fn list_active_keys(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<OidcSigningKeyRecord>>>;

    fn create_signing_key(
        &self,
        instance_id: &str,
        key: &OidcNewSigningKey,
    ) -> BoxFuture<'_, anyhow::Result<()>>;
}

// ─── Effects ──────────────────────────────────────────────

/// Repository for durable side-effects with retry semantics.
pub trait EffectRepository: Send + Sync {
    /// Insert effects (typically in the same transaction as events).
    /// Implementations must be idempotent by `(instance_id, source_key)`.
    fn enqueue_batch(
        &self,
        instance_id: &str,
        effects: &[Effect],
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    /// Claim due effects for dispatch by assigning a lease atomically.
    fn claim_due(
        &self,
        instance_id: &str,
        worker_id: &str,
        lease_ttl_secs: u64,
        limit: u32,
    ) -> BoxFuture<'_, anyhow::Result<Vec<Effect>>>;

    /// Mark an effect as successfully delivered.
    fn mark_completed(
        &self,
        instance_id: &str,
        effect_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    /// Record a failed attempt and schedule retry with backoff.
    fn record_failure(
        &self,
        instance_id: &str,
        effect_id: &str,
        error: &str,
        next_retry_at: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    /// Mark as dead (max attempts exhausted, will not be retried).
    fn mark_dead(
        &self,
        instance_id: &str,
        effect_id: &str,
        error: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>>;

    /// Delete completed/dead effects older than the given cutoff.
    fn cleanup(
        &self,
        instance_id: &str,
        older_than: &str,
        limit: u32,
    ) -> BoxFuture<'_, anyhow::Result<u64>>;
}
