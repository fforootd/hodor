use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use anyhow::Context;
use async_recursion::async_recursion;
use google_cloud_spanner::{
    client::Error as SpannerError, row::Row as SpannerRow, statement::Statement,
};
use sqlx::Row;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tokio::sync::RwLock;
use uuid::Uuid;
use zitadel_db::{
    Db, Dialect, DEFAULT_INSTANCE_ID, list_active_child_instance_ownerships,
    list_active_org_role_memberships, list_active_role_bindings_for_scope, list_role_assignments,
};
use zitadel_app::repo::RoleAssignmentFilter;
use zitadel_authz::relation_name_for_role;

use crate::core_model::*;
use crate::dto::*;
use crate::error::FgaError;
use crate::evaluation::*;
use crate::traits::*;
use crate::{CORE_MODEL_VERSION, LIST_SCAN_FALLBACK_LIMIT, PLATFORM_STORE_ID};

#[derive(Clone)]
pub struct FgaService {
    pub(crate) db: Db,
    pub(crate) max_depth: usize,
    pub(crate) store_cache: Arc<RwLock<HashMap<String, StoreInfo>>>,
    pub(crate) active_model_cache: Arc<RwLock<HashMap<(String, String), CachedModel>>>,
    pub(crate) explicit_model_cache: Arc<RwLock<HashMap<(String, String, String), CachedModel>>>,
}

impl FgaService {
    pub fn new(db: Db) -> Self {
        Self {
            db,
            max_depth: 32,
            store_cache: Arc::new(RwLock::new(HashMap::new())),
            active_model_cache: Arc::new(RwLock::new(HashMap::new())),
            explicit_model_cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(crate) fn scoped(&self, instance_id: &str) -> zitadel_db::scoped::ScopedDb {
        self.db.scoped(instance_id.to_string())
    }

    pub(crate) fn spanner_db(&self) -> Result<&zitadel_db::SpannerDb, FgaError> {
        self.db
            .spanner()
            .ok_or_else(|| anyhow::anyhow!("FGA Spanner path requires native Spanner db").into())
    }

    pub(crate) async fn spanner_query_one(
        &self,
        stmt: Statement,
        context: &'static str,
    ) -> Result<Option<SpannerRow>, FgaError> {
        let spanner = self.spanner_db()?;
        let mut tx = spanner.client().single().await.context(context)?;
        let mut rows = tx.query(stmt).await.context(context)?;
        rows.next().await.context(context).map_err(Into::into)
    }

    pub(crate) async fn spanner_query_all(
        &self,
        stmt: Statement,
        context: &'static str,
    ) -> Result<Vec<SpannerRow>, FgaError> {
        let spanner = self.spanner_db()?;
        let mut tx = spanner.client().single().await.context(context)?;
        let mut rows = tx.query(stmt).await.context(context)?;
        let mut out = Vec::new();
        while let Some(row) = rows.next().await.context(context)? {
            out.push(row);
        }
        Ok(out)
    }

    pub async fn initialize_platform_store(&self) -> Result<StoreInfo, FgaError> {
        self.initialize_instance(PLATFORM_STORE_ID).await
    }

    pub async fn discover_platform_store(&self) -> Result<StoreInfo, FgaError> {
        self.discover_store(PLATFORM_STORE_ID).await
    }

    pub async fn rebuild_platform_store(&self) -> Result<(), FgaError> {
        let store = self.initialize_platform_store().await?;
        let desired = self
            .desired_platform_tuples(DEFAULT_INSTANCE_ID)
            .await
            .map_err(FgaError::Internal)?;
        let current = self.read_all_store_tuples(PLATFORM_STORE_ID, &store.id).await?;
        self.reconcile_managed_tuple_set(PLATFORM_STORE_ID, &store.id, desired, current)
            .await
    }

    pub async fn reconcile_root_hierarchy(&self, root_instance_id: &str) -> Result<(), FgaError> {
        let _ = root_instance_id;
        self.rebuild_platform_store().await
    }

    pub async fn reconcile_parent_hierarchy(
        &self,
        parent_instance_id: &str,
    ) -> Result<(), FgaError> {
        let _ = parent_instance_id;
        self.rebuild_platform_store().await
    }

    pub async fn root_relation_allowed(
        &self,
        root_instance_id: &str,
        user_id: &str,
        relation: &str,
        object: &str,
    ) -> Result<bool, FgaError> {
        self.parent_relation_allowed(root_instance_id, &format!("user:{user_id}"), relation, object)
            .await
    }

    pub async fn parent_relation_allowed(
        &self,
        parent_instance_id: &str,
        principal_ref: &str,
        relation: &str,
        object: &str,
    ) -> Result<bool, FgaError> {
        let _ = parent_instance_id;
        let store = self.discover_platform_store().await?;
        Ok(self
            .check(
                PLATFORM_STORE_ID,
                &store.id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: principal_ref.to_string(),
                        relation: relation.to_string(),
                        object: object.to_string(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await?
            .allowed)
    }

    /// Batch-check which of `object_ids` the user has the given relation on.
    /// Returns the subset of IDs that the user is allowed to see.
    pub async fn root_batch_filter(
        &self,
        root_instance_id: &str,
        user_id: &str,
        relation: &str,
        object_type: &str,
        object_ids: &[String],
    ) -> Result<Vec<String>, FgaError> {
        self.parent_batch_filter(
            root_instance_id,
            &format!("user:{user_id}"),
            relation,
            object_type,
            object_ids,
        )
        .await
    }

    pub async fn parent_batch_filter(
        &self,
        parent_instance_id: &str,
        principal_ref: &str,
        relation: &str,
        object_type: &str,
        object_ids: &[String],
    ) -> Result<Vec<String>, FgaError> {
        if object_ids.is_empty() {
            return Ok(Vec::new());
        }
        let _ = parent_instance_id;
        let store = self.discover_platform_store().await?;
        let checks: Vec<BatchCheckItem> = object_ids
            .iter()
            .enumerate()
            .map(|(i, id)| BatchCheckItem {
                tuple_key: TupleKey {
                    user: principal_ref.to_string(),
                    relation: relation.to_string(),
                    object: format!("{object_type}:{id}"),
                    condition: None,
                },
                correlation_id: Some(i.to_string()),
            })
            .collect();

        let result = self
            .batch_check(
                PLATFORM_STORE_ID,
                &store.id,
                BatchCheckRequest {
                    checks,
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await?;

        let mut allowed = Vec::new();
        for check_result in &result.results {
            if check_result.allowed {
                if let Some(ref corr) = check_result.correlation_id {
                    if let Ok(i) = corr.parse::<usize>() {
                        if let Some(id) = object_ids.get(i) {
                            allowed.push(id.clone());
                        }
                    }
                }
            }
        }
        Ok(allowed)
    }

    async fn desired_platform_tuples(&self, root_instance_id: &str) -> anyhow::Result<Vec<TupleKey>> {
        let mut tuples = Vec::new();
        let mut visited = HashSet::new();
        let mut parents = vec![root_instance_id.to_string()];
        let mut child_ownerships = Vec::new();

        while let Some(parent_instance_id) = parents.pop() {
            if !visited.insert(parent_instance_id.clone()) {
                continue;
            }
            let children =
                list_active_child_instance_ownerships(&self.db, &parent_instance_id).await?;

            for membership in list_active_org_role_memberships(&self.db, &parent_instance_id).await?
            {
                if !matches!(
                    membership.role.as_str(),
                    "owner" | "admin" | "member" | "viewer"
                ) {
                    continue;
                }
                let legacy_role = membership.role.clone();
                tuples.push(TupleKey {
                    user: format!("user:{}", membership.user_id),
                    relation: legacy_role.clone(),
                    object: format!("org:{}", membership.org_id),
                    condition: None,
                });
                if let Some(relation) = legacy_org_catalog_relation(&legacy_role) {
                    tuples.push(TupleKey {
                        user: format!("user:{}", membership.user_id),
                        relation: relation.to_string(),
                        object: format!("org:{}", membership.org_id),
                        condition: None,
                    });
                }
                if let Some(relation) = legacy_parent_instance_catalog_relation(&legacy_role) {
                    tuples.push(TupleKey {
                        user: format!("user:{}", membership.user_id),
                        relation: relation.to_string(),
                        object: format!("instance:{parent_instance_id}"),
                        condition: None,
                    });
                    for child in &children {
                        if child.owner_org_id != membership.org_id {
                            continue;
                        }
                        tuples.push(TupleKey {
                            user: format!("user:{}", membership.user_id),
                            relation: relation.to_string(),
                            object: format!("instance:{}", child.instance_id),
                            condition: None,
                        });
                    }
                }
            }

            for child in children {
                let object = format!("instance:{}", child.instance_id);
                tuples.push(TupleKey {
                    user: format!("instance:{parent_instance_id}"),
                    relation: "parent".to_string(),
                    object: object.clone(),
                    condition: None,
                });
                tuples.push(TupleKey {
                    user: format!("org:{}#owner", child.owner_org_id),
                    relation: "owner".to_string(),
                    object: object.clone(),
                    condition: None,
                });
                tuples.push(TupleKey {
                    user: format!("org:{}#admin", child.owner_org_id),
                    relation: "admin".to_string(),
                    object: object.clone(),
                    condition: None,
                });
                tuples.push(TupleKey {
                    user: format!("org:{}#viewer", child.owner_org_id),
                    relation: "viewer".to_string(),
                    object,
                    condition: None,
                });
                child_ownerships.push((
                    parent_instance_id.clone(),
                    child.instance_id.clone(),
                    child.owner_org_id.clone(),
                ));
                parents.push(child.instance_id);
            }
        }

        let now = role_assignment_cutoff();
        for assignment in list_role_assignments(
            &self.db,
            &RoleAssignmentFilter {
                include_revoked: false,
                ..Default::default()
            },
        )
        .await? {
            if assignment
                .expires_at
                .as_deref()
                .is_some_and(|expires_at| expires_at <= now.as_str())
            {
                continue;
            }
            tuples.push(TupleKey {
                user: assignment.principal_ref,
                relation: relation_name_for_role(&assignment.role_key),
                object: format!("{}:{}", assignment.scope_kind, assignment.scope_id),
                condition: None,
            });
        }

        for (parent_instance_id, child_instance_id, owner_org_id) in child_ownerships {
            for binding in list_active_role_bindings_for_scope(
                &self.db,
                &parent_instance_id,
                "org",
                &owner_org_id,
                None,
            )
            .await? {
                if let Some(relation) = projected_child_relation(&binding.role_key) {
                    tuples.push(TupleKey {
                        user: binding.principal_ref,
                        relation: relation.to_string(),
                        object: format!("instance:{child_instance_id}"),
                        condition: None,
                    });
                }
            }
        }

        Ok(tuples)
    }

    async fn read_all_store_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
    ) -> Result<Vec<TupleKey>, FgaError> {
        let mut continuation = None;
        let mut tuples = Vec::new();
        loop {
            let response = self
                .read_tuples(
                    instance_id,
                    store_id,
                    ReadRequest {
                        tuple_key: None,
                        page_size: Some(500),
                        continuation_token: continuation.clone(),
                    },
                )
                .await?;
            tuples.extend(response.tuples.into_iter().map(|record| record.key));
            if response.continuation_token.is_none() {
                break;
            }
            continuation = response.continuation_token;
        }
        Ok(tuples)
    }

    async fn reconcile_managed_tuple_set(
        &self,
        instance_id: &str,
        store_id: &str,
        desired: Vec<TupleKey>,
        current: Vec<TupleKey>,
    ) -> Result<(), FgaError> {
        let desired_set = desired.iter().map(tuple_identity).collect::<BTreeSet<String>>();
        let current_set = current.iter().map(tuple_identity).collect::<BTreeSet<String>>();

        let writes = desired
            .into_iter()
            .filter(|tuple| !current_set.contains(&tuple_identity(tuple)))
            .collect::<Vec<_>>();
        let deletes = current
            .into_iter()
            .filter(|tuple| !desired_set.contains(&tuple_identity(tuple)))
            .collect::<Vec<_>>();

        if writes.is_empty() && deletes.is_empty() {
            return Ok(());
        }

        self.write_tuples(
            instance_id,
            store_id,
            WriteRequest {
                writes: TupleKeySet { tuple_keys: writes },
                deletes: TupleKeySet { tuple_keys: deletes },
                authorization_model_id: None,
            },
        )
        .await
    }

    pub(crate) async fn cached_store(&self, instance_id: &str) -> Option<StoreInfo> {
        self.store_cache.read().await.get(instance_id).cloned()
    }

    pub(crate) async fn cache_store(&self, instance_id: &str, store: &StoreInfo) {
        self.store_cache
            .write()
            .await
            .insert(instance_id.to_string(), store.clone());
    }

    pub(crate) async fn cached_active_model(&self, instance_id: &str, store_id: &str) -> Option<CachedModel> {
        self.active_model_cache
            .read()
            .await
            .get(&(instance_id.to_string(), store_id.to_string()))
            .cloned()
    }

    pub(crate) async fn cached_explicit_model(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: &str,
    ) -> Option<CachedModel> {
        self.explicit_model_cache
            .read()
            .await
            .get(&(
                instance_id.to_string(),
                store_id.to_string(),
                model_id.to_string(),
            ))
            .cloned()
    }

    pub(crate) async fn cache_model(
        &self,
        instance_id: &str,
        store_id: &str,
        model: &CachedModel,
        is_active: bool,
    ) {
        self.explicit_model_cache.write().await.insert(
            (
                instance_id.to_string(),
                store_id.to_string(),
                model.model_id.clone(),
            ),
            model.clone(),
        );
        if is_active {
            self.active_model_cache.write().await.insert(
                (instance_id.to_string(), store_id.to_string()),
                model.clone(),
            );
        }
    }

    pub(crate) async fn invalidate_store_cache(&self, instance_id: &str) {
        self.store_cache.write().await.remove(instance_id);
    }

    pub(crate) async fn invalidate_model_caches(&self, instance_id: &str, store_id: &str) {
        self.active_model_cache
            .write()
            .await
            .remove(&(instance_id.to_string(), store_id.to_string()));
        self.explicit_model_cache.write().await.retain(
            |(cached_instance_id, cached_store_id, _), _| {
                cached_instance_id != instance_id || cached_store_id != store_id
            },
        );
    }

    pub(crate) async fn load_store_row(&self, instance_id: &str) -> Result<Option<StoreInfo>, FgaError> {
        if let Some(store) = self.cached_store(instance_id).await {
            return Ok(Some(store));
        }

        let store = match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let row = sqlx::query_as::<_, (String,)>(
                    "SELECT store_id FROM fga_instance_stores WHERE instance_id = $1",
                )
                .bind(scoped.instance_id())
                .fetch_optional(scoped.pool())
                .await
                .context("load instance store")?;

                row.map(|row| StoreInfo {
                    id: row.0,
                    name: format!("zitadel-{instance_id}"),
                })
            }
            Db::Spanner(_) => {
                let mut stmt = Statement::new(
                    "SELECT store_id FROM fga_instance_stores WHERE instance_id = @instance_id LIMIT 1",
                );
                stmt.add_param("instance_id", &instance_id);
                let row: Option<SpannerRow> =
                    self.spanner_query_one(stmt, "load instance store").await?;
                row.map(|row| -> Result<StoreInfo, FgaError> {
                    Ok(StoreInfo {
                        id: row
                            .column_by_name::<String>("store_id")
                            .context("read spanner store_id")?,
                        name: format!("zitadel-{instance_id}"),
                    })
                })
                .transpose()?
            }
        };
        if let Some(store) = &store {
            self.cache_store(instance_id, store).await;
        }
        Ok(store)
    }

    pub(crate) async fn ensure_store_row(&self, instance_id: &str) -> Result<StoreInfo, FgaError> {
        if let Some(store) = self.load_store_row(instance_id).await? {
            return Ok(store);
        }

        let store = match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let store_id = instance_id.to_string();
                let insert = match scoped.dialect() {
                    Dialect::Sqlite => {
                        "INSERT OR IGNORE INTO fga_instance_stores (instance_id, store_id) VALUES ($1, $2)"
                    }
                    Dialect::Postgres => {
                        "INSERT INTO fga_instance_stores (instance_id, store_id) VALUES ($1, $2) ON CONFLICT (instance_id) DO NOTHING"
                    }
                    Dialect::Spanner => unreachable!("native Spanner does not use ScopedDb"),
                };
                sqlx::query(insert)
                    .bind(scoped.instance_id())
                    .bind(&store_id)
                    .execute(scoped.pool())
                    .await
                    .context("insert instance store")?;
                StoreInfo {
                    id: store_id,
                    name: format!("zitadel-{instance_id}"),
                }
            }
            Db::Spanner(spanner) => {
                let instance_id = instance_id.to_string();
                let store_id = instance_id.clone();
                let (_, store) = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let instance_id = instance_id.clone();
                        let store_id = store_id.clone();
                        Box::pin(async move {
                            let mut check = Statement::new(
                                "SELECT store_id FROM fga_instance_stores WHERE instance_id = @instance_id LIMIT 1",
                            );
                            check.add_param("instance_id", &instance_id);
                            let mut rows = tx.query(check).await?;
                            if let Some(row) = rows.next().await? {
                                return Ok::<StoreInfo, SpannerError>(StoreInfo {
                                    id: row.column_by_name::<String>("store_id")?,
                                    name: format!("zitadel-{instance_id}"),
                                });
                            }

                            let mut insert = Statement::new(
                                "INSERT INTO fga_instance_stores (instance_id, store_id) VALUES (@instance_id, @store_id)",
                            );
                            insert.add_param("instance_id", &instance_id);
                            insert.add_param("store_id", &store_id);
                            tx.update(insert).await?;
                            Ok::<StoreInfo, SpannerError>(StoreInfo {
                                id: store_id,
                                name: format!("zitadel-{instance_id}"),
                            })
                        })
                    })
                    .await
                    .context("insert instance store")?;
                store
            }
        };
        self.cache_store(instance_id, &store).await;
        self.ensure_default_model(instance_id, &store.id).await?;

        Ok(store)
    }

    pub(crate) async fn ensure_default_model(
        &self,
        instance_id: &str,
        store_id: &str,
    ) -> Result<(), FgaError> {
        if is_platform_store(store_id) {
            if let Some(fragments) = self
                .load_active_model_fragments(instance_id, store_id)
                .await?
            {
                if fragments.core_model_version == CORE_MODEL_VERSION
                    && fragments.custom_model == "{}"
                    && fragments.module_fragments == "[]"
                {
                    if self
                        .cached_active_model(instance_id, store_id)
                        .await
                        .is_none()
                    {
                        let cached = self
                            .load_model_row_from_db(instance_id, store_id, None)
                            .await?
                            .ok_or_else(|| {
                                FgaError::NotFound("authorization model not found".into())
                            })?;
                        self.cache_model(instance_id, store_id, &cached, true).await;
                    }
                    return Ok(());
                }
            }

            self.persist_model(instance_id, store_id, core_authorization_model())
                .await?;
            return Ok(());
        }

        if self
            .cached_active_model(instance_id, store_id)
            .await
            .is_some_and(|cached| cached.core_model_version == CORE_MODEL_VERSION)
        {
            return Ok(());
        }
        if let Some(fragments) = self
            .load_active_model_fragments(instance_id, store_id)
            .await?
        {
            if fragments.core_model_version == CORE_MODEL_VERSION {
                if self
                    .cached_active_model(instance_id, store_id)
                    .await
                    .is_none()
                {
                    let cached = self
                        .load_model_row_from_db(instance_id, store_id, None)
                        .await?
                        .ok_or_else(|| {
                            FgaError::NotFound("authorization model not found".into())
                        })?;
                    self.cache_model(instance_id, store_id, &cached, true).await;
                }
                return Ok(());
            }

            let rebuilt =
                rebuild_model_from_fragments(&fragments.custom_model, &fragments.module_fragments)?;
            self.persist_model(instance_id, store_id, rebuilt).await?;
            return Ok(());
        }

        self.persist_model(instance_id, store_id, core_authorization_model())
            .await?;
        Ok(())
    }

    pub(crate) async fn load_active_model_fragments(
        &self,
        instance_id: &str,
        store_id: &str,
    ) -> Result<Option<StoredModelFragments>, FgaError> {
        match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let row = sqlx::query_as::<_, (String, String, String)>(
                    "SELECT core_model_version, CAST(custom_model AS TEXT), CAST(module_fragments AS TEXT) \
                     FROM fga_authorization_models \
                     WHERE instance_id = $1 AND store_id = $2 AND is_active = 1 \
                     ORDER BY created_at DESC LIMIT 1",
                )
                .bind(scoped.instance_id())
                .bind(store_id)
                .fetch_optional(scoped.pool())
                .await
                .context("load active fga model fragments")?;
                Ok(
                    row.map(|(core_model_version, custom_model, module_fragments)| {
                        StoredModelFragments {
                            core_model_version,
                            custom_model,
                            module_fragments,
                        }
                    }),
                )
            }
            Db::Spanner(_) => {
                let mut stmt = Statement::new(
                    "SELECT IFNULL(core_model_version, '') AS core_model_version, \
                            IFNULL(custom_model, '{}') AS custom_model, \
                            IFNULL(module_fragments, '[]') AS module_fragments \
                     FROM fga_authorization_models \
                     WHERE instance_id = @instance_id AND store_id = @store_id AND is_active = 1 \
                     ORDER BY created_at DESC LIMIT 1",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("store_id", &store_id);
                let row = self
                    .spanner_query_one(stmt, "load active fga model fragments")
                    .await?;
                Ok(row.map(|row| StoredModelFragments {
                    core_model_version: row
                        .column_by_name::<String>("core_model_version")
                        .unwrap_or_default(),
                    custom_model: row
                        .column_by_name::<String>("custom_model")
                        .unwrap_or_else(|_| "{}".to_string()),
                    module_fragments: row
                        .column_by_name::<String>("module_fragments")
                        .unwrap_or_else(|_| "[]".to_string()),
                }))
            }
        }
    }

    pub(crate) async fn persist_model(
        &self,
        instance_id: &str,
        store_id: &str,
        request: AuthorizationModelWriteRequest,
    ) -> Result<String, FgaError> {
        if !request.conditions.is_empty() {
            return Err(FgaError::Unsupported(
                "conditions are not supported by the embedded v1 server".into(),
            ));
        }

        let compiled = CompiledModel::from_request(&request)?;
        validate_sealed_core(&compiled)?;
        let custom = extract_custom_fragment(&request);
        if is_platform_store(store_id)
            && custom
                .get("type_definitions")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|types| !types.is_empty())
        {
            return Err(FgaError::Forbidden(
                "platform authorization model is sealed and cannot be customized".into(),
            ));
        }
        let model_id = Uuid::now_v7().to_string();
        let raw = serde_json::to_string(&request).context("serialize authorization model")?;

        match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let mut tx = scoped
                    .pool()
                    .begin()
                    .await
                    .context("begin model transaction")?;
                sqlx::query(
                    "UPDATE fga_authorization_models SET is_active = 0 WHERE instance_id = $1 AND store_id = $2",
                )
                .bind(scoped.instance_id())
                .bind(store_id)
                .execute(&mut *tx)
                .await
                .context("deactivate previous models")?;
                sqlx::query(
                    "INSERT INTO fga_authorization_models \
                     (instance_id, store_id, model_id, schema_version, core_model_version, compiled_model, custom_model, module_fragments, is_active) \
                     VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 1)",
                )
                .bind(scoped.instance_id())
                .bind(store_id)
                .bind(&model_id)
                .bind(&compiled.schema_version)
                .bind(CORE_MODEL_VERSION)
                .bind(&raw)
                .bind(custom.to_string())
                .bind("[]")
                .execute(&mut *tx)
                .await
                .context("insert authorization model")?;
                tx.commit().await.context("commit model transaction")?;
            }
            Db::Spanner(spanner) => {
                let instance_id = instance_id.to_string();
                let store_id = store_id.to_string();
                let schema_version = compiled.schema_version.clone();
                let model_id = model_id.clone();
                let raw = raw.clone();
                let custom = custom.to_string();
                let _ = spanner
                    .client()
                    .read_write_transaction(|tx| {
                        let instance_id = instance_id.clone();
                        let store_id = store_id.clone();
                        let schema_version = schema_version.clone();
                        let model_id = model_id.clone();
                        let raw = raw.clone();
                        let custom = custom.clone();
                        Box::pin(async move {
                            let mut deactivate = Statement::new(
                                "UPDATE fga_authorization_models SET is_active = 0 \
                                 WHERE instance_id = @instance_id AND store_id = @store_id",
                            );
                            deactivate.add_param("instance_id", &instance_id);
                            deactivate.add_param("store_id", &store_id);
                            tx.update(deactivate).await?;

                            let mut insert = Statement::new(
                                "INSERT INTO fga_authorization_models \
                                 (instance_id, store_id, model_id, schema_version, core_model_version, compiled_model, custom_model, module_fragments, is_active) \
                                 VALUES \
                                 (@instance_id, @store_id, @model_id, @schema_version, @core_model_version, @compiled_model, @custom_model, @module_fragments, 1)",
                            );
                            insert.add_param("instance_id", &instance_id);
                            insert.add_param("store_id", &store_id);
                            insert.add_param("model_id", &model_id);
                            insert.add_param("schema_version", &schema_version);
                            insert.add_param("core_model_version", &CORE_MODEL_VERSION);
                            insert.add_param("compiled_model", &raw);
                            insert.add_param("custom_model", &custom);
                            insert.add_param("module_fragments", &"[]");
                            tx.update(insert).await?;
                            Ok::<(), SpannerError>(())
                        })
                    })
                    .await
                    .context("commit spanner model transaction")?;
            }
        }

        self.invalidate_model_caches(instance_id, store_id).await;
        let cached = self
            .load_model_row_from_db(instance_id, store_id, Some(&model_id))
            .await?
            .ok_or_else(|| FgaError::NotFound("authorization model not found".into()))?;
        self.cache_model(instance_id, store_id, &cached, true).await;
        Ok(model_id)
    }

    pub(crate) async fn require_store(
        &self,
        instance_id: &str,
        store_id: &str,
    ) -> Result<StoreInfo, FgaError> {
        let store = self
            .load_store_row(instance_id)
            .await?
            .ok_or_else(|| FgaError::NotFound(format!("store {store_id} not found")))?;
        if store.id != store_id {
            return Err(FgaError::NotFound(format!("store {store_id} not found")));
        }
        Ok(store)
    }

    pub(crate) fn build_cached_model(
        &self,
        model_id: String,
        raw: String,
        created_at: String,
        core_model_version: String,
    ) -> Result<CachedModel, FgaError> {
        let request: AuthorizationModelWriteRequest =
            serde_json::from_str(&raw).context("parse compiled authorization model")?;
        let compiled = CompiledModel::from_request(&request)?;
        Ok(CachedModel {
            model_id,
            raw,
            created_at,
            core_model_version,
            compiled: Arc::new(compiled),
        })
    }

    pub(crate) async fn load_model_row_from_db(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> Result<Option<CachedModel>, FgaError> {
        let cached = match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let row = if let Some(model_id) = model_id {
                    sqlx::query_as::<_, (String, String, String, String)>(
                        "SELECT model_id, compiled_model, CAST(created_at AS TEXT), core_model_version \
                         FROM fga_authorization_models \
                         WHERE instance_id = $1 AND store_id = $2 AND model_id = $3 LIMIT 1",
                    )
                    .bind(scoped.instance_id())
                    .bind(store_id)
                    .bind(model_id)
                    .fetch_optional(scoped.pool())
                    .await
                    .context("load fga model by id")?
                } else {
                    sqlx::query_as::<_, (String, String, String, String)>(
                        "SELECT model_id, compiled_model, CAST(created_at AS TEXT), core_model_version \
                         FROM fga_authorization_models \
                         WHERE instance_id = $1 AND store_id = $2 AND is_active = 1 \
                         ORDER BY created_at DESC LIMIT 1",
                    )
                    .bind(scoped.instance_id())
                    .bind(store_id)
                    .fetch_optional(scoped.pool())
                    .await
                    .context("load active fga model")?
                };
                let Some((resolved_model_id, raw, created_at, core_model_version)) = row else {
                    return Ok(None);
                };
                self.build_cached_model(resolved_model_id, raw, created_at, core_model_version)?
            }
            Db::Spanner(_) => {
                let mut stmt = if model_id.is_some() {
                    Statement::new(
                        "SELECT model_id, compiled_model, CAST(created_at AS STRING) AS created_at, \
                                IFNULL(core_model_version, '') AS core_model_version \
                         FROM fga_authorization_models \
                         WHERE instance_id = @instance_id AND store_id = @store_id AND model_id = @model_id LIMIT 1",
                    )
                } else {
                    Statement::new(
                        "SELECT model_id, compiled_model, CAST(created_at AS STRING) AS created_at, \
                                IFNULL(core_model_version, '') AS core_model_version \
                         FROM fga_authorization_models \
                         WHERE instance_id = @instance_id AND store_id = @store_id AND is_active = 1 \
                         ORDER BY created_at DESC LIMIT 1",
                    )
                };
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("store_id", &store_id);
                if let Some(model_id) = model_id {
                    stmt.add_param("model_id", &model_id);
                }
                let row: Option<SpannerRow> = self
                    .spanner_query_one(
                        stmt,
                        if model_id.is_some() {
                            "load fga model by id"
                        } else {
                            "load active fga model"
                        },
                    )
                    .await?;
                let Some(row) = row else {
                    return Ok(None);
                };
                self.build_cached_model(
                    row.column_by_name::<String>("model_id")
                        .context("read spanner model_id")?,
                    row.column_by_name::<String>("compiled_model")
                        .context("read spanner compiled_model")?,
                    row.column_by_name::<String>("created_at")
                        .context("read spanner created_at")?,
                    row.column_by_name::<String>("core_model_version")
                        .unwrap_or_default(),
                )?
            }
        };

        self.cache_model(instance_id, store_id, &cached, model_id.is_none())
            .await;
        Ok(Some(cached))
    }

    pub(crate) async fn load_model_row(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> Result<CachedModel, FgaError> {
        let _store = self.require_store(instance_id, store_id).await?;
        if let Some(model_id) = model_id {
            if let Some(cached) = self
                .cached_explicit_model(instance_id, store_id, model_id)
                .await
            {
                return Ok(cached);
            }
        } else {
            if let Some(cached) = self.cached_active_model(instance_id, store_id).await {
                return Ok(cached);
            }
        }

        if let Some(cached) = self
            .load_model_row_from_db(instance_id, store_id, model_id)
            .await?
        {
            return Ok(cached);
        }

        if model_id.is_none() {
            self.ensure_default_model(instance_id, store_id).await?;
            if let Some(cached) = self
                .load_model_row_from_db(instance_id, store_id, None)
                .await?
            {
                return Ok(cached);
            }
        }

        Err(FgaError::NotFound("authorization model not found".into()))
    }

    pub(crate) async fn load_compiled_model(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> Result<(String, Arc<CompiledModel>), FgaError> {
        let model = self.load_model_row(instance_id, store_id, model_id).await?;
        Ok((model.model_id, model.compiled))
    }

    pub(crate) async fn validate_model_id(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> Result<String, FgaError> {
        let (active_id, _) = self
            .load_compiled_model(instance_id, store_id, model_id)
            .await?;
        Ok(active_id)
    }

    pub(crate) async fn load_direct_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        object: &ObjectRef,
        relation: &str,
        contextual: &[ParsedTupleKey],
    ) -> Result<Vec<StoredTuple>, FgaError> {
        let mut tuples: Vec<StoredTuple> = match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let rows = sqlx::query(
                    "SELECT object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user FROM fga_tuples WHERE instance_id = $1 AND store_id = $2 AND object_type = $3 AND object_id = $4 AND relation = $5",
                )
                .bind(scoped.instance_id())
                .bind(store_id)
                .bind(&object.object_type)
                .bind(&object.object_id)
                .bind(relation)
                .fetch_all(scoped.pool())
                .await
                .context("load direct tuples")?;

                rows.into_iter()
                    .map(|row| StoredTuple {
                        user: stored_user_from_parts(
                            &row.get::<String, _>("user_type"),
                            &row.get::<String, _>("user_id"),
                            &row.get::<String, _>("user_relation"),
                        ),
                        raw_user: row.get::<String, _>("raw_user"),
                    })
                    .collect()
            }
            Db::Spanner(_) => {
                let mut stmt = Statement::new(
                    "SELECT user_type, user_id, user_relation, raw_user \
                     FROM fga_tuples \
                     WHERE instance_id = @instance_id AND store_id = @store_id \
                       AND object_type = @object_type AND object_id = @object_id AND relation = @relation",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("store_id", &store_id);
                stmt.add_param("object_type", &object.object_type);
                stmt.add_param("object_id", &object.object_id);
                stmt.add_param("relation", &relation);
                let rows: Vec<SpannerRow> =
                    self.spanner_query_all(stmt, "load direct tuples").await?;
                rows.into_iter()
                    .map(|row| -> Result<StoredTuple, FgaError> {
                        Ok(StoredTuple {
                            user: stored_user_from_parts(
                                &row.column_by_name::<String>("user_type")
                                    .context("read spanner user_type")?,
                                &row.column_by_name::<String>("user_id")
                                    .context("read spanner user_id")?,
                                &row.column_by_name::<String>("user_relation")
                                    .context("read spanner user_relation")?,
                            ),
                            raw_user: row
                                .column_by_name::<String>("raw_user")
                                .context("read spanner raw_user")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?
            }
        };

        tuples.extend(
            contextual
                .iter()
                .filter(|tuple| tuple.object == *object && tuple.relation == relation)
                .map(|tuple| StoredTuple {
                    user: tuple.user.clone(),
                    raw_user: tuple.user.as_raw(),
                }),
        );
        Ok(tuples)
    }

    pub(crate) async fn direct_objects_for_subject(
        &self,
        instance_id: &str,
        store_id: &str,
        object_type: &str,
        relation: &str,
        subject: &UserRef,
        contextual: &[ParsedTupleKey],
    ) -> Result<Vec<ObjectRef>, FgaError> {
        let mut set = BTreeSet::new();
        match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let rows = sqlx::query(
                    "SELECT DISTINCT object_type, object_id \
                     FROM fga_tuples \
                     WHERE instance_id = $1 AND store_id = $2 AND object_type = $3 AND relation = $4 \
                       AND user_type = $5 AND user_id = $6 AND user_relation = $7 \
                     ORDER BY object_id",
                )
                .bind(scoped.instance_id())
                .bind(store_id)
                .bind(object_type)
                .bind(relation)
                .bind(subject.user_type())
                .bind(subject.user_id())
                .bind(subject.relation_name().unwrap_or_default())
                .fetch_all(scoped.pool())
                .await
                .context("load direct object candidates by subject")?;
                for row in rows {
                    set.insert(ObjectRef {
                        object_type: row.get::<String, _>("object_type"),
                        object_id: row.get::<String, _>("object_id"),
                    });
                }
            }
            Db::Spanner(_) => {
                let mut stmt = Statement::new(
                    "SELECT DISTINCT object_type, object_id \
                     FROM fga_tuples \
                     WHERE instance_id = @instance_id AND store_id = @store_id \
                       AND object_type = @object_type AND relation = @relation \
                       AND user_type = @user_type AND user_id = @user_id AND user_relation = @user_relation \
                     ORDER BY object_id",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("store_id", &store_id);
                stmt.add_param("object_type", &object_type);
                stmt.add_param("relation", &relation);
                stmt.add_param("user_type", &subject.user_type());
                stmt.add_param("user_id", &subject.user_id());
                stmt.add_param(
                    "user_relation",
                    &subject.relation_name().unwrap_or_default(),
                );
                let rows: Vec<SpannerRow> = self
                    .spanner_query_all(stmt, "load direct object candidates by subject")
                    .await?;
                for row in rows {
                    set.insert(ObjectRef {
                        object_type: row
                            .column_by_name::<String>("object_type")
                            .context("read spanner object_type")?,
                        object_id: row
                            .column_by_name::<String>("object_id")
                            .context("read spanner object_id")?,
                    });
                }
            }
        }
        for tuple in contextual {
            if tuple.object.object_type == object_type
                && tuple.relation == relation
                && tuple.user == *subject
            {
                set.insert(tuple.object.clone());
            }
        }
        Ok(set.into_iter().collect())
    }

    pub(crate) async fn direct_objects_for_wildcard(
        &self,
        instance_id: &str,
        store_id: &str,
        object_type: &str,
        relation: &str,
        user_type: &str,
        contextual: &[ParsedTupleKey],
    ) -> Result<Vec<ObjectRef>, FgaError> {
        let mut set = BTreeSet::new();
        match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let rows = sqlx::query(
                    "SELECT DISTINCT object_type, object_id \
                     FROM fga_tuples \
                     WHERE instance_id = $1 AND store_id = $2 AND object_type = $3 AND relation = $4 \
                       AND user_type = $5 AND user_id = '*' AND user_relation = '' \
                     ORDER BY object_id",
                )
                .bind(scoped.instance_id())
                .bind(store_id)
                .bind(object_type)
                .bind(relation)
                .bind(user_type)
                .fetch_all(scoped.pool())
                .await
                .context("load direct object candidates by wildcard")?;
                for row in rows {
                    set.insert(ObjectRef {
                        object_type: row.get::<String, _>("object_type"),
                        object_id: row.get::<String, _>("object_id"),
                    });
                }
            }
            Db::Spanner(_) => {
                let mut stmt = Statement::new(
                    "SELECT DISTINCT object_type, object_id \
                     FROM fga_tuples \
                     WHERE instance_id = @instance_id AND store_id = @store_id \
                       AND object_type = @object_type AND relation = @relation \
                       AND user_type = @user_type AND user_id = '*' AND user_relation = '' \
                     ORDER BY object_id",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("store_id", &store_id);
                stmt.add_param("object_type", &object_type);
                stmt.add_param("relation", &relation);
                stmt.add_param("user_type", &user_type);
                let rows: Vec<SpannerRow> = self
                    .spanner_query_all(stmt, "load direct object candidates by wildcard")
                    .await?;
                for row in rows {
                    set.insert(ObjectRef {
                        object_type: row
                            .column_by_name::<String>("object_type")
                            .context("read spanner object_type")?,
                        object_id: row
                            .column_by_name::<String>("object_id")
                            .context("read spanner object_id")?,
                    });
                }
            }
        }
        for tuple in contextual {
            if tuple.object.object_type == object_type
                && tuple.relation == relation
                && matches!(
                    &tuple.user,
                    UserRef::Wildcard { object_type: wildcard_type } if wildcard_type == user_type
                )
            {
                set.insert(tuple.object.clone());
            }
        }
        Ok(set.into_iter().collect())
    }

    #[async_recursion]
    pub(crate) async fn planned_object_candidates(
        &self,
        instance_id: &str,
        store_id: &str,
        model: &CompiledModel,
        object_type: &str,
        relation: &str,
        user: &UserRef,
        contextual: &[ParsedTupleKey],
        visiting: &mut HashSet<(String, String, String)>,
    ) -> Result<Option<Vec<ObjectRef>>, FgaError> {
        let visit_key = (user.as_raw(), object_type.to_string(), relation.to_string());
        if !visiting.insert(visit_key.clone()) {
            return Ok(None);
        }
        let result = match model.list_plan(object_type, relation)? {
            ListPlan::Planned { sources } => {
                let mut set = BTreeSet::new();
                for source in sources {
                    let Some(candidates) = self
                        .resolve_object_candidate_source(
                            instance_id,
                            store_id,
                            model,
                            object_type,
                            relation,
                            user,
                            &source,
                            contextual,
                            visiting,
                        )
                        .await?
                    else {
                        visiting.remove(&visit_key);
                        return Ok(None);
                    };
                    set.extend(candidates);
                }
                Some(set.into_iter().collect())
            }
            ListPlan::ScanFallback => None,
        };
        visiting.remove(&visit_key);
        Ok(result)
    }

    #[async_recursion]
    async fn resolve_object_candidate_source(
        &self,
        instance_id: &str,
        store_id: &str,
        model: &CompiledModel,
        object_type: &str,
        relation: &str,
        user: &UserRef,
        source: &CandidateSource,
        contextual: &[ParsedTupleKey],
        visiting: &mut HashSet<(String, String, String)>,
    ) -> Result<Option<Vec<ObjectRef>>, FgaError> {
        match source {
            CandidateSource::Direct => {
                let semantics = model.relation_semantics(object_type, relation)?;
                if semantics.allowed_direct_users.is_empty() {
                    return Ok(None);
                }
                let mut set = BTreeSet::new();
                set.extend(
                    self.direct_objects_for_subject(
                        instance_id,
                        store_id,
                        object_type,
                        relation,
                        user,
                        contextual,
                    )
                    .await?,
                );
                if !matches!(user, UserRef::Wildcard { .. })
                    && semantics
                        .allowed_direct_users
                        .iter()
                        .any(|allowed| allowed.wildcard && allowed.user_type == user.user_type())
                {
                    set.extend(
                        self.direct_objects_for_wildcard(
                            instance_id,
                            store_id,
                            object_type,
                            relation,
                            user.user_type(),
                            contextual,
                        )
                        .await?,
                    );
                }
                for allowed in &semantics.allowed_direct_users {
                    if !allowed.is_userset() {
                        continue;
                    }
                    let user_relation = allowed.relation.as_deref().unwrap_or_default();
                    let Some(intermediates) = self
                        .planned_object_candidates(
                            instance_id,
                            store_id,
                            model,
                            &allowed.user_type,
                            user_relation,
                            user,
                            contextual,
                            visiting,
                        )
                        .await?
                    else {
                        return Ok(None);
                    };
                    for intermediate in intermediates {
                        set.extend(
                            self.direct_objects_for_subject(
                                instance_id,
                                store_id,
                                object_type,
                                relation,
                                &UserRef::Userset {
                                    object: intermediate,
                                    relation: user_relation.to_string(),
                                },
                                contextual,
                            )
                            .await?,
                        );
                    }
                }
                Ok(Some(set.into_iter().collect()))
            }
            CandidateSource::ComputedUserset { relation: computed } => {
                self.planned_object_candidates(
                    instance_id,
                    store_id,
                    model,
                    object_type,
                    computed,
                    user,
                    contextual,
                    visiting,
                )
                .await
            }
            CandidateSource::TupleToUserset {
                tupleset,
                computed_userset,
            } => {
                let semantics = model.relation_semantics(object_type, tupleset)?;
                if semantics.allowed_direct_users.is_empty()
                    || semantics
                        .allowed_direct_users
                        .iter()
                        .any(|allowed| allowed.wildcard || allowed.is_userset())
                {
                    return Ok(None);
                }
                let mut set = BTreeSet::new();
                for allowed in &semantics.allowed_direct_users {
                    let Some(intermediates) = self
                        .planned_object_candidates(
                            instance_id,
                            store_id,
                            model,
                            &allowed.user_type,
                            computed_userset,
                            user,
                            contextual,
                            visiting,
                        )
                        .await?
                    else {
                        return Ok(None);
                    };
                    for intermediate in intermediates {
                        set.extend(
                            self.direct_objects_for_subject(
                                instance_id,
                                store_id,
                                object_type,
                                tupleset,
                                &UserRef::Object(intermediate),
                                contextual,
                            )
                            .await?,
                        );
                    }
                }
                Ok(Some(set.into_iter().collect()))
            }
        }
    }

    #[async_recursion]
    pub(crate) async fn planned_user_candidates(
        &self,
        instance_id: &str,
        store_id: &str,
        model: &CompiledModel,
        object: &ObjectRef,
        relation: &str,
        filter: &UserFilter,
        contextual: &[ParsedTupleKey],
        visiting: &mut HashSet<(String, String, String)>,
    ) -> Result<Option<Vec<UserRef>>, FgaError> {
        let visit_key = (
            object.as_raw(),
            relation.to_string(),
            format!(
                "{}#{}",
                filter.user_type,
                filter.relation.as_deref().unwrap_or_default()
            ),
        );
        if !visiting.insert(visit_key.clone()) {
            return Ok(None);
        }
        let result = match model.list_plan(&object.object_type, relation)? {
            ListPlan::Planned { sources } => {
                let mut set = BTreeSet::new();
                for source in sources {
                    let Some(candidates) = self
                        .resolve_user_candidate_source(
                            instance_id,
                            store_id,
                            model,
                            object,
                            relation,
                            filter,
                            &source,
                            contextual,
                            visiting,
                        )
                        .await?
                    else {
                        visiting.remove(&visit_key);
                        return Ok(None);
                    };
                    set.extend(candidates);
                }
                Some(set.into_iter().collect())
            }
            ListPlan::ScanFallback => None,
        };
        visiting.remove(&visit_key);
        Ok(result)
    }

    #[async_recursion]
    async fn resolve_user_candidate_source(
        &self,
        instance_id: &str,
        store_id: &str,
        model: &CompiledModel,
        object: &ObjectRef,
        relation: &str,
        filter: &UserFilter,
        source: &CandidateSource,
        contextual: &[ParsedTupleKey],
        visiting: &mut HashSet<(String, String, String)>,
    ) -> Result<Option<Vec<UserRef>>, FgaError> {
        match source {
            CandidateSource::Direct => {
                let mut set = BTreeSet::new();
                for tuple in self
                    .load_direct_tuples(instance_id, store_id, object, relation, contextual)
                    .await?
                {
                    if user_matches_filter(&tuple.user, filter) {
                        set.insert(tuple.user.clone());
                    }
                    if let UserRef::Userset {
                        object: nested_object,
                        relation: nested_relation,
                    } = &tuple.user
                    {
                        let Some(nested) = self
                            .planned_user_candidates(
                                instance_id,
                                store_id,
                                model,
                                nested_object,
                                nested_relation,
                                filter,
                                contextual,
                                visiting,
                            )
                            .await?
                        else {
                            return Ok(None);
                        };
                        set.extend(nested);
                    }
                }
                Ok(Some(set.into_iter().collect()))
            }
            CandidateSource::ComputedUserset { relation: computed } => {
                self.planned_user_candidates(
                    instance_id,
                    store_id,
                    model,
                    object,
                    computed,
                    filter,
                    contextual,
                    visiting,
                )
                .await
            }
            CandidateSource::TupleToUserset {
                tupleset,
                computed_userset,
            } => {
                let mut set = BTreeSet::new();
                for tuple in self
                    .load_direct_tuples(instance_id, store_id, object, tupleset, contextual)
                    .await?
                {
                    let UserRef::Object(target) = tuple.user else {
                        return Ok(None);
                    };
                    let Some(nested) = self
                        .planned_user_candidates(
                            instance_id,
                            store_id,
                            model,
                            &target,
                            computed_userset,
                            filter,
                            contextual,
                            visiting,
                        )
                        .await?
                    else {
                        return Ok(None);
                    };
                    set.extend(nested);
                }
                Ok(Some(set.into_iter().collect()))
            }
        }
    }

    pub(crate) async fn scan_candidate_objects(
        &self,
        instance_id: &str,
        store_id: &str,
        object_type: &str,
        contextual: &[ParsedTupleKey],
    ) -> Result<Vec<ObjectRef>, FgaError> {
        let mut set = BTreeSet::new();
        let limit = LIST_SCAN_FALLBACK_LIMIT as i64 + 1;
        match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let rows = sqlx::query(
                    "SELECT DISTINCT object_type, object_id FROM fga_tuples \
                     WHERE instance_id = $1 AND store_id = $2 AND object_type = $3 \
                     ORDER BY object_id LIMIT $4",
                )
                .bind(scoped.instance_id())
                .bind(store_id)
                .bind(object_type)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await
                .context("load scanned candidate objects")?;
                for row in rows {
                    set.insert(ObjectRef {
                        object_type: row.get::<String, _>("object_type"),
                        object_id: row.get::<String, _>("object_id"),
                    });
                }
            }
            Db::Spanner(_) => {
                let mut stmt = Statement::new(
                    "SELECT DISTINCT object_type, object_id FROM fga_tuples \
                     WHERE instance_id = @instance_id AND store_id = @store_id AND object_type = @object_type \
                     ORDER BY object_id LIMIT @limit",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("store_id", &store_id);
                stmt.add_param("object_type", &object_type);
                stmt.add_param("limit", &limit);
                let rows: Vec<SpannerRow> = self
                    .spanner_query_all(stmt, "load scanned candidate objects")
                    .await?;
                for row in rows {
                    set.insert(ObjectRef {
                        object_type: row
                            .column_by_name::<String>("object_type")
                            .context("read spanner object_type")?,
                        object_id: row
                            .column_by_name::<String>("object_id")
                            .context("read spanner object_id")?,
                    });
                }
            }
        }
        for tuple in contextual {
            if tuple.object.object_type == object_type {
                set.insert(tuple.object.clone());
            }
        }
        if set.len() > LIST_SCAN_FALLBACK_LIMIT {
            return Err(FgaError::Unsupported(
                "list operation exceeds embedded planner budget".into(),
            ));
        }
        Ok(set.into_iter().collect())
    }

    pub(crate) async fn scan_candidate_users(
        &self,
        instance_id: &str,
        store_id: &str,
        filter: &UserFilter,
        contextual: &[ParsedTupleKey],
    ) -> Result<Vec<UserRef>, FgaError> {
        let mut set = BTreeSet::new();
        let limit = LIST_SCAN_FALLBACK_LIMIT as i64 + 1;
        match &self.db {
            Db::Sql(_) => {
                let scoped = self.scoped(instance_id);
                let rows = sqlx::query(
                    "SELECT DISTINCT user_type, user_id, user_relation FROM fga_tuples \
                     WHERE instance_id = $1 AND store_id = $2 AND user_type = $3 \
                     ORDER BY user_id LIMIT $4",
                )
                .bind(scoped.instance_id())
                .bind(store_id)
                .bind(&filter.user_type)
                .bind(limit)
                .fetch_all(scoped.pool())
                .await
                .context("load scanned candidate users")?;
                for row in rows {
                    let user = stored_user_from_parts(
                        &row.get::<String, _>("user_type"),
                        &row.get::<String, _>("user_id"),
                        &row.get::<String, _>("user_relation"),
                    );
                    if user_matches_filter(&user, filter) {
                        set.insert(user);
                    }
                }
            }
            Db::Spanner(_) => {
                let mut stmt = Statement::new(
                    "SELECT DISTINCT user_type, user_id, user_relation FROM fga_tuples \
                     WHERE instance_id = @instance_id AND store_id = @store_id AND user_type = @user_type \
                     ORDER BY user_id LIMIT @limit",
                );
                stmt.add_param("instance_id", &instance_id);
                stmt.add_param("store_id", &store_id);
                stmt.add_param("user_type", &filter.user_type);
                stmt.add_param("limit", &limit);
                let rows: Vec<SpannerRow> = self
                    .spanner_query_all(stmt, "load scanned candidate users")
                    .await?;
                for row in rows {
                    let user = stored_user_from_parts(
                        &row.column_by_name::<String>("user_type")
                            .context("read spanner user_type")?,
                        &row.column_by_name::<String>("user_id")
                            .context("read spanner user_id")?,
                        &row.column_by_name::<String>("user_relation")
                            .context("read spanner user_relation")?,
                    );
                    if user_matches_filter(&user, filter) {
                        set.insert(user);
                    }
                }
            }
        }

        for tuple in contextual {
            if user_matches_filter(&tuple.user, filter) {
                set.insert(tuple.user.clone());
            }
        }

        if set.len() > LIST_SCAN_FALLBACK_LIMIT {
            return Err(FgaError::Unsupported(
                "list operation exceeds embedded planner budget".into(),
            ));
        }

        Ok(set.into_iter().collect())
    }

    pub(crate) fn evaluate_internal<'a>(
        &'a self,
        instance_id: &'a str,
        store_id: &'a str,
        model: &'a CompiledModel,
        contextual: &'a [ParsedTupleKey],
    ) -> EvaluatorContext<'a> {
        EvaluatorContext {
            service: self,
            instance_id,
            store_id,
            model,
            contextual,
            tuple_cache: HashMap::new(),
            decision_cache: HashMap::new(),
            active: HashSet::new(),
            max_depth: self.max_depth,
            request_issue: None,
        }
    }
}

fn legacy_org_catalog_relation(role: &str) -> Option<&'static str> {
    match role {
        "owner" | "admin" => Some("org_owner"),
        "viewer" => Some("org_owner_viewer"),
        _ => None,
    }
}

fn legacy_parent_instance_catalog_relation(role: &str) -> Option<&'static str> {
    match role {
        "owner" | "admin" => Some("iam_owner"),
        "viewer" => Some("iam_owner_viewer"),
        _ => None,
    }
}

fn projected_child_relation(role_key: &str) -> Option<&'static str> {
    match role_key {
        "ORG_OWNER" => Some("iam_owner"),
        "ORG_OWNER_VIEWER" => Some("iam_owner_viewer"),
        "ORG_USER_MANAGER" => Some("iam_user_manager"),
        "ORG_ADMIN_IMPERSONATOR" => Some("iam_admin_impersonator"),
        "ORG_END_USER_IMPERSONATOR" => Some("iam_end_user_impersonator"),
        _ => None,
    }
}

fn is_platform_store(store_id: &str) -> bool {
    store_id == PLATFORM_STORE_ID
}

fn role_assignment_cutoff() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}
