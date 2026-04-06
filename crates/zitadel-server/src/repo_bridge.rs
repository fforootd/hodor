//! Bridge repository implementations for server wiring (ADR-032).
//!
//! Builds `Repositories` from production implementations in `zitadel-db/src/repo_impls/`
//! plus thin wrappers for Session (KvStore), FGA (FgaService), and RawQuery (Db).

use std::sync::Arc;
use zitadel_app::repo::*;
use zitadel_db::Db;
use zitadel_db::repo_impls::*;
use zitadel_fga::{
    CheckRequest, Evaluator, FgaService, ReadRequest, StoreResolver, TupleFilter, TupleKey,
    TupleKeySet, TupleRepository, WriteRequest,
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
        raw: Arc::new(DbRawQueryRepo(db.clone())),
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
            let store = self.0.discover_store(&instance_id).await?;
            Ok(self.0.check(&instance_id, &store.id, req).await?.allowed)
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
            let store = self.0.discover_store(&instance_id).await?;
            self.0.write_tuples(&instance_id, &store.id, req).await?;
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
            let store = self.0.discover_store(&instance_id).await?;
            self.0.write_tuples(&instance_id, &store.id, req).await?;
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
            let store = self.0.discover_store(&instance_id).await?;
            let req = ReadRequest {
                tuple_key: tuple_filter,
                page_size: None,
                continuation_token: None,
            };
            let resp = self.0.read_tuples(&instance_id, &store.id, req).await?;
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

// ─── Raw Query (delegates to zitadel_db functions) ──────

struct DbRawQueryRepo(Db);

fn static_table(table: &str) -> anyhow::Result<&'static str> {
    match table {
        "apps" => Ok("apps"),
        "projects" => Ok("projects"),
        "groups" => Ok("groups"),
        "orgs" => Ok("orgs"),
        "users" => Ok("users"),
        "schemas" => Ok("schemas"),
        other => anyhow::bail!("unknown table for named resource: {other}"),
    }
}

impl RawQueryRepository for DbRawQueryRepo {
    fn create_named_resource(
        &self,
        instance_id: &str,
        table: &str,
        id: &str,
        name: &str,
        org_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<NamedResourceRecord>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        let tbl = match static_table(table) {
            Ok(t) => t,
            Err(e) => return Box::pin(async move { Err(e) }),
        };
        let id = id.to_string();
        let name = name.to_string();
        let oid = org_id.to_string();
        Box::pin(async move {
            let r = zitadel_db::create_named_resource(&db, &iid, tbl, &id, &name, &oid).await?;
            Ok(NamedResourceRecord {
                id: r.id,
                name: r.name,
                state: r.state,
                created_at: r.created_at,
                updated_at: r.updated_at,
            })
        })
    }
    fn get_named_resource(
        &self,
        instance_id: &str,
        table: &str,
        id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Option<NamedResourceRecord>>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        let tbl = match static_table(table) {
            Ok(t) => t,
            Err(e) => return Box::pin(async move { Err(e) }),
        };
        let id = id.to_string();
        Box::pin(async move {
            let r = zitadel_db::get_named_resource(&db, &iid, tbl, &id).await?;
            Ok(r.map(|r| NamedResourceRecord {
                id: r.id,
                name: r.name,
                state: r.state,
                created_at: r.created_at,
                updated_at: r.updated_at,
            }))
        })
    }
    fn list_named_resources(
        &self,
        instance_id: &str,
        table: &str,
        cursor: &str,
        limit: i64,
    ) -> BoxFuture<'_, anyhow::Result<Vec<NamedResourceRecord>>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        let tbl = match static_table(table) {
            Ok(t) => t,
            Err(e) => return Box::pin(async move { Err(e) }),
        };
        let cur = cursor.to_string();
        Box::pin(async move {
            let rows = zitadel_db::list_named_resources(&db, &iid, tbl, &cur, limit).await?;
            Ok(rows
                .into_iter()
                .map(|r| NamedResourceRecord {
                    id: r.id,
                    name: r.name,
                    state: r.state,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                })
                .collect())
        })
    }
    fn update_named_resource_name(
        &self,
        instance_id: &str,
        table: &str,
        id: &str,
        name: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        let tbl = match static_table(table) {
            Ok(t) => t,
            Err(e) => return Box::pin(async move { Err(e) }),
        };
        let id = id.to_string();
        let name = name.to_string();
        Box::pin(
            async move { zitadel_db::update_named_resource_name(&db, &iid, tbl, &id, &name).await },
        )
    }
    fn delete_named_resource(
        &self,
        instance_id: &str,
        table: &str,
        id: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        let tbl = match static_table(table) {
            Ok(t) => t,
            Err(e) => return Box::pin(async move { Err(e) }),
        };
        let id = id.to_string();
        Box::pin(async move { zitadel_db::delete_instance_row(&db, &iid, tbl, &id).await })
    }
    fn load_console_bootstrap(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<ConsoleBootstrapData>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        Box::pin(async move {
            let data = zitadel_db::load_console_bootstrap_data(&db, &iid).await?;
            Ok(ConsoleBootstrapData {
                counts: data.counts.into_iter().collect(),
                orgs: data
                    .orgs
                    .into_iter()
                    .map(|o| OrgSummary {
                        id: o.id,
                        name: o.name,
                        state: o.state,
                    })
                    .collect(),
                instance: InstanceInfo {
                    instance_id: data.instance.instance_id,
                    kind: data.instance.kind,
                    feature_overrides_json: data.instance.feature_overrides_json,
                    parent_instance_id: data.instance.parent_instance_id,
                },
            })
        })
    }
    fn load_entity_counts(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<(String, i64)>>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        Box::pin(async move {
            let map = zitadel_db::load_entity_counts(&db, &iid).await?;
            Ok(map.into_iter().collect())
        })
    }
    fn list_fingerprints(
        &self,
        instance_id: &str,
        cursor: &str,
        limit: i64,
    ) -> BoxFuture<'_, anyhow::Result<Vec<FingerprintRecord>>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        let cur = cursor.to_string();
        Box::pin(async move {
            let rows = zitadel_db::list_fingerprints(&db, &iid, &cur, limit).await?;
            Ok(rows
                .into_iter()
                .map(|r| FingerprintRecord {
                    id: r.id,
                    type_: r.type_,
                    raw_data_json: r.raw_data_json,
                    created_at: r.created_at,
                })
                .collect())
        })
    }
    fn upsert_fingerprint(
        &self,
        instance_id: &str,
        id: &str,
        type_: &str,
        raw_data: &str,
    ) -> BoxFuture<'_, anyhow::Result<()>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        let id = id.to_string();
        let t = type_.to_string();
        let rd = raw_data.to_string();
        Box::pin(async move {
            zitadel_db::upsert_fingerprint(&db, &iid, &id, &t, &rd)
                .await
                .map(|_| ())
        })
    }
    fn list_jobs(&self, instance_id: &str) -> BoxFuture<'_, anyhow::Result<Vec<JobRecord>>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        Box::pin(async move {
            let rows = zitadel_db::list_jobs_for_instance(&db, &iid).await?;
            Ok(rows
                .into_iter()
                .map(|r| JobRecord {
                    name: r.name,
                    display_name: r.display_name,
                    description: r.description,
                    cron: r.cron,
                    enabled: r.enabled,
                    last_status: r.last_status,
                    last_error: r.last_error,
                    run_count: r.run_count,
                    last_rows_removed: r.last_rows_removed,
                    last_run_at: r.last_run_at,
                    next_run_at: r.next_run_at,
                    lease_expires_at: r.lease_expires_at,
                    config_json: r.config_json,
                    created_at: r.created_at,
                    updated_at: r.updated_at,
                })
                .collect())
        })
    }
    fn list_saved_queries(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SavedQueryRecord>>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        Box::pin(async move {
            let rows = zitadel_db::list_saved_queries(&db, &iid).await?;
            Ok(rows
                .into_iter()
                .map(|r| SavedQueryRecord {
                    id: r.id,
                    name: r.name,
                    description: r.description,
                    sql: r.sql,
                    created_at: r.created_at,
                })
                .collect())
        })
    }
    fn create_saved_query(
        &self,
        instance_id: &str,
        id: &str,
        name: &str,
        description: &str,
        sql: &str,
    ) -> BoxFuture<'_, anyhow::Result<SavedQueryRecord>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        let id = id.to_string();
        let name = name.to_string();
        let desc = description.to_string();
        let sql = sql.to_string();
        Box::pin(async move {
            let r = zitadel_db::create_saved_query(&db, &iid, &id, &name, &desc, &sql).await?;
            Ok(SavedQueryRecord {
                id: r.id,
                name: r.name,
                description: r.description,
                sql: r.sql,
                created_at: r.created_at,
            })
        })
    }
    fn delete_saved_query(
        &self,
        instance_id: &str,
        id: &str,
    ) -> BoxFuture<'_, anyhow::Result<bool>> {
        let db = self.0.clone();
        let iid = instance_id.to_string();
        let id = id.to_string();
        Box::pin(async move { zitadel_db::delete_saved_query(&db, &iid, &id).await })
    }
}
