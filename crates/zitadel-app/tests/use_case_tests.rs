//! Use case unit tests with mock repositories (ADR-032 CLAUDE-4).
//!
//! These tests verify business logic in isolation — no database, no HTTP.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use zitadel_app::ApplicationServices;
use zitadel_app::context::{ActorContext, AuthContext, Capability, Identity, InstanceContext};
use zitadel_app::effect::Effect;
use zitadel_app::error::AppError;
use zitadel_app::hook::HookPipeline;
use zitadel_app::repo::{
    AppRecord, AppRepository, BoxFuture, ConsoleBootstrapData, ConsoleQueryRepository,
    CreatedSession, DomainRecord, DomainRemoveResult, EffectRepository, FingerprintRecord,
    GroupRecord, GroupRepository, InstanceInfo, InstanceRecord, InstanceRepository, JobRecord,
    JobRepository, ListParams, ListResult, NamedResourceRecord, OrgSummary, ProjectRepository,
    Repositories, RouteResolution, SavedQueryRecord, SavedQueryRepository, SessionDetail,
    SessionInfo, SessionRepository, TelemetryRepository,
};
use zitadel_app::users::CreateUserCommand;
use zitadel_app::{
    domains::AddCustomDomainCommand,
    groups::{CreateGroupCommand, UpdateGroupCommand},
    instances::{CreateInstanceCommand, UpdateInstanceCommand},
    resources::{CreateNamedResourceCommand, UpdateNamedResourceCommand},
};

fn test_ctx() -> ActorContext {
    ActorContext {
        auth: AuthContext {
            identity: Identity {
                user_id: "actor-1".into(),
                principal_ref: "user:actor-1".into(),
                session_id: "sess-1".into(),
                token_type: "session".into(),
                org_id: "org-1".into(),
                issuer_instance_id: None,
                support_grant: None,
            },
            capabilities: vec![Capability::OperatorAdmin],
        },
        instance: InstanceContext {
            instance_id: "test-instance".into(),
            placement_mode: "global".into(),
            region_key: None,
            feature_overrides: Default::default(),
            host: "localhost".into(),
        },
    }
}

fn self_service_ctx() -> ActorContext {
    ActorContext {
        auth: AuthContext {
            identity: Identity {
                user_id: "actor-1".into(),
                principal_ref: "user:actor-1".into(),
                session_id: "sess-1".into(),
                token_type: "session".into(),
                org_id: "org-1".into(),
                issuer_instance_id: None,
                support_grant: None,
            },
            capabilities: vec![],
        },
        instance: InstanceContext {
            instance_id: "test-instance".into(),
            placement_mode: "global".into(),
            region_key: None,
            feature_overrides: Default::default(),
            host: "localhost".into(),
        },
    }
}

fn test_services() -> (Arc<ApplicationServices>, Arc<Repositories>) {
    let repos = Arc::new(zitadel_app::mock::mock_repositories());
    let hooks = Arc::new(HookPipeline::empty());
    let app = Arc::new(ApplicationServices::new(repos.clone(), hooks, false));
    (app, repos)
}

fn test_services_with_repositories(
    repos: Repositories,
) -> (Arc<ApplicationServices>, Arc<Repositories>) {
    test_services_with_repositories_and_cloud(repos, false)
}

fn test_services_with_repositories_and_cloud(
    repos: Repositories,
    cloud_enabled: bool,
) -> (Arc<ApplicationServices>, Arc<Repositories>) {
    let repos = Arc::new(repos);
    let hooks = Arc::new(HookPipeline::empty());
    let app = Arc::new(ApplicationServices::new(
        repos.clone(),
        hooks,
        cloud_enabled,
    ));
    (app, repos)
}

#[derive(Default)]
struct MemoryGroupRepository {
    store: Mutex<HashMap<(String, String), GroupRecord>>,
}

impl MemoryGroupRepository {
    fn new() -> Self {
        Self::default()
    }
}

impl GroupRepository for MemoryGroupRepository {
    fn create(
        &self,
        instance_id: &str,
        group: &GroupRecord,
    ) -> BoxFuture<'_, anyhow::Result<GroupRecord>> {
        let key = (instance_id.to_string(), group.id.clone());
        let group = group.clone();
        Box::pin(async move {
            self.store.lock().unwrap().insert(key, group.clone());
            Ok(group)
        })
    }

    fn get(
        &self,
        instance_id: &str,
        group_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<GroupRecord>>> {
        let key = (instance_id.to_string(), group_id.to_string());
        Box::pin(async move { Ok(self.store.lock().unwrap().get(&key).cloned()) })
    }

    fn list(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        _params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<GroupRecord>>> {
        let instance_id = instance_id.to_string();
        let org_id = org_id.map(ToOwned::to_owned);
        Box::pin(async move {
            let items = self
                .store
                .lock()
                .unwrap()
                .iter()
                .filter(|((stored_instance_id, _), group)| {
                    stored_instance_id == &instance_id
                        && org_id
                            .as_ref()
                            .is_none_or(|expected_org_id| &group.org_id == expected_org_id)
                })
                .map(|(_, group)| group.clone())
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
        group: &GroupRecord,
    ) -> BoxFuture<'_, anyhow::Result<GroupRecord>> {
        let key = (instance_id.to_string(), group.id.clone());
        let group = group.clone();
        Box::pin(async move {
            self.store.lock().unwrap().insert(key, group.clone());
            Ok(group)
        })
    }

    fn delete(&self, instance_id: &str, group_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let key = (instance_id.to_string(), group_id.to_string());
        Box::pin(async move {
            self.store.lock().unwrap().remove(&key);
            Ok(())
        })
    }

    fn add_member(
        &self,
        _instance_id: &str,
        _group_id: &str,
        _user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn remove_member(
        &self,
        _instance_id: &str,
        _group_id: &str,
        _user_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

struct MemorySessionRepository {
    store: Mutex<HashMap<(String, String), SessionDetail>>,
}

impl MemorySessionRepository {
    fn new(sessions: Vec<SessionDetail>) -> Self {
        let store = sessions
            .into_iter()
            .map(|session| (("test-instance".to_string(), session.id.clone()), session))
            .collect();
        Self {
            store: Mutex::new(store),
        }
    }
}

impl SessionRepository for MemorySessionRepository {
    #[allow(clippy::too_many_arguments)]
    fn create(
        &self,
        _instance_id: &str,
        user_id: &str,
        org_id: &str,
        _auth_method: &str,
        _user_agent: &str,
        _ip_address: &str,
        _fingerprint: &str,
    ) -> BoxFuture<'_, anyhow::Result<CreatedSession>> {
        let user_id = user_id.to_string();
        let org_id = org_id.to_string();
        Box::pin(async move {
            Ok(CreatedSession {
                session_id: format!("created-{user_id}-{org_id}"),
                token: "mock-token".to_string(),
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

    fn revoke(&self, instance_id: &str, session_id: &str) -> BoxFuture<'_, anyhow::Result<bool>> {
        let key = (instance_id.to_string(), session_id.to_string());
        Box::pin(async move {
            let mut store = self.store.lock().unwrap();
            let Some(session) = store.get_mut(&key) else {
                return Ok(false);
            };
            session.revoked_at = Some("2026-04-06T00:00:00Z".to_string());
            Ok(true)
        })
    }

    fn update_metadata(
        &self,
        _instance_id: &str,
        _session_id: &str,
        _metadata_json: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn list_by_instance(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SessionDetail>>> {
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            Ok(self
                .store
                .lock()
                .unwrap()
                .iter()
                .filter(|((stored_instance_id, _), _)| stored_instance_id == &instance_id)
                .map(|(_, session)| session.clone())
                .collect())
        })
    }

    fn get(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SessionDetail>>> {
        let key = (instance_id.to_string(), session_id.to_string());
        Box::pin(async move { Ok(self.store.lock().unwrap().get(&key).cloned()) })
    }
}

#[derive(Default)]
struct MemoryNamedResourceRepository {
    store: Mutex<HashMap<(String, String, String), NamedResourceRecord>>,
}

impl MemoryNamedResourceRepository {
    fn new() -> Self {
        Self::default()
    }
}

impl AppRepository for MemoryNamedResourceRepository {
    fn create(
        &self,
        instance_id: &str,
        app: &AppRecord,
    ) -> BoxFuture<'_, anyhow::Result<AppRecord>> {
        let key = (
            instance_id.to_string(),
            "apps".to_string(),
            app.id.to_string(),
        );
        let app = app.clone();
        Box::pin(async move {
            self.store.lock().unwrap().insert(
                key,
                NamedResourceRecord {
                    id: app.id.clone(),
                    name: app.name.clone(),
                    state: app.state.clone(),
                    created_at: app.created_at.clone(),
                    updated_at: app.updated_at.clone(),
                },
            );
            Ok(app)
        })
    }

    fn get(&self, instance_id: &str, id: &str) -> BoxFuture<'_, anyhow::Result<Option<AppRecord>>> {
        let key = (instance_id.to_string(), "apps".to_string(), id.to_string());
        Box::pin(async move {
            Ok(self
                .store
                .lock()
                .unwrap()
                .get(&key)
                .cloned()
                .map(|record| AppRecord {
                    id: record.id,
                    group_id: "org-1".into(),
                    name: record.name,
                    protocol: String::new(),
                    state: record.state,
                    metadata: serde_json::Value::Object(Default::default()),
                    created_at: record.created_at,
                    updated_at: record.updated_at,
                }))
        })
    }

    fn list(
        &self,
        instance_id: &str,
        _group_id: Option<&str>,
        _params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<AppRecord>>> {
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            let mut items: Vec<_> = self
                .store
                .lock()
                .unwrap()
                .iter()
                .filter(|((stored_instance_id, stored_table, _), _)| {
                    stored_instance_id == &instance_id && stored_table == "apps"
                })
                .map(|(_, item)| AppRecord {
                    id: item.id.clone(),
                    group_id: "org-1".into(),
                    name: item.name.clone(),
                    protocol: String::new(),
                    state: item.state.clone(),
                    metadata: serde_json::Value::Object(Default::default()),
                    created_at: item.created_at.clone(),
                    updated_at: item.updated_at.clone(),
                })
                .collect();
            items.sort_by(|left, right| left.id.cmp(&right.id));
            Ok(ListResult {
                items,
                next_cursor: None,
                total_count: None,
            })
        })
    }

    fn update_name(
        &self,
        instance_id: &str,
        id: &str,
        name: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let key = (instance_id.to_string(), "apps".to_string(), id.to_string());
        let next_name = name.to_string();
        Box::pin(async move {
            let mut guard = self.store.lock().unwrap();
            if let Some(record) = guard.get_mut(&key) {
                record.name = next_name;
                record.updated_at = "updated".into();
                Ok(true)
            } else {
                Ok(false)
            }
        })
    }

    fn delete(&self, instance_id: &str, id: &str) -> BoxFuture<'_, anyhow::Result<bool>> {
        let key = (instance_id.to_string(), "apps".to_string(), id.to_string());
        Box::pin(async move { Ok(self.store.lock().unwrap().remove(&key).is_some()) })
    }
}

impl ProjectRepository for MemoryNamedResourceRepository {
    fn create(
        &self,
        instance_id: &str,
        project: &NamedResourceRecord,
        _org_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<NamedResourceRecord>> {
        let key = (
            instance_id.to_string(),
            "projects".to_string(),
            project.id.to_string(),
        );
        let record = project.clone();
        Box::pin(async move {
            self.store.lock().unwrap().insert(key, record.clone());
            Ok(record)
        })
    }

    fn get(
        &self,
        instance_id: &str,
        id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<NamedResourceRecord>>> {
        let key = (
            instance_id.to_string(),
            "projects".to_string(),
            id.to_string(),
        );
        Box::pin(async move { Ok(self.store.lock().unwrap().get(&key).cloned()) })
    }

    fn list(
        &self,
        instance_id: &str,
        _params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<NamedResourceRecord>>> {
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            let mut items: Vec<_> = self
                .store
                .lock()
                .unwrap()
                .iter()
                .filter(|((stored_instance_id, stored_table, _), _)| {
                    stored_instance_id == &instance_id && stored_table == "projects"
                })
                .map(|(_, item)| item.clone())
                .collect();
            items.sort_by(|left, right| left.id.cmp(&right.id));
            Ok(ListResult {
                items,
                next_cursor: None,
                total_count: None,
            })
        })
    }

    fn update_name(
        &self,
        instance_id: &str,
        id: &str,
        name: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let key = (
            instance_id.to_string(),
            "projects".to_string(),
            id.to_string(),
        );
        let next_name = name.to_string();
        Box::pin(async move {
            let mut guard = self.store.lock().unwrap();
            if let Some(record) = guard.get_mut(&key) {
                record.name = next_name;
                record.updated_at = "updated".into();
                Ok(true)
            } else {
                Ok(false)
            }
        })
    }

    fn delete(&self, instance_id: &str, id: &str) -> BoxFuture<'_, anyhow::Result<bool>> {
        let key = (
            instance_id.to_string(),
            "projects".to_string(),
            id.to_string(),
        );
        Box::pin(async move { Ok(self.store.lock().unwrap().remove(&key).is_some()) })
    }
}

impl ConsoleQueryRepository for MemoryNamedResourceRepository {
    fn load_console_bootstrap(
        &self,
        _instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<ConsoleBootstrapData>> {
        Box::pin(async {
            Ok(ConsoleBootstrapData {
                counts: vec![],
                orgs: Vec::<OrgSummary>::new(),
                instance: InstanceInfo {
                    instance_id: "test-instance".into(),
                    kind: "root".into(),
                    feature_overrides_json: "{}".into(),
                    parent_instance_id: None,
                },
            })
        })
    }

    fn load_entity_counts(
        &self,
        _instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<(String, i64)>>> {
        Box::pin(async { Ok(vec![]) })
    }
}

impl TelemetryRepository for MemoryNamedResourceRepository {
    fn list_fingerprints(
        &self,
        _instance_id: &str,
        _cursor: &str,
        _limit: i64,
    ) -> BoxFuture<'_, anyhow::Result<Vec<FingerprintRecord>>> {
        Box::pin(async { Ok(vec![]) })
    }

    fn upsert_fingerprint(
        &self,
        _instance_id: &str,
        _id: &str,
        _type_: &str,
        _raw_data: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

impl JobRepository for MemoryNamedResourceRepository {
    fn list_jobs(&self, _instance_id: &str) -> BoxFuture<'_, anyhow::Result<Vec<JobRecord>>> {
        Box::pin(async { Ok(vec![]) })
    }
}

impl SavedQueryRepository for MemoryNamedResourceRepository {
    fn list_saved_queries(
        &self,
        _instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SavedQueryRecord>>> {
        Box::pin(async { Ok(vec![]) })
    }

    fn create_saved_query(
        &self,
        _instance_id: &str,
        _id: &str,
        _name: &str,
        _description: &str,
        _sql: &str,
    ) -> BoxFuture<'_, anyhow::Result<SavedQueryRecord>> {
        Box::pin(async { anyhow::bail!("not implemented in test repository") })
    }

    fn delete_saved_query(
        &self,
        _instance_id: &str,
        _id: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        Box::pin(async { Ok(false) })
    }
}

#[derive(Default)]
struct MemoryInstanceRepository {
    store: Mutex<HashMap<String, InstanceRecord>>,
    domains: Mutex<HashMap<(String, Option<String>, String), DomainRecord>>,
}

impl MemoryInstanceRepository {
    fn new() -> Self {
        Self::default()
    }

    fn domain_key(
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
    ) -> (String, Option<String>, String) {
        (
            instance_id.to_string(),
            org_id.map(ToOwned::to_owned),
            domain.to_string(),
        )
    }
}

impl InstanceRepository for MemoryInstanceRepository {
    fn create(
        &self,
        _root_instance_id: &str,
        instance: &InstanceRecord,
    ) -> BoxFuture<'_, anyhow::Result<InstanceRecord>> {
        let instance = instance.clone();
        let key = instance.instance_id.clone();
        Box::pin(async move {
            self.store.lock().unwrap().insert(key, instance.clone());
            Ok(instance)
        })
    }

    fn get(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Option<InstanceRecord>>> {
        let instance_id = instance_id.to_string();
        Box::pin(async move { Ok(self.store.lock().unwrap().get(&instance_id).cloned()) })
    }

    fn list(
        &self,
        _root_instance_id: &str,
        _params: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<InstanceRecord>>> {
        Box::pin(async move {
            let mut items: Vec<_> = self.store.lock().unwrap().values().cloned().collect();
            items.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));
            Ok(ListResult {
                items,
                next_cursor: None,
                total_count: None,
            })
        })
    }

    fn update(&self, instance: &InstanceRecord) -> BoxFuture<'_, anyhow::Result<InstanceRecord>> {
        let instance = instance.clone();
        let key = instance.instance_id.clone();
        Box::pin(async move {
            self.store.lock().unwrap().insert(key, instance.clone());
            Ok(instance)
        })
    }

    fn deprovision(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            if let Some(instance) = self.store.lock().unwrap().get_mut(&instance_id) {
                instance.state = "deprovisioning".into();
                instance.updated_at = "deprovisioned".into();
            }
            Ok(())
        })
    }

    fn resolve_domain(
        &self,
        domain: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<RouteResolution>>> {
        let domain = domain.to_string();
        Box::pin(async move {
            let guard = self.domains.lock().unwrap();
            Ok(guard
                .values()
                .find(|record| record.domain == domain && record.state == "active")
                .map(|record| RouteResolution {
                    instance_id: record.instance_id.clone(),
                    resolved_org_id: record.org_id.clone(),
                    placement_mode: "global".into(),
                    region_key: None,
                }))
        })
    }

    fn list_domains(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Vec<DomainRecord>>> {
        let instance_id = instance_id.to_string();
        Box::pin(async move {
            let mut items: Vec<_> = self
                .domains
                .lock()
                .unwrap()
                .values()
                .filter(|record| record.instance_id == instance_id)
                .cloned()
                .collect();
            items.sort_by(|left, right| left.domain.cmp(&right.domain));
            Ok(items)
        })
    }

    fn set_domain(
        &self,
        instance_id: &str,
        domain: &DomainRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let key = Self::domain_key(instance_id, domain.org_id.as_deref(), &domain.domain);
        let domain = domain.clone();
        Box::pin(async move {
            self.domains.lock().unwrap().insert(key, domain);
            Ok(())
        })
    }

    fn remove_domain(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
    ) -> BoxFuture<'_, anyhow::Result<DomainRemoveResult>> {
        let key = Self::domain_key(instance_id, org_id, domain);
        Box::pin(async move {
            let mut guard = self.domains.lock().unwrap();
            let Some(existing) = guard.get(&key) else {
                return Ok(DomainRemoveResult::NotFound);
            };
            if existing.is_primary {
                return Ok(DomainRemoveResult::PrimaryDomain);
            }
            guard.remove(&key);
            Ok(DomainRemoveResult::Deleted)
        })
    }

    fn find_domain(&self, domain: &str) -> BoxFuture<'_, anyhow::Result<Option<DomainRecord>>> {
        let domain = domain.to_string();
        Box::pin(async move {
            Ok(self
                .domains
                .lock()
                .unwrap()
                .values()
                .find(|record| record.domain == domain)
                .cloned())
        })
    }

    fn get_domain(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<DomainRecord>>> {
        let key = Self::domain_key(instance_id, org_id, domain);
        Box::pin(async move { Ok(self.domains.lock().unwrap().get(&key).cloned()) })
    }

    fn update_domain_state(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
        new_state: &str,
        verified: bool,
        provisioning_error: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let key = Self::domain_key(instance_id, org_id, domain);
        let new_state = new_state.to_string();
        let provisioning_error = provisioning_error.map(ToOwned::to_owned);
        Box::pin(async move {
            if let Some(record) = self.domains.lock().unwrap().get_mut(&key) {
                record.state = new_state;
                record.verified = verified;
                record.provisioning_error = provisioning_error.unwrap_or_default();
            }
            Ok(())
        })
    }

    fn update_domain_certificate_state(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
        cert_state: &str,
        cert_id: &str,
        cert_map_entry: Option<&str>,
        dns_authorization_id: Option<&str>,
        dns_record_name: Option<&str>,
        dns_record_type: Option<&str>,
        dns_record_value: Option<&str>,
        provisioning_error: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let key = Self::domain_key(instance_id, org_id, domain);
        let cert_state = cert_state.to_string();
        let cert_id = cert_id.to_string();
        let cert_map_entry = cert_map_entry.map(ToOwned::to_owned);
        let dns_authorization_id = dns_authorization_id.map(ToOwned::to_owned);
        let dns_record_name = dns_record_name.map(ToOwned::to_owned);
        let dns_record_type = dns_record_type.map(ToOwned::to_owned);
        let dns_record_value = dns_record_value.map(ToOwned::to_owned);
        let provisioning_error = provisioning_error.map(ToOwned::to_owned);
        Box::pin(async move {
            if let Some(record) = self.domains.lock().unwrap().get_mut(&key) {
                record.certificate_state = cert_state;
                record.certificate_id = cert_id;
                if let Some(value) = cert_map_entry {
                    record.certificate_map_entry = value;
                }
                if let Some(value) = dns_authorization_id {
                    record.dns_authorization_id = value;
                }
                if let Some(value) = dns_record_name {
                    record.certificate_dns_record_name = value;
                }
                if let Some(value) = dns_record_type {
                    record.certificate_dns_record_type = value;
                }
                if let Some(value) = dns_record_value {
                    record.certificate_dns_record_value = value;
                }
                record.provisioning_error = provisioning_error.unwrap_or_default();
            }
            Ok(())
        })
    }

    fn update_domain_origin_trust_state(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
        domain: &str,
        state: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let key = Self::domain_key(instance_id, org_id, domain);
        let state = state.to_string();
        Box::pin(async move {
            if let Some(record) = self.domains.lock().unwrap().get_mut(&key) {
                record.origin_trust_state = state;
            }
            Ok(())
        })
    }

    fn list_domains_for_instance(
        &self,
        instance_id: &str,
        org_id: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<Vec<DomainRecord>>> {
        let instance_id = instance_id.to_string();
        let org_id = org_id.map(ToOwned::to_owned);
        Box::pin(async move {
            let mut items: Vec<_> = self
                .domains
                .lock()
                .unwrap()
                .values()
                .filter(|record| {
                    record.instance_id == instance_id
                        && match &org_id {
                            Some(expected) => record.org_id.as_ref() == Some(expected),
                            None => record.org_id.is_none(),
                        }
                })
                .cloned()
                .collect();
            items.sort_by(|left, right| left.domain.cmp(&right.domain));
            Ok(items)
        })
    }
}

#[derive(Default)]
struct MemoryEffectRepository {
    store: Mutex<Vec<(String, Effect)>>,
}

impl MemoryEffectRepository {
    fn new() -> Self {
        Self::default()
    }

    fn effects_for_instance(&self, instance_id: &str) -> Vec<Effect> {
        self.store
            .lock()
            .unwrap()
            .iter()
            .filter(|(stored_instance_id, _)| stored_instance_id == instance_id)
            .map(|(_, effect)| effect.clone())
            .collect()
    }
}

impl EffectRepository for MemoryEffectRepository {
    fn enqueue_batch(
        &self,
        instance_id: &str,
        effects: &[Effect],
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let instance_id = instance_id.to_string();
        let effects = effects.to_vec();
        Box::pin(async move {
            let mut guard = self.store.lock().unwrap();
            for effect in effects {
                guard.push((instance_id.clone(), effect));
            }
            Ok(())
        })
    }

    fn claim_due(
        &self,
        _instance_id: &str,
        _worker_id: &str,
        _lease_ttl_secs: u64,
        _limit: u32,
    ) -> BoxFuture<'_, anyhow::Result<Vec<Effect>>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn mark_completed(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn record_failure(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn mark_dead(&self, _: &str, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn cleanup(&self, _: &str, _: &str, _: u32) -> BoxFuture<'_, anyhow::Result<u64>> {
        Box::pin(async { Ok(0) })
    }
}

// ─── CreateUser tests ─��──────────────────────────────────

#[tokio::test]
async fn create_user_success() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    // Seed a default org so first_org_id returns something.
    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let cmd = CreateUserCommand {
        identifier: "alice@example.com".into(),
        display_name: "Alice".into(),
        user_type: "human".into(),
        schema_id: "default".into(),
        org_id: Some("org-1".into()),
        metadata: serde_json::json!({}),
    };

    let user = app.create_user.execute(&ctx, cmd).await.unwrap();
    assert_eq!(user.identifier, "alice@example.com");
    assert_eq!(user.display_name, "Alice");
    assert_eq!(user.state, "active");
    assert_eq!(user.org_id, "org-1");
}

#[tokio::test]
async fn create_user_empty_identifier_rejected() {
    let (app, _) = test_services();
    let ctx = test_ctx();

    let cmd = CreateUserCommand {
        identifier: "".into(),
        display_name: "Alice".into(),
        user_type: "human".into(),
        schema_id: "default".into(),
        org_id: Some("org-1".into()),
        metadata: serde_json::json!({}),
    };

    let err = app.create_user.execute(&ctx, cmd).await.unwrap_err();
    assert!(matches!(err, AppError::Validation { .. }));
    assert_eq!(err.status_code(), 400);
}

#[tokio::test]
async fn create_user_duplicate_rejected() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let cmd = CreateUserCommand {
        identifier: "alice@example.com".into(),
        display_name: "Alice".into(),
        user_type: "human".into(),
        schema_id: "default".into(),
        org_id: Some("org-1".into()),
        metadata: serde_json::json!({}),
    };

    app.create_user.execute(&ctx, cmd).await.unwrap();

    // Attempt duplicate
    let cmd2 = CreateUserCommand {
        identifier: "alice@example.com".into(),
        display_name: "Alice 2".into(),
        user_type: "human".into(),
        schema_id: "default".into(),
        org_id: Some("org-1".into()),
        metadata: serde_json::json!({}),
    };

    let err = app.create_user.execute(&ctx, cmd2).await.unwrap_err();
    assert!(matches!(err, AppError::AlreadyExists { .. }));
    assert_eq!(err.status_code(), 409);
}

// ─── GetUser tests ───────────────────────────────────────

#[tokio::test]
async fn get_user_not_found() {
    let (app, _) = test_services();
    let ctx = test_ctx();

    let err = app.get_user.execute(&ctx, "nonexistent").await.unwrap_err();
    assert!(matches!(err, AppError::NotFound { .. }));
    assert_eq!(err.status_code(), 404);
}

#[tokio::test]
async fn get_user_after_create() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let created = app
        .create_user
        .execute(
            &ctx,
            CreateUserCommand {
                identifier: "bob".into(),
                display_name: "Bob".into(),
                user_type: "human".into(),
                schema_id: "default".into(),
                org_id: Some("org-1".into()),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    let fetched = app.get_user.execute(&ctx, &created.id).await.unwrap();
    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.identifier, "bob");
}

// ─── DeactivateUser tests ────────────────────────────────

#[tokio::test]
async fn deactivate_user_success() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let user = app
        .create_user
        .execute(
            &ctx,
            CreateUserCommand {
                identifier: "charlie".into(),
                display_name: "Charlie".into(),
                user_type: "human".into(),
                schema_id: "default".into(),
                org_id: Some("org-1".into()),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    app.deactivate_user.execute(&ctx, &user.id).await.unwrap();

    let deactivated = app.get_user.execute(&ctx, &user.id).await.unwrap();
    assert_eq!(deactivated.state, "deactivated");
}

#[tokio::test]
async fn deactivate_nonexistent_user_fails() {
    let (app, _) = test_services();
    let ctx = test_ctx();

    let err = app
        .deactivate_user
        .execute(&ctx, "no-such-user")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::NotFound { .. }));
}

// ─── Hook pipeline tests ─────────────────────────────────
// The UseCaseRunner requires `impl UseCase`. Use cases in this crate have custom
// execute methods but don't implement the trait directly. Test the interceptor
// pipeline by creating a trivial inline use case.

use std::future::Future;
use std::pin::Pin;
use zitadel_app::hook::{
    DenyReason, HookContext, HookPhase, InterceptResult, PolicyInterceptor, StepUpKind,
};
use zitadel_app::usecase::UseCase;

struct NoopUseCase;

impl UseCase for NoopUseCase {
    type Command = ();
    type Result = String;
    type Error = AppError;

    fn execute(
        &self,
        _ctx: &ActorContext,
        _cmd: (),
    ) -> impl Future<Output = Result<String, AppError>> + Send {
        async { Ok("ok".to_string()) }
    }
}

#[tokio::test]
async fn hook_pipeline_empty_runs_use_case() {
    let runner = zitadel_app::UseCaseRunner::new(vec![], vec![], vec![]);
    let ctx = test_ctx();
    let result = runner
        .run_usecase(&NoopUseCase, &ctx, (), "test.noop")
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "ok");
}

#[tokio::test]
async fn hook_pipeline_deny_interceptor_blocks() {
    struct AlwaysDeny;
    impl PolicyInterceptor for AlwaysDeny {
        fn intercept(
            &self,
            _phase: HookPhase,
            _ctx: &HookContext,
        ) -> Pin<Box<dyn Future<Output = InterceptResult> + Send + '_>> {
            Box::pin(async {
                InterceptResult::Deny(DenyReason {
                    code: "test.denied".into(),
                    message: "blocked by test".into(),
                })
            })
        }
    }

    let runner = zitadel_app::UseCaseRunner::new(
        vec![Arc::new(AlwaysDeny) as Arc<dyn PolicyInterceptor>],
        vec![],
        vec![],
    );
    let ctx = test_ctx();
    let err = runner
        .run_usecase(&NoopUseCase, &ctx, (), "test.noop")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::PolicyDenied { .. }));
    assert_eq!(err.status_code(), 403);
}

#[tokio::test]
async fn hook_pipeline_step_up_interceptor() {
    struct RequireMfa;
    impl PolicyInterceptor for RequireMfa {
        fn intercept(
            &self,
            _phase: HookPhase,
            _ctx: &HookContext,
        ) -> Pin<Box<dyn Future<Output = InterceptResult> + Send + '_>> {
            Box::pin(async { InterceptResult::RequireStepUp(StepUpKind::Otp) })
        }
    }

    let runner = zitadel_app::UseCaseRunner::new(
        vec![],
        vec![Arc::new(RequireMfa) as Arc<dyn PolicyInterceptor>],
        vec![],
    );
    let ctx = test_ctx();
    let err = runner
        .run_usecase(&NoopUseCase, &ctx, (), "test.noop")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::StepUpRequired { .. }));
}

#[tokio::test]
async fn hook_pipeline_interceptor_ordering() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct OrderedInterceptor {
        expected_order: usize,
    }
    impl PolicyInterceptor for OrderedInterceptor {
        fn intercept(
            &self,
            _phase: HookPhase,
            _ctx: &HookContext,
        ) -> Pin<Box<dyn Future<Output = InterceptResult> + Send + '_>> {
            let expected = self.expected_order;
            Box::pin(async move {
                let actual = COUNTER.fetch_add(1, Ordering::SeqCst);
                assert_eq!(actual, expected, "interceptor ran out of order");
                InterceptResult::Continue
            })
        }
    }

    COUNTER.store(0, Ordering::SeqCst);
    let runner = zitadel_app::UseCaseRunner::new(
        vec![
            Arc::new(OrderedInterceptor { expected_order: 0 }) as Arc<dyn PolicyInterceptor>,
            Arc::new(OrderedInterceptor { expected_order: 1 }),
        ],
        vec![Arc::new(OrderedInterceptor { expected_order: 2 })],
        vec![],
    );
    let ctx = test_ctx();
    let result = runner
        .run_usecase(&NoopUseCase, &ctx, (), "test.order")
        .await;
    assert!(result.is_ok());
    assert_eq!(COUNTER.load(Ordering::SeqCst), 3);
}

// ─── DeleteOrg tests ────────────────────────────────────

#[tokio::test]
async fn delete_org_does_not_delete_users() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    // Create an org and a user in it.
    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let user = app
        .create_user
        .execute(
            &ctx,
            CreateUserCommand {
                identifier: "admin".into(),
                display_name: "Admin".into(),
                user_type: "human".into(),
                schema_id: "default".into(),
                org_id: Some("org-1".into()),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap();

    // Delete the org.
    app.delete_org.execute(&ctx, "org-1").await.unwrap();

    // Org should be gone.
    let err = app.get_org.execute(&ctx, "org-1").await.unwrap_err();
    assert!(matches!(err, AppError::NotFound { .. }));

    // User must still exist (the mock does not cascade — the DB migration
    // changes the FK from CASCADE to SET NULL so the real DB behaves the same).
    let fetched = app.get_user.execute(&ctx, &user.id).await.unwrap();
    assert_eq!(fetched.identifier, "admin");
}

#[tokio::test]
async fn delete_org_success() {
    let (app, repos) = test_services();
    let ctx = test_ctx();

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-a".into(),
                name: "Org A".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-b".into(),
                name: "Org B".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    // Delete org-a — should succeed.
    app.delete_org.execute(&ctx, "org-a").await.unwrap();

    // org-a gone, org-b still around.
    assert!(matches!(
        app.get_org.execute(&ctx, "org-a").await,
        Err(AppError::NotFound { .. })
    ));
    assert!(app.get_org.execute(&ctx, "org-b").await.is_ok());
}

#[tokio::test]
async fn delete_org_not_found() {
    let (app, _) = test_services();
    let ctx = test_ctx();

    let err = app
        .delete_org
        .execute(&ctx, "nonexistent")
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::NotFound { .. }));
}

#[tokio::test]
async fn delete_org_requires_operator_admin() {
    let (app, repos) = test_services();

    // Create a context without operator admin capability.
    let ctx = ActorContext {
        auth: AuthContext {
            identity: Identity {
                user_id: "actor-1".into(),
                principal_ref: "user:actor-1".into(),
                session_id: "sess-1".into(),
                token_type: "session".into(),
                org_id: "org-1".into(),
                issuer_instance_id: None,
                support_grant: None,
            },
            capabilities: vec![], // no operator admin
        },
        instance: InstanceContext {
            instance_id: "test-instance".into(),
            placement_mode: "global".into(),
            region_key: None,
            feature_overrides: Default::default(),
            host: "localhost".into(),
        },
    };

    repos
        .orgs
        .create(
            "test-instance",
            &zitadel_app::repo::OrgRecord {
                id: "org-1".into(),
                name: "Default".into(),
                state: "active".into(),
                metadata: serde_json::Value::Null,
                created_at: String::new(),
                updated_at: String::new(),
            },
        )
        .await
        .unwrap();

    let err = app.delete_org.execute(&ctx, "org-1").await.unwrap_err();
    assert!(matches!(err, AppError::OperatorAdminRequired));
}

// ─── Group subsystem tests ───────────────────────────────

#[tokio::test]
async fn groups_validate_and_persist_crud_behavior() {
    let mut repos = zitadel_app::mock::mock_repositories();
    repos.groups = Arc::new(MemoryGroupRepository::new());
    let (app, _) = test_services_with_repositories(repos);
    let ctx = test_ctx();

    let validation_err = app
        .create_group
        .execute(
            &ctx,
            CreateGroupCommand {
                name: String::new(),
                org_id: "org-1".into(),
                metadata: serde_json::json!({}),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(validation_err, AppError::Validation { .. }));

    let created = app
        .create_group
        .execute(
            &ctx,
            CreateGroupCommand {
                name: "Platform".into(),
                org_id: "org-1".into(),
                metadata: serde_json::json!({ "team": "identity" }),
            },
        )
        .await
        .unwrap();
    assert_eq!(created.name, "Platform");
    assert_eq!(created.org_id, "org-1");

    let fetched = app.get_group.execute(&ctx, &created.id).await.unwrap();
    assert_eq!(fetched.id, created.id);

    let listed = app
        .list_groups
        .execute(
            &ctx,
            Some("org-1"),
            &ListParams {
                limit: Some(50),
                cursor: None,
                search: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].id, created.id);

    let updated = app
        .update_group
        .execute(
            &ctx,
            UpdateGroupCommand {
                group_id: created.id.clone(),
                name: Some("Platform Security".into()),
                metadata: Some(serde_json::json!({ "team": "security" })),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "Platform Security");
    assert_eq!(updated.metadata["team"], "security");

    app.delete_group.execute(&ctx, &created.id).await.unwrap();

    let missing = app.get_group.execute(&ctx, &created.id).await.unwrap_err();
    assert!(matches!(missing, AppError::NotFound { .. }));

    let update_missing = app
        .update_group
        .execute(
            &ctx,
            UpdateGroupCommand {
                group_id: created.id.clone(),
                name: Some("No longer here".into()),
                metadata: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(update_missing, AppError::NotFound { .. }));

    let delete_missing = app
        .delete_group
        .execute(&ctx, &created.id)
        .await
        .unwrap_err();
    assert!(matches!(delete_missing, AppError::NotFound { .. }));
}

// ─── Named resource subsystem tests ──────────────────────

#[tokio::test]
async fn named_resources_support_projects_and_apps() {
    let mut repos = zitadel_app::mock::mock_repositories();
    let shared = Arc::new(MemoryNamedResourceRepository::new());
    repos.apps = shared.clone();
    repos.projects = shared.clone();
    repos.console_queries = shared.clone();
    repos.telemetry = shared.clone();
    repos.jobs = shared.clone();
    repos.saved_queries = shared.clone();
    let (app, _) = test_services_with_repositories(repos);
    let ctx = test_ctx();

    let project = app
        .create_named_resource
        .execute(
            &ctx,
            CreateNamedResourceCommand {
                kind: "projects".into(),
                name: "Customer Portal".into(),
                org_id: "org-1".into(),
            },
        )
        .await
        .unwrap();
    let application = app
        .create_named_resource
        .execute(
            &ctx,
            CreateNamedResourceCommand {
                kind: "apps".into(),
                name: "Console Frontend".into(),
                org_id: "org-1".into(),
            },
        )
        .await
        .unwrap();

    let listed_projects = app
        .list_named_resources
        .execute(&ctx, "projects", "", 50)
        .await
        .unwrap();
    assert_eq!(listed_projects.len(), 1);
    assert_eq!(listed_projects[0].id, project.id);

    let listed_apps = app
        .list_named_resources
        .execute(&ctx, "apps", "", 50)
        .await
        .unwrap();
    assert_eq!(listed_apps.len(), 1);
    assert_eq!(listed_apps[0].id, application.id);

    let loaded_project = app
        .get_named_resource
        .execute(&ctx, "projects", &project.id)
        .await
        .unwrap();
    assert_eq!(loaded_project.name, "Customer Portal");

    let updated_project = app
        .update_named_resource
        .execute(
            &ctx,
            UpdateNamedResourceCommand {
                kind: "projects".into(),
                id: project.id.clone(),
                name: "Customer Portal Renamed".into(),
            },
        )
        .await
        .unwrap();
    assert!(updated_project);

    let reloaded_project = app
        .get_named_resource
        .execute(&ctx, "projects", &project.id)
        .await
        .unwrap();
    assert_eq!(reloaded_project.name, "Customer Portal Renamed");

    let deleted_application = app
        .delete_named_resource
        .execute(&ctx, "apps", &application.id)
        .await
        .unwrap();
    assert!(deleted_application);

    let missing_application = app
        .get_named_resource
        .execute(&ctx, "apps", &application.id)
        .await
        .unwrap_err();
    assert!(matches!(missing_application, AppError::NotFound { .. }));

    let update_missing = app
        .update_named_resource
        .execute(
            &ctx,
            UpdateNamedResourceCommand {
                kind: "apps".into(),
                id: "missing".into(),
                name: "Missing".into(),
            },
        )
        .await
        .unwrap();
    assert!(!update_missing);

    let delete_missing = app
        .delete_named_resource
        .execute(&ctx, "projects", "missing")
        .await
        .unwrap();
    assert!(!delete_missing);
}

// ─── Instance subsystem tests ────────────────────────────

#[tokio::test]
async fn instances_support_create_update_and_deprovision_state_transitions() {
    let mut repos = zitadel_app::mock::mock_repositories();
    repos.instances = Arc::new(MemoryInstanceRepository::new());
    let (app, _) = test_services_with_repositories(repos);
    let ctx = test_ctx();

    let created = app
        .create_instance
        .execute(
            &ctx,
            CreateInstanceCommand {
                kind: "managed".into(),
                placement_mode: "global".into(),
                region_key: None,
                owner_org_id: "org-1".into(),
                feature_overrides: serde_json::json!({}),
                primary_domain: Some("tenant.example.com".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(created.state, "active");
    assert_eq!(
        created.primary_domain.as_deref(),
        Some("tenant.example.com")
    );

    let fetched = app
        .get_instance
        .execute(&ctx, &created.instance_id)
        .await
        .unwrap();
    assert_eq!(fetched.instance_id, created.instance_id);

    let listed = app
        .list_instances
        .execute(
            &ctx,
            &ListParams {
                limit: Some(50),
                cursor: None,
                search: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].instance_id, created.instance_id);

    let updated = app
        .update_instance
        .execute(
            &ctx,
            UpdateInstanceCommand {
                instance_id: created.instance_id.clone(),
                placement_mode: Some("regional".into()),
                region_key: Some("europe-west1".into()),
                feature_overrides: Some(serde_json::json!({ "custom_domains": true })),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.placement_mode, "regional");
    assert_eq!(updated.region_key.as_deref(), Some("europe-west1"));
    assert_eq!(updated.feature_overrides["custom_domains"], true);

    app.deprovision_instance
        .execute(&ctx, &created.instance_id)
        .await
        .unwrap();

    let deprovisioned = app
        .get_instance
        .execute(&ctx, &created.instance_id)
        .await
        .unwrap();
    assert_eq!(deprovisioned.state, "deprovisioning");

    app.deprovision_instance
        .execute(&ctx, &created.instance_id)
        .await
        .unwrap();

    let update_missing = app
        .update_instance
        .execute(
            &ctx,
            UpdateInstanceCommand {
                instance_id: "missing-instance".into(),
                placement_mode: Some("regional".into()),
                region_key: None,
                feature_overrides: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(update_missing, AppError::NotFound { .. }));
}

#[tokio::test]
async fn self_session_use_cases_bypass_org_viewer_checks_but_scope_to_the_actor() {
    let mut repos = zitadel_app::mock::mock_repositories();
    repos.fga = Arc::new(zitadel_app::mock::MockFgaRepository { allow_all: false });
    repos.sessions = Arc::new(MemorySessionRepository::new(vec![
        SessionDetail {
            id: "sess-1".into(),
            user_id: "actor-1".into(),
            org_id: "org-1".into(),
            user_agent: "Browser A".into(),
            ip_address: "127.0.0.1".into(),
            created_at: "2026-04-06T00:00:00Z".into(),
            expires_at: None,
            revoked_at: None,
        },
        SessionDetail {
            id: "sess-2".into(),
            user_id: "other-user".into(),
            org_id: "org-2".into(),
            user_agent: "Browser B".into(),
            ip_address: "127.0.0.2".into(),
            created_at: "2026-04-06T01:00:00Z".into(),
            expires_at: None,
            revoked_at: None,
        },
    ]));
    let (app, _) = test_services_with_repositories(repos);
    let ctx = self_service_ctx();

    let denied = app.list_sessions.execute(&ctx).await.unwrap_err();
    assert!(matches!(denied, AppError::PermissionDenied { .. }));

    let rows = app.list_sessions.execute_self(&ctx).await.unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "sess-1");

    let owned = app.get_session.execute_self(&ctx, "sess-1").await.unwrap();
    assert_eq!(owned.user_id, "actor-1");

    let denied = app
        .get_session
        .execute_self(&ctx, "sess-2")
        .await
        .unwrap_err();
    assert!(matches!(denied, AppError::PermissionDenied { .. }));
}

#[tokio::test]
async fn revoke_own_session_does_not_require_operator_admin() {
    let mut repos = zitadel_app::mock::mock_repositories();
    repos.fga = Arc::new(zitadel_app::mock::MockFgaRepository { allow_all: false });
    repos.sessions = Arc::new(MemorySessionRepository::new(vec![SessionDetail {
        id: "sess-1".into(),
        user_id: "actor-1".into(),
        org_id: "org-1".into(),
        user_agent: "Browser A".into(),
        ip_address: "127.0.0.1".into(),
        created_at: "2026-04-06T00:00:00Z".into(),
        expires_at: None,
        revoked_at: None,
    }]));
    let (app, _) = test_services_with_repositories(repos);
    let ctx = self_service_ctx();

    let denied = app
        .revoke_session
        .execute(&ctx, "sess-1")
        .await
        .unwrap_err();
    assert!(matches!(denied, AppError::PermissionDenied { .. }));

    app.revoke_session
        .execute_self(&ctx, "sess-1")
        .await
        .unwrap();

    let session = app.get_session.execute_self(&ctx, "sess-1").await.unwrap();
    assert_eq!(session.revoked_at.as_deref(), Some("2026-04-06T00:00:00Z"));
}

#[tokio::test]
async fn custom_domains_are_scoped_between_instance_and_org() {
    let mut repos = zitadel_app::mock::mock_repositories();
    repos.instances = Arc::new(MemoryInstanceRepository::new());
    let (app, _) = test_services_with_repositories(repos);
    let ctx = test_ctx();

    let instance_domain = app
        .add_custom_domain
        .execute(
            &ctx,
            "test-instance",
            AddCustomDomainCommand {
                domain: "Portal.Example.COM.".into(),
                purpose: "served".into(),
                org_id: None,
            },
        )
        .await
        .unwrap();
    let org_domain = app
        .add_custom_domain
        .execute(
            &ctx,
            "test-instance",
            AddCustomDomainCommand {
                domain: "org.example.com".into(),
                purpose: "allowed".into(),
                org_id: Some("org-2".into()),
            },
        )
        .await
        .unwrap();

    assert_eq!(instance_domain.domain, "portal.example.com");
    assert_eq!(instance_domain.org_id, None);
    assert_eq!(org_domain.org_id.as_deref(), Some("org-2"));

    let instance_items = app
        .list_custom_domains
        .execute(&ctx, "test-instance", None)
        .await
        .unwrap();
    assert_eq!(instance_items.len(), 1);
    assert_eq!(instance_items[0].domain, "portal.example.com");

    let org_items = app
        .list_custom_domains
        .execute(&ctx, "test-instance", Some("org-2"))
        .await
        .unwrap();
    assert_eq!(org_items.len(), 1);
    assert_eq!(org_items[0].domain, "org.example.com");
    assert_eq!(org_items[0].purpose, "allowed");

    let missing_in_org = app
        .get_custom_domain
        .execute(&ctx, "test-instance", Some("org-2"), "portal.example.com")
        .await
        .unwrap();
    assert!(missing_in_org.is_none());

    let missing_in_instance = app
        .get_custom_domain
        .execute(&ctx, "test-instance", None, "org.example.com")
        .await
        .unwrap();
    assert!(missing_in_instance.is_none());
}

#[tokio::test]
async fn custom_domains_reject_duplicates_across_scopes() {
    let mut repos = zitadel_app::mock::mock_repositories();
    repos.instances = Arc::new(MemoryInstanceRepository::new());
    let (app, _) = test_services_with_repositories(repos);
    let ctx = test_ctx();

    app.add_custom_domain
        .execute(
            &ctx,
            "test-instance",
            AddCustomDomainCommand {
                domain: "duplicate.example.com".into(),
                purpose: "served".into(),
                org_id: None,
            },
        )
        .await
        .unwrap();

    let err = app
        .add_custom_domain
        .execute(
            &ctx,
            "test-instance",
            AddCustomDomainCommand {
                domain: "DUPLICATE.EXAMPLE.COM".into(),
                purpose: "served".into(),
                org_id: Some("org-2".into()),
            },
        )
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::AlreadyExists { .. }));
}

#[tokio::test]
async fn removing_cloud_managed_domain_marks_deprovisioning_and_enqueues_cleanup_effect() {
    let mut repos = zitadel_app::mock::mock_repositories();
    repos.instances = Arc::new(MemoryInstanceRepository::new());
    let effects = Arc::new(MemoryEffectRepository::new());
    repos.effects = effects.clone();
    let (app, _) = test_services_with_repositories_and_cloud(repos, true);
    let ctx = test_ctx();

    app.add_custom_domain
        .execute(
            &ctx,
            "test-instance",
            AddCustomDomainCommand {
                domain: "cloud.example.com".into(),
                purpose: "served".into(),
                org_id: None,
            },
        )
        .await
        .unwrap();

    app.repos
        .instances
        .update_domain_certificate_state(
            "test-instance",
            None,
            "cloud.example.com",
            "active",
            "cert-1",
            Some("entry-1"),
            Some("dns-auth-1"),
            Some("_acme.example.com"),
            Some("CNAME"),
            Some("challenge.example.net"),
            None,
        )
        .await
        .unwrap();

    let result = app
        .remove_custom_domain
        .execute(&ctx, "test-instance", None, "cloud.example.com")
        .await
        .unwrap();
    assert_eq!(result, DomainRemoveResult::Deleted);

    let reloaded = app
        .get_custom_domain
        .execute(&ctx, "test-instance", None, "cloud.example.com")
        .await
        .unwrap()
        .expect("domain should remain until cleanup effect completes");
    assert_eq!(reloaded.state, "deprovisioning");

    let effects = effects.effects_for_instance("test-instance");
    assert_eq!(effects.len(), 1);
    assert_eq!(
        effects[0].effect_type,
        zitadel_app::effect::EffectType::DomainDeprovisioning
    );
    assert_eq!(effects[0].config["domain"], "cloud.example.com");
}

// ─── ApplicationServices wiring test ─────────────────────

#[tokio::test]
async fn application_services_wired_correctly() {
    let (app, _) = test_services();

    // Verify all use case fields are accessible (compile-time check + runtime sanity)
    // Each field should be callable without panicking on construction
    let _ = &app.create_user;
    let _ = &app.get_user;
    let _ = &app.list_users;
    let _ = &app.update_user;
    let _ = &app.deactivate_user;
    let _ = &app.set_password;
    let _ = &app.verify_password;
    let _ = &app.link_identity;
    let _ = &app.start_login;
    let _ = &app.submit_login_step;
    let _ = &app.issue_session;
    let _ = &app.revoke_session;
    let _ = &app.create_org;
    let _ = &app.get_org;
    let _ = &app.list_orgs;
    let _ = &app.update_org;
    let _ = &app.delete_org;
    let _ = &app.create_group;
    let _ = &app.get_group;
    let _ = &app.list_groups;
    let _ = &app.update_group;
    let _ = &app.create_instance;
    let _ = &app.get_instance;
    let _ = &app.list_instances;
    let _ = &app.update_instance;
    let _ = &app.deprovision_instance;
    let _ = &app.get_settings;
    let _ = &app.update_settings;
    let _ = &app.create_provider;
    let _ = &app.get_provider;
    let _ = &app.list_providers;
    let _ = &app.update_provider;
    let _ = &app.delete_provider;
    let _ = &app.register_schema;
    let _ = &app.get_schema;
    let _ = &app.list_schemas;
    let _ = &app.hooks;
}
