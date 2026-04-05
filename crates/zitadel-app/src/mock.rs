//! Mock repository implementations for testing use cases.
//!
//! Each mock stores data in memory behind a `Mutex<HashMap>` so tests can
//! verify business logic without a real database.

use crate::event::DomainEvent;
use crate::repo::*;
use std::collections::HashMap;
use std::sync::Mutex;

// ─── MockUserRepository ──────────────────────────────────

pub struct MockUserRepository {
    store: Mutex<HashMap<(String, String), UserRecord>>,
}

impl MockUserRepository {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for MockUserRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl UserRepository for MockUserRepository {
    fn create(
        &self,
        instance_id: &str,
        user: &UserRecord,
    ) -> BoxFuture<'_, anyhow::Result<UserRecord>> {
        let key = (instance_id.to_string(), user.id.clone());
        let user = user.clone();
        Box::pin(async move {
            self.store.lock().unwrap().insert(key, user.clone());
            Ok(user)
        })
    }

    fn get(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserRecord>>> {
        let key = (instance_id.to_string(), user_id.to_string());
        Box::pin(async move { Ok(self.store.lock().unwrap().get(&key).cloned()) })
    }

    fn find_by_identifier(
        &self,
        instance_id: &str,
        identifier: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserRecord>>> {
        let instance_id = instance_id.to_string();
        let identifier = identifier.to_string();
        Box::pin(async move {
            let guard = self.store.lock().unwrap();
            Ok(guard
                .iter()
                .find(|(k, v)| k.0 == instance_id && v.identifier == identifier)
                .map(|(_, v)| v.clone()))
        })
    }

    fn list(
        &self,
        instance_id: &str,
        _org_id: Option<&str>,
        _params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<UserRecord>>> {
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            let items: Vec<UserRecord> = self
                .store
                .lock()
                .unwrap()
                .iter()
                .filter(|(k, _)| k.0 == instance_id)
                .map(|(_, v)| v.clone())
                .collect();
            Ok(ListResult {
                items,
                next_cursor: None,
                total_count: None,
            })
        })
    }

    fn update(
        &self,
        instance_id: &str,
        user: &UserRecord,
    ) -> BoxFuture<'_, anyhow::Result<UserRecord>> {
        let key = (instance_id.to_string(), user.id.clone());
        let user = user.clone();
        Box::pin(async move {
            self.store.lock().unwrap().insert(key, user.clone());
            Ok(user)
        })
    }

    fn deactivate(&self, instance_id: &str, user_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let key = (instance_id.to_string(), user_id.to_string());
        Box::pin(async move {
            if let Some(user) = self.store.lock().unwrap().get_mut(&key) {
                user.state = "deactivated".to_string();
            }
            Ok(())
        })
    }
}

// ─── MockOrgRepository ───────────────────────────────────

pub struct MockOrgRepository {
    store: Mutex<HashMap<(String, String), OrgRecord>>,
}

impl MockOrgRepository {
    pub fn new() -> Self {
        Self {
            store: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for MockOrgRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl OrgRepository for MockOrgRepository {
    fn create(
        &self,
        instance_id: &str,
        org: &OrgRecord,
    ) -> BoxFuture<'_, anyhow::Result<OrgRecord>> {
        let key = (instance_id.to_string(), org.id.clone());
        let org = org.clone();
        Box::pin(async move {
            self.store.lock().unwrap().insert(key, org.clone());
            Ok(org)
        })
    }

    fn get(
        &self,
        instance_id: &str,
        org_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OrgRecord>>> {
        let key = (instance_id.to_string(), org_id.to_string());
        Box::pin(async move { Ok(self.store.lock().unwrap().get(&key).cloned()) })
    }

    fn list(
        &self,
        instance_id: &str,
        _params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<OrgRecord>>> {
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            let items: Vec<OrgRecord> = self
                .store
                .lock()
                .unwrap()
                .iter()
                .filter(|(k, _)| k.0 == instance_id)
                .map(|(_, v)| v.clone())
                .collect();
            Ok(ListResult {
                items,
                next_cursor: None,
                total_count: None,
            })
        })
    }

    fn update(
        &self,
        instance_id: &str,
        org: &OrgRecord,
    ) -> BoxFuture<'_, anyhow::Result<OrgRecord>> {
        let key = (instance_id.to_string(), org.id.clone());
        let org = org.clone();
        Box::pin(async move {
            self.store.lock().unwrap().insert(key, org.clone());
            Ok(org)
        })
    }

    fn first_org_id(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Option<String>>> {
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            Ok(self
                .store
                .lock()
                .unwrap()
                .iter()
                .find(|(k, _)| k.0 == instance_id)
                .map(|(_, v)| v.id.clone()))
        })
    }
}

// ─── MockEventRepository ─────────────────────────────────

pub struct MockEventRepository {
    pub events: Mutex<Vec<(String, DomainEvent)>>,
}

impl MockEventRepository {
    pub fn new() -> Self {
        Self {
            events: Mutex::new(Vec::new()),
        }
    }
}

impl Default for MockEventRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl EventRepository for MockEventRepository {
    fn append(
        &self,
        instance_id: &str,
        event: &DomainEvent,
        _request_id: Option<&str>,
        _session_id: Option<&str>,
        _flow_id: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let instance_id = instance_id.to_string();
        let event = event.clone();
        Box::pin(async move {
            self.events.lock().unwrap().push((instance_id, event));
            Ok(())
        })
    }

    fn list(
        &self,
        _instance_id: &str,
        _params: &EventQueryParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<EventRecord>>> {
        Box::pin(async move {
            Ok(ListResult {
                items: vec![],
                next_cursor: None,
                total_count: None,
            })
        })
    }
}

// ─── Noop implementations for repos not tested in use case unit tests ─────

macro_rules! noop_repo {
    ($name:ident, $trait:ident { $($method:ident ( $($arg:ident : $ty:ty),* ) -> $ret:ty;)* }) => {
        pub struct $name;
        impl $trait for $name {
            $(
                fn $method(&self, $($arg: $ty),*) -> BoxFuture<'_, anyhow::Result<$ret>> {
                    Box::pin(async move { anyhow::bail!(concat!(stringify!($name), "::", stringify!($method), " not implemented in mock")) })
                }
            )*
        }
    };
}

// Credential
pub struct MockCredentialRepository {
    passwords: Mutex<HashMap<(String, String), String>>,
}

impl MockCredentialRepository {
    pub fn new() -> Self {
        Self {
            passwords: Mutex::new(HashMap::new()),
        }
    }
}

impl Default for MockCredentialRepository {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialRepository for MockCredentialRepository {
    fn set_password(
        &self,
        instance_id: &str,
        user_id: &str,
        password_hash: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let key = (instance_id.to_string(), user_id.to_string());
        let hash = password_hash.to_string();
        Box::pin(async move {
            self.passwords.lock().unwrap().insert(key, hash);
            Ok(())
        })
    }

    fn get_password_hash(
        &self,
        instance_id: &str,
        user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<String>>> {
        let key = (instance_id.to_string(), user_id.to_string());
        Box::pin(async move { Ok(self.passwords.lock().unwrap().get(&key).cloned()) })
    }

    fn link_identity(
        &self,
        _instance_id: &str,
        _link: &LinkedIdentityRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn unlink_identity(
        &self,
        _instance_id: &str,
        _user_id: &str,
        _provider_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn list_linked_identities(
        &self,
        _instance_id: &str,
        _user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<LinkedIdentityRecord>>> {
        Box::pin(async { Ok(vec![]) })
    }

    fn find_by_external_sub(
        &self,
        _instance_id: &str,
        _provider_id: &str,
        _external_sub: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<LinkedIdentityRecord>>> {
        Box::pin(async { Ok(None) })
    }
}

// Session
pub struct MockSessionRepository;

impl SessionRepository for MockSessionRepository {
    fn create(
        &self,
        _instance_id: &str,
        _user_id: &str,
        _org_id: &str,
        _auth_method: &str,
    ) -> BoxFuture<'_, anyhow::Result<CreatedSession>> {
        Box::pin(async {
            Ok(CreatedSession {
                session_id: uuid::Uuid::new_v4().to_string(),
                token: format!("mock-token-{}", uuid::Uuid::new_v4()),
            })
        })
    }

    fn find_by_token(
        &self,
        _instance_id: &str,
        _token: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SessionInfo>>> {
        Box::pin(async { Ok(None) })
    }

    fn revoke(&self, _instance_id: &str, _session_id: &str) -> BoxFuture<'_, anyhow::Result<bool>> {
        Box::pin(async { Ok(true) })
    }
}

// FGA
pub struct MockFgaRepository {
    pub allow_all: bool,
}

impl MockFgaRepository {
    pub fn allow_all() -> Self {
        Self { allow_all: true }
    }
}

impl FgaRepository for MockFgaRepository {
    fn check(
        &self,
        _instance_id: &str,
        _user: &str,
        _relation: &str,
        _object: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let allowed = self.allow_all;
        Box::pin(async move { Ok(allowed) })
    }

    fn write_tuple(
        &self,
        _instance_id: &str,
        _user: &str,
        _relation: &str,
        _object: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn delete_tuple(
        &self,
        _instance_id: &str,
        _user: &str,
        _relation: &str,
        _object: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn list_relations(
        &self,
        _instance_id: &str,
        _user: &str,
        _object_type: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<FgaRelation>>> {
        Box::pin(async { Ok(vec![]) })
    }
}

// Simple noop mocks for repos not under direct test
pub struct NoopInstanceRepository;
impl InstanceRepository for NoopInstanceRepository {
    fn create(&self, _: &str, _: &InstanceRecord) -> BoxFuture<'_, anyhow::Result<InstanceRecord>> {
        Box::pin(async { anyhow::bail!("noop") })
    }
    fn get(&self, _: &str) -> BoxFuture<'_, anyhow::Result<Option<InstanceRecord>>> {
        Box::pin(async { Ok(None) })
    }
    fn list(
        &self,
        _: &str,
        _: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<InstanceRecord>>> {
        Box::pin(async {
            Ok(ListResult {
                items: vec![],
                next_cursor: None,
                total_count: None,
            })
        })
    }
    fn update(&self, _: &InstanceRecord) -> BoxFuture<'_, anyhow::Result<InstanceRecord>> {
        Box::pin(async { anyhow::bail!("noop") })
    }
    fn deprovision(&self, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn resolve_domain(&self, _: &str) -> BoxFuture<'_, anyhow::Result<Option<RouteResolution>>> {
        Box::pin(async { Ok(None) })
    }
    fn list_domains(&self, _: &str) -> BoxFuture<'_, anyhow::Result<Vec<DomainRecord>>> {
        Box::pin(async { Ok(vec![]) })
    }
    fn set_domain(&self, _: &str, _: &DomainRecord) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

pub struct NoopProviderRepository;
impl ProviderRepository for NoopProviderRepository {
    fn create(&self, _: &str, _: &ProviderRecord) -> BoxFuture<'_, anyhow::Result<ProviderRecord>> {
        Box::pin(async { anyhow::bail!("noop") })
    }
    fn get(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<ProviderRecord>>> {
        Box::pin(async { Ok(None) })
    }
    fn list(
        &self,
        _: &str,
        _: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<ProviderRecord>>> {
        Box::pin(async {
            Ok(ListResult {
                items: vec![],
                next_cursor: None,
                total_count: None,
            })
        })
    }
    fn update(&self, _: &str, _: &ProviderRecord) -> BoxFuture<'_, anyhow::Result<ProviderRecord>> {
        Box::pin(async { anyhow::bail!("noop") })
    }
    fn delete(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

pub struct NoopLoginFlowRepository;
impl LoginFlowRepository for NoopLoginFlowRepository {
    fn get_flow(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<LoginFlowRecord>>> {
        Box::pin(async { Ok(None) })
    }
    fn list_flows(&self, _: &str) -> BoxFuture<'_, anyhow::Result<Vec<LoginFlowRecord>>> {
        Box::pin(async { Ok(vec![]) })
    }
    fn upsert_flow(&self, _: &str, _: &LoginFlowRecord) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn delete_flow(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

pub struct NoopOidcRepository;
impl OidcRepository for NoopOidcRepository {
    fn find_client(
        &self,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OidcClientInfo>>> {
        Box::pin(async { Ok(None) })
    }
    fn create_auth_request(
        &self,
        _: &str,
        _: &OidcAuthRequest,
    ) -> BoxFuture<'_, anyhow::Result<String>> {
        Box::pin(async { Ok(String::new()) })
    }
    fn consume_auth_code(
        &self,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OidcAuthRequest>>> {
        Box::pin(async { Ok(None) })
    }
    fn load_user_claims(
        &self,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserClaims>>> {
        Box::pin(async { Ok(None) })
    }
}

pub struct NoopSettingsRepository;
impl SettingsRepository for NoopSettingsRepository {
    fn get(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SettingsRecord>>> {
        Box::pin(async { Ok(None) })
    }
    fn set(&self, _: &str, _: &SettingsRecord) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn resolve(
        &self,
        _: &str,
        st: &str,
        _: Option<&str>,
        _: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<SettingsRecord>> {
        let st = st.to_string();
        Box::pin(async move {
            Ok(SettingsRecord {
                settings_type: st,
                scope: "instance".into(),
                data: serde_json::json!({}),
            })
        })
    }
}

pub struct NoopSchemaRepository;
impl SchemaRepository for NoopSchemaRepository {
    fn register(&self, _: &str, _: &SchemaRecord) -> BoxFuture<'_, anyhow::Result<SchemaRecord>> {
        Box::pin(async { anyhow::bail!("noop") })
    }
    fn get(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<SchemaRecord>>> {
        Box::pin(async { Ok(None) })
    }
    fn get_by_type(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<SchemaRecord>>> {
        Box::pin(async { Ok(None) })
    }
    fn list(
        &self,
        _: &str,
        _: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<SchemaRecord>>> {
        Box::pin(async {
            Ok(ListResult {
                items: vec![],
                next_cursor: None,
                total_count: None,
            })
        })
    }
    fn update(&self, _: &str, _: &SchemaRecord) -> BoxFuture<'_, anyhow::Result<SchemaRecord>> {
        Box::pin(async { anyhow::bail!("noop") })
    }
}

pub struct NoopGroupRepository;
impl GroupRepository for NoopGroupRepository {
    fn create(&self, _: &str, _: &GroupRecord) -> BoxFuture<'_, anyhow::Result<GroupRecord>> {
        Box::pin(async { anyhow::bail!("noop") })
    }
    fn get(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<GroupRecord>>> {
        Box::pin(async { Ok(None) })
    }
    fn list(
        &self,
        _: &str,
        _: Option<&str>,
        _: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<GroupRecord>>> {
        Box::pin(async {
            Ok(ListResult {
                items: vec![],
                next_cursor: None,
                total_count: None,
            })
        })
    }
    fn update(&self, _: &str, _: &GroupRecord) -> BoxFuture<'_, anyhow::Result<GroupRecord>> {
        Box::pin(async { anyhow::bail!("noop") })
    }
    fn delete(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn add_member(&self, _: &str, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn remove_member(&self, _: &str, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

pub struct NoopPatRepository;
impl PatRepository for NoopPatRepository {
    fn create(&self, _: &str, _: &PatRecord, _: &str) -> BoxFuture<'_, anyhow::Result<String>> {
        Box::pin(async { Ok(String::new()) })
    }
    fn get(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<PatRecord>>> {
        Box::pin(async { Ok(None) })
    }
    fn list(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Vec<PatRecord>>> {
        Box::pin(async { Ok(vec![]) })
    }
    fn revoke(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
    fn resolve_token(
        &self,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<ResolvedPat>>> {
        Box::pin(async { Ok(None) })
    }
}

pub struct NoopSearchRepository;
impl SearchRepository for NoopSearchRepository {
    fn search(
        &self,
        _: &str,
        _: &str,
        _: Option<&[&str]>,
        _: Option<u32>,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SearchResult>>> {
        Box::pin(async { Ok(vec![]) })
    }
}

pub struct NoopActionRepository;
impl ActionRepository for NoopActionRepository {
    fn list(
        &self,
        _: &str,
        _: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<ActionRecord>>> {
        Box::pin(async {
            Ok(ListResult {
                items: vec![],
                next_cursor: None,
                total_count: None,
            })
        })
    }
    fn get(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<ActionRecord>>> {
        Box::pin(async { Ok(None) })
    }
    fn create(&self, _: &str, _: &ActionRecord) -> BoxFuture<'_, anyhow::Result<ActionRecord>> {
        Box::pin(async { anyhow::bail!("noop") })
    }
    fn update(&self, _: &str, _: &ActionRecord) -> BoxFuture<'_, anyhow::Result<ActionRecord>> {
        Box::pin(async { anyhow::bail!("noop") })
    }
    fn delete(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

/// Build a `Repositories` container with all mock implementations.
/// Useful for use case unit tests.
pub fn mock_repositories() -> Repositories {
    Repositories {
        users: std::sync::Arc::new(MockUserRepository::new()),
        orgs: std::sync::Arc::new(MockOrgRepository::new()),
        credentials: std::sync::Arc::new(MockCredentialRepository::new()),
        sessions: std::sync::Arc::new(MockSessionRepository),
        instances: std::sync::Arc::new(NoopInstanceRepository),
        providers: std::sync::Arc::new(NoopProviderRepository),
        login_flows: std::sync::Arc::new(NoopLoginFlowRepository),
        oidc: std::sync::Arc::new(NoopOidcRepository),
        events: std::sync::Arc::new(MockEventRepository::new()),
        settings: std::sync::Arc::new(NoopSettingsRepository),
        fga: std::sync::Arc::new(MockFgaRepository::allow_all()),
        schemas: std::sync::Arc::new(NoopSchemaRepository),
        groups: std::sync::Arc::new(NoopGroupRepository),
        pats: std::sync::Arc::new(NoopPatRepository),
        search: std::sync::Arc::new(NoopSearchRepository),
        actions: std::sync::Arc::new(NoopActionRepository),
    }
}
