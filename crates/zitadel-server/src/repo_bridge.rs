//! Bridge repository implementations for server wiring (ADR-032 CLAUDE-4).
//!
//! Provides stub `Repositories` so `ApplicationServices` can be wired at startup.
//! The system continues to work through old code paths (handlers call DB directly)
//! until CLAUDE-1/CLAUDE-2 rewrite handlers to call use cases.
//!
//! CODEX-1/CODEX-2 will deliver production-quality repo impls in
//! `zitadel-db/src/repo_impls/` to replace these stubs.

use std::sync::Arc;
use zitadel_app::repo::*;
use zitadel_db::Db;
use zitadel_fga::{Evaluator, FgaService};
use zitadel_storage::{DefaultStatefulStorage, DefaultTransientStorage};

/// Build the complete `Repositories` container from existing infrastructure.
///
/// Most repositories are stub implementations that `todo!()` on every method.
/// The Session bridge delegates to `DefaultTransientStorage` and works end-to-end.
/// The FGA bridge delegates to `FgaService::check()`.
pub fn build_repositories(
    db: Db,
    _stateful: Arc<DefaultStatefulStorage>,
    transient: Arc<DefaultTransientStorage>,
    fga: Arc<FgaService>,
) -> Repositories {
    let stub = StubDb(db);
    Repositories {
        users: Arc::new(StubUserRepo(stub.clone())),
        orgs: Arc::new(StubOrgRepo(stub.clone())),
        credentials: Arc::new(StubCredentialRepo(stub.clone())),
        sessions: Arc::new(KvSessionRepo(transient)),
        instances: Arc::new(StubInstanceRepo(stub.clone())),
        providers: Arc::new(StubProviderRepo(stub.clone())),
        login_flows: Arc::new(StubLoginFlowRepo(stub.clone())),
        oidc: Arc::new(StubOidcRepo(stub.clone())),
        events: Arc::new(StubEventRepo(stub.clone())),
        settings: Arc::new(StubSettingsRepo(stub.clone())),
        fga: Arc::new(FgaBridge(fga)),
        schemas: Arc::new(StubSchemaRepo(stub.clone())),
        groups: Arc::new(StubGroupRepo(stub.clone())),
        pats: Arc::new(StubPatRepo(stub.clone())),
        search: Arc::new(StubSearchRepo(stub.clone())),
        actions: Arc::new(StubActionRepo(stub)),
    }
}

// ─── Shared wrapper ──────────────────────────────────────

#[derive(Clone)]
struct StubDb(Db);

// ─── Session (live — delegates to KvStore) ───────────────

struct KvSessionRepo(Arc<DefaultTransientStorage>);

impl SessionRepository for KvSessionRepo {
    fn create(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        auth_method: &str,
    ) -> BoxFuture<'_, anyhow::Result<CreatedSession>> {
        let iid = instance_id.to_string();
        let uid = user_id.to_string();
        let oid = org_id.to_string();
        let am = auth_method.to_string();
        Box::pin(async move {
            let created = self.0.create_session(&iid, &uid, &oid, &am, "", "").await?;
            Ok(CreatedSession {
                session_id: created.session_id,
                token: created.token,
            })
        })
    }

    fn find_by_token(
        &self,
        instance_id: &str,
        token: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SessionInfo>>> {
        let iid = instance_id.to_string();
        let tok = token.to_string();
        Box::pin(async move {
            let s = self.0.find_session_by_token(&iid, &tok).await?;
            Ok(s.map(|s| SessionInfo {
                session_id: s.id,
                user_id: s.user_id,
                org_id: s.org_id,
                token_type: "session".to_string(),
            }))
        })
    }

    fn revoke(&self, instance_id: &str, session_id: &str) -> BoxFuture<'_, anyhow::Result<bool>> {
        let iid = instance_id.to_string();
        let sid = session_id.to_string();
        Box::pin(async move {
            self.0.revoke_session(&iid, &sid).await?;
            Ok(true)
        })
    }
}

// ─── FGA (live — delegates to FgaService) ────────────────

struct FgaBridge(Arc<FgaService>);

impl FgaRepository for FgaBridge {
    fn check(
        &self,
        instance_id: &str,
        user: &str,
        relation: &str,
        object: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let instance_id = instance_id.to_string();
        let req = zitadel_fga::CheckRequest {
            tuple_key: zitadel_fga::TupleKey {
                user: user.to_string(),
                relation: relation.to_string(),
                object: object.to_string(),
                condition: None,
            },
            authorization_model_id: None,
            contextual_tuples: None,
            context: None,
        };
        Box::pin(async move {
            let resp = self.0.check(&instance_id, &instance_id, req).await?;
            Ok(resp.allowed)
        })
    }
    fn write_tuple(&self, _: &str, _: &str, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-2: FGA write") })
    }
    fn delete_tuple(
        &self,
        _: &str,
        _: &str,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-2: FGA delete") })
    }
    fn list_relations(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<FgaRelation>>> {
        Box::pin(async { todo!("CODEX-2: FGA list") })
    }
}

// ─── Stub repos (todo! on all methods) ──────────────────
// These compile and satisfy the type system. Runtime calls panic with
// a clear message indicating which CODEX stream owns the implementation.

macro_rules! stub_struct {
    ($name:ident) => {
        #[allow(dead_code)]
        struct $name(StubDb);
    };
}

stub_struct!(StubUserRepo);
stub_struct!(StubOrgRepo);
stub_struct!(StubCredentialRepo);
stub_struct!(StubInstanceRepo);
stub_struct!(StubProviderRepo);
stub_struct!(StubLoginFlowRepo);
stub_struct!(StubOidcRepo);
stub_struct!(StubEventRepo);
stub_struct!(StubSettingsRepo);
stub_struct!(StubSchemaRepo);
stub_struct!(StubGroupRepo);
stub_struct!(StubPatRepo);
stub_struct!(StubSearchRepo);
stub_struct!(StubActionRepo);

impl UserRepository for StubUserRepo {
    fn create(&self, _: &str, _: &UserRecord) -> BoxFuture<'_, anyhow::Result<UserRecord>> {
        Box::pin(async { todo!("CODEX-1: UserRepository::create") })
    }
    fn get(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<UserRecord>>> {
        Box::pin(async { todo!("CODEX-1: UserRepository::get") })
    }
    fn find_by_identifier(
        &self,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserRecord>>> {
        Box::pin(async { todo!("CODEX-1: UserRepository::find_by_identifier") })
    }
    fn list(
        &self,
        _: &str,
        _: Option<&str>,
        _: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<UserRecord>>> {
        Box::pin(async { todo!("CODEX-1: UserRepository::list") })
    }
    fn update(&self, _: &str, _: &UserRecord) -> BoxFuture<'_, anyhow::Result<UserRecord>> {
        Box::pin(async { todo!("CODEX-1: UserRepository::update") })
    }
    fn deactivate(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-1: UserRepository::deactivate") })
    }
}

impl OrgRepository for StubOrgRepo {
    fn create(&self, _: &str, _: &OrgRecord) -> BoxFuture<'_, anyhow::Result<OrgRecord>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn get(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<OrgRecord>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn list(
        &self,
        _: &str,
        _: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<OrgRecord>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn update(&self, _: &str, _: &OrgRecord) -> BoxFuture<'_, anyhow::Result<OrgRecord>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn first_org_id(&self, _: &str) -> BoxFuture<'_, anyhow::Result<Option<String>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
}

impl CredentialRepository for StubCredentialRepo {
    fn set_password(&self, _: &str, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-2") })
    }
    fn get_password_hash(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<String>>> {
        Box::pin(async { todo!("CODEX-2") })
    }
    fn link_identity(
        &self,
        _: &str,
        _: &LinkedIdentityRecord,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-2") })
    }
    fn unlink_identity(&self, _: &str, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-2") })
    }
    fn list_linked_identities(
        &self,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<LinkedIdentityRecord>>> {
        Box::pin(async { todo!("CODEX-2") })
    }
    fn find_by_external_sub(
        &self,
        _: &str,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<LinkedIdentityRecord>>> {
        Box::pin(async { todo!("CODEX-2") })
    }
}

impl InstanceRepository for StubInstanceRepo {
    fn create(&self, _: &str, _: &InstanceRecord) -> BoxFuture<'_, anyhow::Result<InstanceRecord>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn get(&self, _: &str) -> BoxFuture<'_, anyhow::Result<Option<InstanceRecord>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn list(
        &self,
        _: &str,
        _: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<InstanceRecord>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn update(&self, _: &InstanceRecord) -> BoxFuture<'_, anyhow::Result<InstanceRecord>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn deprovision(&self, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn resolve_domain(&self, _: &str) -> BoxFuture<'_, anyhow::Result<Option<RouteResolution>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn list_domains(&self, _: &str) -> BoxFuture<'_, anyhow::Result<Vec<DomainRecord>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn set_domain(&self, _: &str, _: &DomainRecord) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-1") })
    }
}

impl ProviderRepository for StubProviderRepo {
    fn create(&self, _: &str, _: &ProviderRecord) -> BoxFuture<'_, anyhow::Result<ProviderRecord>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn get(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<ProviderRecord>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn list(
        &self,
        _: &str,
        _: &ListParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<ProviderRecord>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn update(&self, _: &str, _: &ProviderRecord) -> BoxFuture<'_, anyhow::Result<ProviderRecord>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn delete(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-1") })
    }
}

impl LoginFlowRepository for StubLoginFlowRepo {
    fn get_flow(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<Option<LoginFlowRecord>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn list_flows(&self, _: &str) -> BoxFuture<'_, anyhow::Result<Vec<LoginFlowRecord>>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn upsert_flow(&self, _: &str, _: &LoginFlowRecord) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-1") })
    }
    fn delete_flow(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { todo!("CODEX-1") })
    }
}

impl OidcRepository for StubOidcRepo {
    fn find_client(
        &self,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OidcClientInfo>>> {
        Box::pin(async { todo!("CODEX-2") })
    }
    fn create_auth_request(
        &self,
        _: &str,
        _: &OidcAuthRequest,
    ) -> BoxFuture<'_, anyhow::Result<String>> {
        Box::pin(async { todo!("CODEX-2") })
    }
    fn consume_auth_code(
        &self,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<OidcAuthRequest>>> {
        Box::pin(async { todo!("CODEX-2") })
    }
    fn load_user_claims(
        &self,
        _: &str,
        _: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<UserClaims>>> {
        Box::pin(async { todo!("CODEX-2") })
    }
}

impl EventRepository for StubEventRepo {
    fn append(
        &self,
        _: &str,
        _: &zitadel_app::DomainEvent,
        _: Option<&str>,
        _: Option<&str>,
        _: Option<&str>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        // Events are fire-and-forget for now; silently succeed rather than panicking.
        Box::pin(async { Ok(()) })
    }
    fn list(
        &self,
        _: &str,
        _: &EventQueryParams,
    ) -> BoxFuture<'_, anyhow::Result<ListResult<EventRecord>>> {
        Box::pin(async {
            Ok(ListResult {
                items: vec![],
                next_cursor: None,
                total_count: None,
            })
        })
    }
}

impl SettingsRepository for StubSettingsRepo {
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

impl SchemaRepository for StubSchemaRepo {
    fn register(&self, _: &str, _: &SchemaRecord) -> BoxFuture<'_, anyhow::Result<SchemaRecord>> {
        Box::pin(async { todo!("CODEX-1") })
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
        Box::pin(async { todo!("CODEX-1") })
    }
}

impl GroupRepository for StubGroupRepo {
    fn create(&self, _: &str, _: &GroupRecord) -> BoxFuture<'_, anyhow::Result<GroupRecord>> {
        Box::pin(async { todo!("CODEX-1") })
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
        Box::pin(async { todo!("CODEX-1") })
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

impl PatRepository for StubPatRepo {
    fn create(&self, _: &str, _: &PatRecord, _: &str) -> BoxFuture<'_, anyhow::Result<String>> {
        Box::pin(async { todo!("CODEX-2") })
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

impl SearchRepository for StubSearchRepo {
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

impl ActionRepository for StubActionRepo {
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
        Box::pin(async { todo!("CODEX-2") })
    }
    fn update(&self, _: &str, _: &ActionRecord) -> BoxFuture<'_, anyhow::Result<ActionRecord>> {
        Box::pin(async { todo!("CODEX-2") })
    }
    fn delete(&self, _: &str, _: &str) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}
