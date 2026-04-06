//! Bridge repository implementations for server wiring (ADR-032).
//!
//! Builds `Repositories` from production implementations in `zitadel-db/src/repo_impls/`
//! plus thin wrappers for Session (KvStore) and FGA (FgaService).

use std::sync::Arc;
use zitadel_app::repo::*;
use zitadel_db::Db;
use zitadel_db::repo_impls::*;
use zitadel_fga::{
    CheckRequest, Evaluator, FgaService, ReadRequest, TupleFilter, TupleKey, TupleKeySet,
    TupleRepository, WriteRequest,
};
use zitadel_storage::DefaultTransientStorage;

/// Build the complete `Repositories` container from existing infrastructure.
pub fn build_repositories(
    db: Db,
    transient: Arc<DefaultTransientStorage>,
    fga: Arc<FgaService>,
) -> Repositories {
    Repositories {
        users: Arc::new(SqlUserRepository::new(db.clone())),
        orgs: Arc::new(SqlOrgRepository::new(db.clone())),
        apps: Arc::new(DbAppRepository::new(db.clone())),
        projects: Arc::new(DbProjectRepository::new(db.clone())),
        credentials: Arc::new(DbCredentialRepository::new(db.clone())),
        sessions: Arc::new(KvSessionRepo(transient, db.clone())),
        instances: Arc::new(SqlInstanceRepository::new(db.clone())),
        providers: Arc::new(SqlProviderRepository::new(db.clone())),
        login_flows: Arc::new(DbLoginFlowRepository::new(db.clone())),
        oidc: Arc::new(DbOidcRepository::new(db.clone())),
        events: Arc::new(DbEventRepository::new(db.clone())),
        settings: Arc::new(SqlSettingsRepository::new(db.clone())),
        fga: Arc::new(FgaBridge(fga)),
        schemas: Arc::new(SqlSchemaRepository::new(db.clone())),
        groups: Arc::new(SqlGroupRepository::new(db.clone())),
        pats: Arc::new(DbPatRepository::new(db.clone())),
        search: Arc::new(SqlSearchRepository::new(db.clone())),
        actions: Arc::new(DbActionRepository::new(db.clone())),
        memberships: Arc::new(DbMembershipRepository::new(db.clone())),
        console_queries: Arc::new(DbConsoleQueryRepository::new(db.clone())),
        telemetry: Arc::new(DbTelemetryRepository::new(db.clone())),
        jobs: Arc::new(DbJobRepository::new(db.clone())),
        saved_queries: Arc::new(DbSavedQueryRepository::new(db.clone())),
        authorization: Arc::new(DbAuthorizationRepository::new(db.clone())),
        uow: Arc::new(SqlUnitOfWorkFactory::new(db)),
    }
}

// ─── Session (delegates to KvStore + SQL for metadata) ───

struct KvSessionRepo(Arc<DefaultTransientStorage>, Db);

impl SessionRepository for KvSessionRepo {
    fn create(
        &self,
        instance_id: &str,
        user_id: &str,
        org_id: &str,
        _auth_method: &str,
        user_agent: &str,
        ip_address: &str,
        fingerprint: &str,
    ) -> BoxFuture<'_, anyhow::Result<CreatedSession>> {
        let iid = instance_id.to_string();
        let uid = user_id.to_string();
        let oid = org_id.to_string();
        let ua = user_agent.to_string();
        let ip = ip_address.to_string();
        let fp = fingerprint.to_string();
        Box::pin(async move {
            let created = self
                .0
                .create_session(&iid, &uid, &oid, &ua, &ip, &fp)
                .await?;
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

    fn update_metadata(
        &self,
        instance_id: &str,
        session_id: &str,
        metadata_json: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.1.clone();
        let iid = instance_id.to_string();
        let sid = session_id.to_string();
        let meta = metadata_json.to_string();
        Box::pin(async move {
            zitadel_db::update_session_metadata(&db, &iid, &sid, &meta).await?;
            Ok(())
        })
    }

    fn list_by_instance(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SessionDetail>>> {
        let iid = instance_id.to_string();
        Box::pin(async move {
            let rows = self.0.list_sessions(&iid).await?;
            Ok(rows
                .into_iter()
                .map(|r| SessionDetail {
                    id: r.id,
                    user_id: r.user_id,
                    org_id: r.org_id,
                    user_agent: r.user_agent,
                    ip_address: r.ip_address,
                    created_at: r.created_at,
                    expires_at: r.expires_at,
                    revoked_at: r.revoked_at,
                })
                .collect())
        })
    }

    fn get(
        &self,
        instance_id: &str,
        session_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<SessionDetail>>> {
        let iid = instance_id.to_string();
        let sid = session_id.to_string();
        Box::pin(async move {
            let r = self.0.get_session(&iid, &sid).await?;
            Ok(r.map(|r| SessionDetail {
                id: r.id,
                user_id: r.user_id,
                org_id: r.org_id,
                user_agent: r.user_agent,
                ip_address: r.ip_address,
                created_at: r.created_at,
                expires_at: r.expires_at,
                revoked_at: r.revoked_at,
            }))
        })
    }
}

// ─── FGA (delegates to FgaService) ───────────────────────

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
        let req = CheckRequest {
            tuple_key: TupleKey {
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
            let _ = instance_id;
            let store = self.0.discover_platform_store().await?;
            Ok(self.0.check(zitadel_fga::PLATFORM_STORE_ID, &store.id, req).await?.allowed)
        })
    }

    fn write(
        &self,
        instance_id: &str,
        writes: Vec<(String, String, String)>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let instance_id = instance_id.to_string();
        let tuples = TupleKeySet {
            tuple_keys: writes
                .into_iter()
                .map(|(user, relation, object)| TupleKey {
                    user,
                    relation,
                    object,
                    condition: None,
                })
                .collect(),
        };
        let req = WriteRequest {
            writes: tuples,
            deletes: TupleKeySet::default(),
            authorization_model_id: None,
        };
        Box::pin(async move {
            let _ = instance_id;
            let store = self.0.discover_platform_store().await?;
            self.0
                .write_tuples(zitadel_fga::PLATFORM_STORE_ID, &store.id, req)
                .await?;
            Ok(())
        })
    }

    fn delete(
        &self,
        instance_id: &str,
        deletes: Vec<(String, String, String)>,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let instance_id = instance_id.to_string();
        let tuples = TupleKeySet {
            tuple_keys: deletes
                .into_iter()
                .map(|(user, relation, object)| TupleKey {
                    user,
                    relation,
                    object,
                    condition: None,
                })
                .collect(),
        };
        let req = WriteRequest {
            writes: TupleKeySet::default(),
            deletes: tuples,
            authorization_model_id: None,
        };
        Box::pin(async move {
            let _ = instance_id;
            let store = self.0.discover_platform_store().await?;
            self.0
                .write_tuples(zitadel_fga::PLATFORM_STORE_ID, &store.id, req)
                .await?;
            Ok(())
        })
    }

    fn read(
        &self,
        instance_id: &str,
        filter: Option<(String, String, String)>,
    ) -> BoxFuture<'_, anyhow::Result<Vec<FgaRelation>>> {
        let instance_id = instance_id.to_string();
        let tuple_filter = filter.map(|(user, relation, object)| TupleFilter {
            user: if user.is_empty() { None } else { Some(user) },
            relation: if relation.is_empty() {
                None
            } else {
                Some(relation)
            },
            object: if object.is_empty() {
                None
            } else {
                Some(object)
            },
        });
        Box::pin(async move {
            let _ = instance_id;
            let store = self.0.discover_platform_store().await?;
            let req = ReadRequest {
                tuple_key: tuple_filter,
                page_size: None,
                continuation_token: None,
            };
            let resp = self
                .0
                .read_tuples(zitadel_fga::PLATFORM_STORE_ID, &store.id, req)
                .await?;
            Ok(resp
                .tuples
                .into_iter()
                .map(|t| FgaRelation {
                    user: t.key.user,
                    relation: t.key.relation,
                    object: t.key.object,
                })
                .collect())
        })
    }
}
