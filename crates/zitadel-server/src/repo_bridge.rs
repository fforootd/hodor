//! Bridge repository implementations for server wiring (ADR-032).
//!
//! Builds `Repositories` from production implementations in `zitadel-db/src/repo_impls/`
//! plus thin wrappers for Session (KvStore) and FGA (FgaService).

use std::sync::Arc;
use zitadel_app::repo::*;
use zitadel_db::repo_impls::*;
use zitadel_db::{Db, Dialect};
use zitadel_fga::{
    AuthorizationModelWriteRequest, BatchCheckRequest, ChangeRepository, CheckRequest, Evaluator,
    ExpandRequest, FgaApi, FgaError, FgaService, ListObjectsRequest, ListUsersRequest,
    ModelRepository, ReadRequest, StoreResolver, TupleFilter, TupleKey, TupleKeySet,
    TupleRepository, WriteRequest,
};
use zitadel_storage::{DefaultAnalyticsStorage, DefaultTransientStorage};

/// Build the complete `Repositories` container from existing infrastructure.
pub fn build_repositories(
    db: Db,
    transient: Arc<DefaultTransientStorage>,
    fga: Arc<FgaService>,
    analytics: Arc<DefaultAnalyticsStorage>,
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
        fga: Arc::new(FgaBridge(fga.clone())),
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
        fga_admin: Arc::new(FgaAdminBridge(fga.clone())),
        catalog: Arc::new(CatalogBridge(db.clone())),
        observability: Arc::new(ObservabilityBridge {
            db: db.clone(),
            analytics,
        }),
        schema_registry: Arc::new(SchemaRegistryBridge(db.clone())),
        oidc_tokens: Arc::new(DbOidcTokenRepository::new(db.clone())),
        oidc_keys: Arc::new(DbOidcKeyRepository::new(db.clone())),
        effects: Arc::new(DbEffectRepository::new(db.clone())),
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
            Ok(self
                .0
                .check(zitadel_fga::PLATFORM_STORE_ID, &store.id, req)
                .await?
                .allowed)
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

// ─── FGA Admin (delegates to FgaService) ────────────────

fn fga_to_admin_error(e: FgaError) -> FgaAdminError {
    match e {
        FgaError::BadRequest(msg) => FgaAdminError::BadRequest(msg),
        FgaError::NotFound(msg) => FgaAdminError::NotFound(msg),
        FgaError::Forbidden(msg) => FgaAdminError::Forbidden(msg),
        FgaError::Unsupported(msg) => FgaAdminError::Unsupported(msg),
        FgaError::Internal(e) => FgaAdminError::Internal(e),
    }
}

struct FgaAdminBridge(Arc<FgaService>);

impl FgaAdminRepository for FgaAdminBridge {
    fn discover_store(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, Result<FgaStoreInfo, FgaAdminError>> {
        let iid = instance_id.to_string();
        Box::pin(async move {
            let store = self
                .0
                .discover_store(&iid)
                .await
                .map_err(fga_to_admin_error)?;
            Ok(FgaStoreInfo {
                id: store.id,
                name: store.name,
            })
        })
    }

    fn discover_platform_store(&self) -> BoxFuture<'_, Result<FgaStoreInfo, FgaAdminError>> {
        Box::pin(async move {
            let store = self
                .0
                .discover_platform_store()
                .await
                .map_err(fga_to_admin_error)?;
            Ok(FgaStoreInfo {
                id: store.id,
                name: store.name,
            })
        })
    }

    fn check(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        Box::pin(async move {
            let req: CheckRequest = serde_json::from_value(request)
                .map_err(|e| FgaAdminError::BadRequest(e.to_string()))?;
            let result = self
                .0
                .check(&iid, &sid, req)
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn batch_check(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        Box::pin(async move {
            let req: BatchCheckRequest = serde_json::from_value(request)
                .map_err(|e| FgaAdminError::BadRequest(e.to_string()))?;
            let result = self
                .0
                .batch_check(&iid, &sid, req)
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn read_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        Box::pin(async move {
            let req: ReadRequest = serde_json::from_value(request)
                .map_err(|e| FgaAdminError::BadRequest(e.to_string()))?;
            let result = self
                .0
                .read_tuples(&iid, &sid, req)
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn write_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<(), FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        Box::pin(async move {
            let req: WriteRequest = serde_json::from_value(request)
                .map_err(|e| FgaAdminError::BadRequest(e.to_string()))?;
            self.0
                .write_tuples(&iid, &sid, req)
                .await
                .map_err(fga_to_admin_error)?;
            Ok(())
        })
    }

    fn expand(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        Box::pin(async move {
            let req: ExpandRequest = serde_json::from_value(request)
                .map_err(|e| FgaAdminError::BadRequest(e.to_string()))?;
            let result = self
                .0
                .expand(&iid, &sid, req)
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn list_objects(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        Box::pin(async move {
            let req: ListObjectsRequest = serde_json::from_value(request)
                .map_err(|e| FgaAdminError::BadRequest(e.to_string()))?;
            let result = self
                .0
                .list_objects(&iid, &sid, req)
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn list_users(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        Box::pin(async move {
            let req: ListUsersRequest = serde_json::from_value(request)
                .map_err(|e| FgaAdminError::BadRequest(e.to_string()))?;
            let result = self
                .0
                .list_users(&iid, &sid, req)
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn read_model(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        let mid = model_id.map(|s| s.to_string());
        Box::pin(async move {
            let result = self
                .0
                .read_model(&iid, &sid, mid.as_deref())
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn read_models(
        &self,
        instance_id: &str,
        store_id: &str,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        Box::pin(async move {
            let result = self
                .0
                .read_models(&iid, &sid)
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn write_model(
        &self,
        instance_id: &str,
        store_id: &str,
        request: serde_json::Value,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        Box::pin(async move {
            let req: AuthorizationModelWriteRequest = serde_json::from_value(request)
                .map_err(|e| FgaAdminError::BadRequest(e.to_string()))?;
            let result = self
                .0
                .write_model(&iid, &sid, req)
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn read_changes(
        &self,
        instance_id: &str,
        store_id: &str,
        object_type: Option<&str>,
        page_size: u32,
        continuation_token: Option<&str>,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        let sid = store_id.to_string();
        let ot = object_type.map(|s| s.to_string());
        let ct = continuation_token.map(|s| s.to_string());
        Box::pin(async move {
            let result = self
                .0
                .read_changes(&iid, &sid, ot.as_deref(), page_size, ct.as_deref())
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn legacy_model(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        Box::pin(async move {
            let result = self
                .0
                .legacy_model(&iid)
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn legacy_model_graph(
        &self,
        instance_id: &str,
    ) -> BoxFuture<'_, Result<serde_json::Value, FgaAdminError>> {
        let iid = instance_id.to_string();
        Box::pin(async move {
            let result = self
                .0
                .legacy_model_graph(&iid)
                .await
                .map_err(fga_to_admin_error)?;
            serde_json::to_value(result).map_err(|e| FgaAdminError::Internal(e.into()))
        })
    }

    fn rebuild_platform_store(&self) -> BoxFuture<'_, Result<(), FgaAdminError>> {
        Box::pin(async move {
            self.0
                .rebuild_platform_store()
                .await
                .map_err(fga_to_admin_error)
        })
    }
}

// ─── Catalog (delegates to embedded catalog engine) ──────

struct CatalogBridge(Db);

impl CatalogRepository for CatalogBridge {
    fn install_provider(
        &self,
        _instance_id: &str,
        template_id: &str,
        variables: &serde_json::Value,
    ) -> BoxFuture<'_, anyhow::Result<String>> {
        let tid = template_id.to_string();
        let vars: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_value(variables.clone()).unwrap_or_default();
        let db = self.0.clone();
        Box::pin(async move {
            let catalog = zitadel_catalog::Catalog::embedded();
            catalog.install_provider(&tid, &vars, &db).await
        })
    }

    fn install_action(
        &self,
        _instance_id: &str,
        template_id: &str,
        variables: &serde_json::Value,
    ) -> BoxFuture<'_, anyhow::Result<String>> {
        let tid = template_id.to_string();
        let vars: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_value(variables.clone()).unwrap_or_default();
        let db = self.0.clone();
        Box::pin(async move {
            let catalog = zitadel_catalog::Catalog::embedded();
            catalog.install_action(&tid, &vars, &db).await
        })
    }
}

// ─── Observability (delegates to analytics storage) ─────

struct ObservabilityBridge {
    db: Db,
    analytics: Arc<DefaultAnalyticsStorage>,
}

impl ObservabilityRepository for ObservabilityBridge {
    fn load_overview(
        &self,
        instance_id: &str,
        range_hours: u64,
    ) -> BoxFuture<'_, anyhow::Result<ObservabilityOverview>> {
        let instance_id = instance_id.to_string();
        let analytics = self.analytics.clone();
        let dialect = self.db.dialect();
        Box::pin(async move {
            let hours = range_hours as i64;
            let instance = obs_sql_string_literal(&instance_id);
            let cur_since = obs_recent_timestamp_expr(dialect, hours);
            let prev_since = obs_recent_timestamp_expr(dialect, hours * 2);
            let cur_window = format!("created_at >= {cur_since}");
            let prev_window = format!("created_at >= {prev_since} AND created_at < {cur_since}");
            let now = obs_current_timestamp_expr(dialect);

            // ── Aggregate counts ──
            let counts_sql = format!(
                "SELECT \
                   SUM(CASE WHEN event_type LIKE 'auth.%' AND {cur_window} THEN 1 ELSE 0 END) AS auth_cur, \
                   SUM(CASE WHEN event_type LIKE 'auth.%' AND {prev_window} THEN 1 ELSE 0 END) AS auth_prev, \
                   SUM(CASE WHEN event_type = 'auth.token_issued' AND {cur_window} THEN 1 ELSE 0 END) AS tok_cur, \
                   SUM(CASE WHEN event_type = 'auth.token_issued' AND {prev_window} THEN 1 ELSE 0 END) AS tok_prev, \
                   SUM(CASE WHEN event_type = 'auth.login_failed' AND {cur_window} THEN 1 ELSE 0 END) AS fail_cur, \
                   SUM(CASE WHEN event_type = 'auth.login_failed' AND {prev_window} THEN 1 ELSE 0 END) AS fail_prev \
                 FROM events \
                 WHERE instance_id = {instance} AND event_type NOT LIKE 'log.%' AND created_at >= {prev_since}"
            );
            let counts = obs_row_map(&analytics, counts_sql).await?;

            // ── Session counts ──
            let sessions_current_sql = format!(
                "SELECT COUNT(*) AS total FROM sessions \
                 WHERE instance_id = {instance} AND revoked_at IS NULL AND expires_at > {now} AND {cur_window}"
            );
            let sessions_prev_sql = format!(
                "SELECT COUNT(*) AS total FROM sessions \
                 WHERE instance_id = {instance} AND revoked_at IS NULL AND {prev_window}"
            );
            let sessions_current = obs_scalar_i64(&analytics, sessions_current_sql)
                .await
                .unwrap_or(0);
            let sessions_previous = obs_scalar_i64(&analytics, sessions_prev_sql)
                .await
                .unwrap_or(0);

            // ── Timeseries ──
            let auth_ts = obs_fetch_timestamps(
                &analytics,
                format!(
                    "SELECT created_at FROM events WHERE instance_id = {instance} AND event_type LIKE 'auth.%' AND event_type NOT LIKE 'log.%' AND {cur_window}"
                ),
            ).await?;
            let sess_ts = obs_fetch_timestamps(
                &analytics,
                format!(
                    "SELECT created_at FROM sessions WHERE instance_id = {instance} AND revoked_at IS NULL AND expires_at > {now} AND {cur_window}"
                ),
            ).await?;
            let tok_ts = obs_fetch_timestamps(
                &analytics,
                format!(
                    "SELECT created_at FROM events WHERE instance_id = {instance} AND event_type = 'auth.token_issued' AND {cur_window}"
                ),
            ).await?;
            let fail_ts = obs_fetch_timestamps(
                &analytics,
                format!(
                    "SELECT created_at FROM events WHERE instance_id = {instance} AND event_type = 'auth.login_failed' AND {cur_window}"
                ),
            ).await?;

            // ── Breakdowns ──
            let top_operations = obs_fetch_breakdown(
                &analytics,
                format!(
                    "SELECT event_type AS name, COUNT(*) AS count FROM events \
                     WHERE instance_id = {instance} AND event_type != '' AND event_type NOT LIKE 'log.%' AND {cur_window} \
                     GROUP BY event_type ORDER BY count DESC LIMIT 8"
                ),
            ).await?;
            let top_users = obs_fetch_breakdown(
                &analytics,
                format!(
                    "SELECT COALESCE(NULLIF(actor_id, ''), 'Anonymous') AS name, COUNT(*) AS count FROM events \
                     WHERE instance_id = {instance} AND event_type NOT LIKE 'log.%' AND {cur_window} \
                     GROUP BY COALESCE(NULLIF(actor_id, ''), 'Anonymous') ORDER BY count DESC LIMIT 8"
                ),
            ).await?;
            let top_ips = obs_fetch_breakdown(
                &analytics,
                format!(
                    "SELECT ip_address AS name, COUNT(*) AS count FROM sessions \
                     WHERE instance_id = {instance} AND ip_address IS NOT NULL AND ip_address != '' AND {cur_window} \
                     GROUP BY ip_address ORDER BY count DESC LIMIT 8"
                ),
            ).await?;
            let top_clients = obs_fetch_breakdown(
                &analytics,
                format!(
                    "SELECT COALESCE(NULLIF(client_id, ''), 'Console') AS name, COUNT(*) AS count FROM events \
                     WHERE instance_id = {instance} AND event_type NOT LIKE 'log.%' AND {cur_window} \
                     GROUP BY COALESCE(NULLIF(client_id, ''), 'Console') ORDER BY count DESC LIMIT 8"
                ),
            ).await?;
            let top_sdks = obs_fetch_breakdown(
                &analytics,
                format!(
                    "SELECT COALESCE(NULLIF(sdk_name, ''), 'Browser') AS name, COUNT(*) AS count FROM events \
                     WHERE instance_id = {instance} AND event_type NOT LIKE 'log.%' AND {cur_window} \
                     GROUP BY COALESCE(NULLIF(sdk_name, ''), 'Browser') ORDER BY count DESC LIMIT 8"
                ),
            ).await?;
            let delegation = obs_fetch_breakdown(
                &analytics,
                format!(
                    "SELECT COALESCE(NULLIF(delegation_type, ''), 'direct') AS name, COUNT(*) AS count FROM events \
                     WHERE instance_id = {instance} AND event_type NOT LIKE 'log.%' AND {cur_window} \
                     GROUP BY COALESCE(NULLIF(delegation_type, ''), 'direct') ORDER BY count DESC LIMIT 8"
                ),
            ).await?;

            Ok(ObservabilityOverview {
                auth_current: obs_map_i64(&counts, "auth_cur"),
                auth_previous: obs_map_i64(&counts, "auth_prev"),
                tokens_current: obs_map_i64(&counts, "tok_cur"),
                tokens_previous: obs_map_i64(&counts, "tok_prev"),
                failures_current: obs_map_i64(&counts, "fail_cur"),
                failures_previous: obs_map_i64(&counts, "fail_prev"),
                sessions_current,
                sessions_previous,
                auth_timestamps: auth_ts,
                session_timestamps: sess_ts,
                token_timestamps: tok_ts,
                failure_timestamps: fail_ts,
                top_operations,
                top_users,
                top_ips,
                top_clients,
                top_sdks,
                delegation,
            })
        })
    }
}

// ── Observability helper functions ──────────────────────────

async fn obs_row_map(
    analytics: &DefaultAnalyticsStorage,
    sql: String,
) -> anyhow::Result<std::collections::BTreeMap<String, serde_json::Value>> {
    let result = analytics
        .query(&zitadel_storage::AnalyticsQuery {
            sql,
            params: vec![],
            limit: Some(1),
        })
        .await?;
    if let Some(error) = result.error {
        anyhow::bail!("{error}");
    }
    let Some(row) = result.rows.first() else {
        return Ok(Default::default());
    };
    Ok(result
        .columns
        .iter()
        .cloned()
        .zip(row.iter().cloned())
        .collect())
}

async fn obs_scalar_i64(analytics: &DefaultAnalyticsStorage, sql: String) -> anyhow::Result<i64> {
    let map = obs_row_map(analytics, sql).await?;
    Ok(map.get("total").and_then(obs_value_as_i64).unwrap_or(0))
}

async fn obs_fetch_timestamps(
    analytics: &DefaultAnalyticsStorage,
    sql: String,
) -> anyhow::Result<Vec<i64>> {
    let result = analytics
        .query(&zitadel_storage::AnalyticsQuery {
            sql,
            params: vec![],
            limit: Some(5000),
        })
        .await?;
    if let Some(error) = result.error {
        anyhow::bail!("{error}");
    }
    let created_idx = result
        .columns
        .iter()
        .position(|column| column == "created_at")
        .unwrap_or(0);
    Ok(result
        .rows
        .into_iter()
        .filter_map(|row| row.get(created_idx).and_then(obs_value_as_string))
        .filter_map(|ts| obs_parse_ts_ms(&ts))
        .collect())
}

async fn obs_fetch_breakdown(
    analytics: &DefaultAnalyticsStorage,
    sql: String,
) -> anyhow::Result<Vec<(String, i64)>> {
    let result = analytics
        .query(&zitadel_storage::AnalyticsQuery {
            sql,
            params: vec![],
            limit: Some(8),
        })
        .await?;
    if let Some(error) = result.error {
        anyhow::bail!("{error}");
    }
    let name_idx = result
        .columns
        .iter()
        .position(|column| column == "name")
        .unwrap_or(0);
    let count_idx = result
        .columns
        .iter()
        .position(|column| column == "count")
        .unwrap_or(1);
    Ok(result
        .rows
        .into_iter()
        .map(|row| {
            let name = row
                .get(name_idx)
                .and_then(obs_value_as_string)
                .unwrap_or_default();
            let count = row.get(count_idx).and_then(obs_value_as_i64).unwrap_or(0);
            (name, count)
        })
        .collect())
}

fn obs_map_i64(map: &std::collections::BTreeMap<String, serde_json::Value>, key: &str) -> i64 {
    map.get(key).and_then(obs_value_as_i64).unwrap_or(0)
}

fn obs_value_as_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(raw) => Some(raw.clone()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        serde_json::Value::Bool(flag) => Some(flag.to_string()),
        serde_json::Value::Null => None,
        other => Some(other.to_string()),
    }
}

fn obs_value_as_i64(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(number) => number.as_i64(),
        serde_json::Value::String(raw) => raw.parse().ok(),
        _ => None,
    }
}

fn obs_sql_string_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn obs_current_timestamp_expr(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Sqlite => "datetime('now')",
        Dialect::Postgres => "NOW()",
        Dialect::Spanner => "CURRENT_TIMESTAMP()",
    }
}

fn obs_recent_timestamp_expr(dialect: Dialect, hours: i64) -> String {
    match dialect {
        Dialect::Sqlite => format!("datetime('now', '-{hours} hours')"),
        Dialect::Postgres => format!("NOW() - INTERVAL '{hours} hours'"),
        Dialect::Spanner => format!("TIMESTAMP_SUB(CURRENT_TIMESTAMP(), INTERVAL {hours} HOUR)"),
    }
}

fn obs_parse_ts_ms(ts: &str) -> Option<i64> {
    let s = ts.trim().trim_end_matches('Z');
    let s = s.replace('T', " ");
    let parts: Vec<&str> = s.splitn(6, |c: char| !c.is_ascii_digit()).collect();
    if parts.len() < 6 {
        return None;
    }
    let y: i64 = parts[0].parse().ok()?;
    let mo: i64 = parts[1].parse().ok()?;
    let d: i64 = parts[2].parse().ok()?;
    let h: i64 = parts[3].parse().ok()?;
    let mi: i64 = parts[4].parse().ok()?;
    let se: i64 = parts[5].parse().ok()?;

    let mut days = 0i64;
    for yr in 1970..y {
        days += if yr % 4 == 0 && (yr % 100 != 0 || yr % 400 == 0) {
            366
        } else {
            365
        };
    }
    let month_days = [
        31,
        if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            29
        } else {
            28
        },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    for m in 0..(mo - 1) as usize {
        days += month_days.get(m).copied().unwrap_or(30) as i64;
    }
    days += d - 1;

    Some((days * 86400 + h * 3600 + mi * 60 + se) * 1000)
}

// ─── Schema Registry (delegates to DB) ──────────────────

/// Create a `SchemaRegistryRepository` backed by the given database.
pub fn schema_registry_repo(db: Db) -> Arc<dyn SchemaRegistryRepository> {
    Arc::new(SchemaRegistryBridge(db))
}

struct SchemaRegistryBridge(Db);

impl SchemaRegistryRepository for SchemaRegistryBridge {
    fn list_registry(
        &self,
        _instance_id: &str,
        after_id: &str,
        type_filter: Option<&str>,
        limit: i64,
    ) -> BoxFuture<'_, anyhow::Result<Vec<SchemaRegistryEntry>>> {
        let db = self.0.clone();
        let after = after_id.to_string();
        let tf = type_filter.map(|s| s.to_string());
        Box::pin(async move {
            let rows = zitadel_db::list_schema_registry(&db, &after, tf.as_deref(), limit).await?;
            Ok(rows
                .into_iter()
                .map(|r| SchemaRegistryEntry {
                    id: r.id,
                    type_name: r.type_,
                    version: r.version,
                    visibility: r.visibility,
                    is_default: r.is_default,
                    schema_json: r.schema_json,
                })
                .collect())
        })
    }
}
