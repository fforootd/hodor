use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;

use anyhow::Context;
use async_recursion::async_recursion;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sqlx::Row;
use uuid::Uuid;
use zitadel_db::{Db, Dialect};

pub const SCHEMA_VERSION_1_1: &str = "1.1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StoreInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorizationModelWriteRequest {
    pub schema_version: String,
    #[serde(default)]
    pub type_definitions: Vec<TypeDefinition>,
    #[serde(default)]
    pub conditions: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorizationModelMetadata {
    pub authorization_model_id: String,
    pub schema_version: String,
    pub type_definitions: Vec<TypeDefinition>,
    #[serde(default)]
    pub conditions: Map<String, Value>,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorizationModelWriteResponse {
    pub authorization_model_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthorizationModelsListResponse {
    pub authorization_models: Vec<AuthorizationModelMetadata>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypeDefinition {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub relations: Map<String, Value>,
    #[serde(default)]
    pub metadata: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelationshipCondition {
    pub name: String,
    #[serde(default, skip_serializing_if = "Map::is_empty")]
    pub context: Map<String, Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TupleKey {
    pub user: String,
    pub relation: String,
    pub object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<RelationshipCondition>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContextualTuples {
    #[serde(default)]
    pub tuple_keys: Vec<TupleKey>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckRequest {
    pub tuple_key: TupleKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_tuples: Option<ContextualTuples>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckResponse {
    pub allowed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchCheckItem {
    pub tuple_key: TupleKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchCheckRequest {
    #[serde(default)]
    pub checks: Vec<BatchCheckItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_tuples: Option<ContextualTuples>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchCheckResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    pub allowed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BatchCheckResponse {
    #[serde(default)]
    pub results: Vec<BatchCheckResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tuple_key: Option<TupleFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TupleFilter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TupleRecord {
    pub key: TupleKey,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadResponse {
    #[serde(default)]
    pub tuples: Vec<TupleRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation_token: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TupleKeySet {
    #[serde(default)]
    pub tuple_keys: Vec<TupleKey>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WriteRequest {
    #[serde(default)]
    pub writes: TupleKeySet,
    #[serde(default)]
    pub deletes: TupleKeySet,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_model_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExpandRequest {
    pub object: String,
    pub relation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_tuples: Option<ContextualTuples>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExpandNode {
    pub name: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ExpandNode>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExpandResponse {
    pub tree: ExpandNode,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListObjectsRequest {
    pub user: String,
    pub relation: String,
    #[serde(rename = "type")]
    pub object_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_tuples: Option<ContextualTuples>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListObjectsResponse {
    #[serde(default)]
    pub objects: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserFilter {
    #[serde(rename = "type")]
    pub user_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relation: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListUsersRequest {
    pub object: String,
    pub relation: String,
    #[serde(default)]
    pub user_filters: Vec<UserFilter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization_model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_tuples: Option<ContextualTuples>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListUsersResponse {
    #[serde(default)]
    pub users: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadChangesResponse {
    #[serde(default)]
    pub changes: Vec<TupleChangeRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continuation_token: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TupleChangeRecord {
    pub tuple_key: TupleKey,
    pub operation: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyModelResponse {
    pub schema_version: String,
    #[serde(default)]
    pub types: Vec<LegacyModelType>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyModelType {
    #[serde(rename = "type")]
    pub type_name: String,
    #[serde(default)]
    pub relations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelGraphResponse {
    #[serde(default)]
    pub nodes: Vec<ModelGraphNode>,
    #[serde(default)]
    pub edges: Vec<ModelGraphEdge>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelGraphNode {
    pub id: String,
    #[serde(default)]
    pub relations: Vec<String>,
    #[serde(default)]
    pub permissions: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelGraphEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
    pub kind: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyCheckResponse {
    pub allowed: bool,
    pub user: String,
    pub relation: String,
    pub object: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyTupleListResponse {
    #[serde(default)]
    pub tuples: Vec<TupleKey>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyWriteResponse {
    pub status: String,
    pub written: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LegacyDeleteResponse {
    pub status: String,
    pub deleted: usize,
}

#[derive(Debug)]
pub enum FgaError {
    BadRequest(String),
    NotFound(String),
    Forbidden(String),
    Unsupported(String),
    Internal(anyhow::Error),
}

impl fmt::Display for FgaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadRequest(msg) => write!(f, "{msg}"),
            Self::NotFound(msg) => write!(f, "{msg}"),
            Self::Forbidden(msg) => write!(f, "{msg}"),
            Self::Unsupported(msg) => write!(f, "{msg}"),
            Self::Internal(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for FgaError {}

impl From<anyhow::Error> for FgaError {
    fn from(value: anyhow::Error) -> Self {
        Self::Internal(value)
    }
}

#[async_trait]
pub trait StoreResolver {
    async fn initialize_instance(&self, instance_id: &str) -> Result<StoreInfo, FgaError>;
    async fn discover_store(&self, instance_id: &str) -> Result<StoreInfo, FgaError>;
}

#[async_trait]
pub trait ModelRepository {
    async fn read_model(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> Result<AuthorizationModelMetadata, FgaError>;
    async fn read_models(
        &self,
        instance_id: &str,
        store_id: &str,
    ) -> Result<AuthorizationModelsListResponse, FgaError>;
    async fn write_model(
        &self,
        instance_id: &str,
        store_id: &str,
        request: AuthorizationModelWriteRequest,
    ) -> Result<AuthorizationModelWriteResponse, FgaError>;
}

#[async_trait]
pub trait TupleRepository {
    async fn read_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ReadRequest,
    ) -> Result<ReadResponse, FgaError>;
    async fn write_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        request: WriteRequest,
    ) -> Result<(), FgaError>;
}

#[async_trait]
pub trait ChangeRepository {
    async fn read_changes(
        &self,
        instance_id: &str,
        store_id: &str,
        object_type: Option<&str>,
        page_size: u32,
        continuation_token: Option<&str>,
    ) -> Result<ReadChangesResponse, FgaError>;
}

#[async_trait]
pub trait Evaluator {
    async fn check(
        &self,
        instance_id: &str,
        store_id: &str,
        request: CheckRequest,
    ) -> Result<CheckResponse, FgaError>;
    async fn batch_check(
        &self,
        instance_id: &str,
        store_id: &str,
        request: BatchCheckRequest,
    ) -> Result<BatchCheckResponse, FgaError>;
    async fn expand(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ExpandRequest,
    ) -> Result<ExpandResponse, FgaError>;
    async fn list_objects(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ListObjectsRequest,
    ) -> Result<ListObjectsResponse, FgaError>;
    async fn list_users(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ListUsersRequest,
    ) -> Result<ListUsersResponse, FgaError>;
}

#[async_trait]
pub trait FgaApi {
    async fn legacy_model(&self, instance_id: &str) -> Result<LegacyModelResponse, FgaError>;
    async fn legacy_model_graph(&self, instance_id: &str) -> Result<ModelGraphResponse, FgaError>;
}

#[derive(Clone)]
pub struct FgaService {
    db: Db,
    max_depth: usize,
}

impl FgaService {
    pub fn new(db: Db) -> Self {
        Self { db, max_depth: 32 }
    }

    fn scoped(&self, instance_id: &str) -> zitadel_db::scoped::ScopedDb {
        self.db.scoped(instance_id.to_string())
    }

    async fn ensure_store_row(&self, instance_id: &str) -> Result<StoreInfo, FgaError> {
        let scoped = self.scoped(instance_id);
        if let Some(row) = sqlx::query_as::<_, (String,)>(
            "SELECT store_id FROM fga_instance_stores WHERE instance_id = $1",
        )
        .bind(scoped.instance_id())
        .fetch_optional(scoped.pool())
        .await
        .context("load instance store")?
        {
            let store = StoreInfo {
                id: row.0,
                name: format!("zitadel-{instance_id}"),
            };
            self.ensure_default_model(instance_id, &store.id).await?;
            return Ok(store);
        }

        let store_id = instance_id.to_string();
        let insert = match scoped.dialect() {
            Dialect::Sqlite => {
                "INSERT OR IGNORE INTO fga_instance_stores (instance_id, store_id) VALUES ($1, $2)"
            }
            Dialect::Postgres => {
                "INSERT INTO fga_instance_stores (instance_id, store_id) VALUES ($1, $2) ON CONFLICT (instance_id) DO NOTHING"
            }
        };
        sqlx::query(insert)
            .bind(scoped.instance_id())
            .bind(&store_id)
            .execute(scoped.pool())
            .await
            .context("insert instance store")?;

        self.ensure_default_model(instance_id, &store_id).await?;

        Ok(StoreInfo {
            id: store_id,
            name: format!("zitadel-{instance_id}"),
        })
    }

    async fn ensure_default_model(
        &self,
        instance_id: &str,
        store_id: &str,
    ) -> Result<(), FgaError> {
        let scoped = self.scoped(instance_id);
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT model_id FROM fga_authorization_models WHERE instance_id = $1 AND store_id = $2 AND is_active = 1 LIMIT 1",
        )
        .bind(scoped.instance_id())
        .bind(store_id)
        .fetch_optional(scoped.pool())
        .await
        .context("load active fga model")?;
        if existing.is_some() {
            return Ok(());
        }

        let default_model = core_authorization_model();
        let raw = serde_json::to_string(&default_model).context("serialize default model")?;
        let custom = json!({
            "schema_version": default_model.schema_version,
            "type_definitions": [],
            "conditions": {}
        });
        let model_id = Uuid::now_v7().to_string();
        sqlx::query(
            "INSERT INTO fga_authorization_models (instance_id, store_id, model_id, schema_version, compiled_model, custom_model, module_fragments, is_active) VALUES ($1, $2, $3, $4, $5, $6, $7, 1)",
        )
        .bind(scoped.instance_id())
        .bind(store_id)
        .bind(&model_id)
        .bind(&default_model.schema_version)
        .bind(&raw)
        .bind(custom.to_string())
        .bind("[]")
        .execute(scoped.pool())
        .await
        .context("insert default fga model")?;

        Ok(())
    }

    async fn require_store(&self, instance_id: &str, store_id: &str) -> Result<(), FgaError> {
        let store = self.ensure_store_row(instance_id).await?;
        if store.id != store_id {
            return Err(FgaError::NotFound(format!("store {store_id} not found")));
        }
        Ok(())
    }

    async fn load_model_row(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> Result<(String, String, String), FgaError> {
        self.require_store(instance_id, store_id).await?;
        let scoped = self.scoped(instance_id);
        let row = if let Some(model_id) = model_id {
            sqlx::query_as::<_, (String, String, String)>(
                "SELECT model_id, compiled_model, CAST(created_at AS TEXT) FROM fga_authorization_models WHERE instance_id = $1 AND store_id = $2 AND model_id = $3 LIMIT 1",
            )
            .bind(scoped.instance_id())
            .bind(store_id)
            .bind(model_id)
            .fetch_optional(scoped.pool())
            .await
            .context("load fga model by id")?
        } else {
            sqlx::query_as::<_, (String, String, String)>(
                "SELECT model_id, compiled_model, CAST(created_at AS TEXT) FROM fga_authorization_models WHERE instance_id = $1 AND store_id = $2 AND is_active = 1 ORDER BY created_at DESC LIMIT 1",
            )
            .bind(scoped.instance_id())
            .bind(store_id)
            .fetch_optional(scoped.pool())
            .await
            .context("load active fga model")?
        };

        row.ok_or_else(|| FgaError::NotFound("authorization model not found".into()))
    }

    async fn load_compiled_model(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> Result<(String, CompiledModel), FgaError> {
        let (model_id, raw, _) = self.load_model_row(instance_id, store_id, model_id).await?;
        let request: AuthorizationModelWriteRequest =
            serde_json::from_str(&raw).context("parse compiled authorization model")?;
        let compiled = CompiledModel::from_request(&request)?;
        Ok((model_id, compiled))
    }

    async fn validate_model_id(
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

    async fn load_direct_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        object: &ObjectRef,
        relation: &str,
        contextual: &[ParsedTupleKey],
    ) -> Result<Vec<StoredTuple>, FgaError> {
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

        let mut tuples: Vec<StoredTuple> = rows
            .into_iter()
            .map(|row| StoredTuple {
                user: stored_user_from_parts(
                    &row.get::<String, _>("user_type"),
                    &row.get::<String, _>("user_id"),
                    &row.get::<String, _>("user_relation"),
                ),
                raw_user: row.get::<String, _>("raw_user"),
            })
            .collect();

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

    async fn candidate_objects(
        &self,
        instance_id: &str,
        store_id: &str,
        object_type: &str,
        contextual: &[ParsedTupleKey],
    ) -> Result<Vec<ObjectRef>, FgaError> {
        let scoped = self.scoped(instance_id);
        let rows = sqlx::query(
            "SELECT DISTINCT object_type, object_id FROM fga_tuples WHERE instance_id = $1 AND store_id = $2 AND object_type = $3 ORDER BY object_id",
        )
        .bind(scoped.instance_id())
        .bind(store_id)
        .bind(object_type)
        .fetch_all(scoped.pool())
        .await
        .context("load candidate objects")?;
        let mut set = BTreeSet::new();
        for row in rows {
            set.insert(ObjectRef {
                object_type: row.get::<String, _>("object_type"),
                object_id: row.get::<String, _>("object_id"),
            });
        }
        for tuple in contextual {
            if tuple.object.object_type == object_type {
                set.insert(tuple.object.clone());
            }
        }
        Ok(set.into_iter().collect())
    }

    async fn candidate_users(
        &self,
        instance_id: &str,
        store_id: &str,
        filter: &UserFilter,
        contextual: &[ParsedTupleKey],
    ) -> Result<Vec<UserRef>, FgaError> {
        let scoped = self.scoped(instance_id);
        let rows = sqlx::query(
            "SELECT DISTINCT user_type, user_id, user_relation FROM fga_tuples WHERE instance_id = $1 AND store_id = $2 AND user_type = $3 ORDER BY user_id",
        )
        .bind(scoped.instance_id())
        .bind(store_id)
        .bind(&filter.user_type)
        .fetch_all(scoped.pool())
        .await
        .context("load candidate users")?;

        let mut set = BTreeSet::new();
        for row in rows {
            let user = stored_user_from_parts(
                &row.get::<String, _>("user_type"),
                &row.get::<String, _>("user_id"),
                &row.get::<String, _>("user_relation"),
            );
            if filter
                .relation
                .as_deref()
                .is_none_or(|rel| user.relation_name() == Some(rel))
            {
                set.insert(user);
            }
        }

        for tuple in contextual {
            if tuple.user.user_type() == filter.user_type
                && filter
                    .relation
                    .as_deref()
                    .is_none_or(|rel| tuple.user.relation_name() == Some(rel))
            {
                set.insert(tuple.user.clone());
            }
        }

        Ok(set.into_iter().collect())
    }

    fn evaluate_internal<'a>(
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
        }
    }
}

#[async_trait]
impl StoreResolver for FgaService {
    async fn initialize_instance(&self, instance_id: &str) -> Result<StoreInfo, FgaError> {
        self.ensure_store_row(instance_id).await
    }

    async fn discover_store(&self, instance_id: &str) -> Result<StoreInfo, FgaError> {
        self.ensure_store_row(instance_id).await
    }
}

#[async_trait]
impl ModelRepository for FgaService {
    async fn read_model(
        &self,
        instance_id: &str,
        store_id: &str,
        model_id: Option<&str>,
    ) -> Result<AuthorizationModelMetadata, FgaError> {
        let (model_id, raw, created_at) =
            self.load_model_row(instance_id, store_id, model_id).await?;
        let request: AuthorizationModelWriteRequest =
            serde_json::from_str(&raw).context("parse authorization model row")?;
        Ok(AuthorizationModelMetadata {
            authorization_model_id: model_id,
            schema_version: request.schema_version,
            type_definitions: request.type_definitions,
            conditions: request.conditions,
            created_at,
        })
    }

    async fn read_models(
        &self,
        instance_id: &str,
        store_id: &str,
    ) -> Result<AuthorizationModelsListResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        let scoped = self.scoped(instance_id);
        let rows = sqlx::query(
            "SELECT model_id, compiled_model, CAST(created_at AS TEXT) AS created_at FROM fga_authorization_models WHERE instance_id = $1 AND store_id = $2 ORDER BY created_at DESC",
        )
        .bind(scoped.instance_id())
        .bind(store_id)
        .fetch_all(scoped.pool())
        .await
        .context("list authorization models")?;

        let mut models = Vec::with_capacity(rows.len());
        for row in rows {
            let raw = row.get::<String, _>("compiled_model");
            let request: AuthorizationModelWriteRequest =
                serde_json::from_str(&raw).context("parse authorization model in list")?;
            models.push(AuthorizationModelMetadata {
                authorization_model_id: row.get::<String, _>("model_id"),
                schema_version: request.schema_version,
                type_definitions: request.type_definitions,
                conditions: request.conditions,
                created_at: row.get::<String, _>("created_at"),
            });
        }

        Ok(AuthorizationModelsListResponse {
            authorization_models: models,
        })
    }

    async fn write_model(
        &self,
        instance_id: &str,
        store_id: &str,
        request: AuthorizationModelWriteRequest,
    ) -> Result<AuthorizationModelWriteResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        if !request.conditions.is_empty() {
            return Err(FgaError::Unsupported(
                "conditions are not supported by the embedded v1 server".into(),
            ));
        }

        let compiled = CompiledModel::from_request(&request)?;
        validate_sealed_core(&compiled)?;
        let model_id = Uuid::now_v7().to_string();
        let raw = serde_json::to_string(&request).context("serialize authorization model")?;
        let custom = extract_custom_fragment(&request);
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
            "INSERT INTO fga_authorization_models (instance_id, store_id, model_id, schema_version, compiled_model, custom_model, module_fragments, is_active) VALUES ($1, $2, $3, $4, $5, $6, $7, 1)",
        )
        .bind(scoped.instance_id())
        .bind(store_id)
        .bind(&model_id)
        .bind(&compiled.schema_version)
        .bind(&raw)
        .bind(custom.to_string())
        .bind("[]")
        .execute(&mut *tx)
        .await
        .context("insert authorization model")?;
        tx.commit().await.context("commit model transaction")?;

        Ok(AuthorizationModelWriteResponse {
            authorization_model_id: model_id,
        })
    }
}

#[async_trait]
impl TupleRepository for FgaService {
    async fn read_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ReadRequest,
    ) -> Result<ReadResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        let scoped = self.scoped(instance_id);
        let page_size = request.page_size.unwrap_or(100).clamp(1, 500) as i64;
        let offset = decode_offset(request.continuation_token.as_deref())?;
        let filter = request.tuple_key.unwrap_or(TupleFilter {
            user: None,
            relation: None,
            object: None,
        });

        let rows = sqlx::query(
            "SELECT raw_user, relation, raw_object, CAST(inserted_at AS TEXT) AS inserted_at
             FROM fga_tuples
             WHERE instance_id = $1
               AND store_id = $2
               AND ($3 = '' OR raw_user = $3)
               AND ($4 = '' OR relation = $4)
               AND ($5 = '' OR raw_object = $5)
             ORDER BY raw_object, relation, raw_user
             LIMIT $6 OFFSET $7",
        )
        .bind(scoped.instance_id())
        .bind(store_id)
        .bind(filter.user.unwrap_or_default())
        .bind(filter.relation.unwrap_or_default())
        .bind(filter.object.unwrap_or_default())
        .bind(page_size)
        .bind(offset)
        .fetch_all(scoped.pool())
        .await
        .context("read tuples")?;

        let tuples: Vec<TupleRecord> = rows
            .iter()
            .map(|row| TupleRecord {
                key: TupleKey {
                    user: row.get::<String, _>("raw_user"),
                    relation: row.get::<String, _>("relation"),
                    object: row.get::<String, _>("raw_object"),
                    condition: None,
                },
                timestamp: Some(row.get::<String, _>("inserted_at")),
            })
            .collect();
        let next = if tuples.len() as i64 == page_size {
            Some((offset + page_size).to_string())
        } else {
            None
        };

        Ok(ReadResponse {
            tuples,
            continuation_token: next,
        })
    }

    async fn write_tuples(
        &self,
        instance_id: &str,
        store_id: &str,
        request: WriteRequest,
    ) -> Result<(), FgaError> {
        self.require_store(instance_id, store_id).await?;
        let model_id = self
            .validate_model_id(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let (_, model) = self
            .load_compiled_model(instance_id, store_id, Some(&model_id))
            .await?;
        validate_duplicate_request_tuples(&request)?;

        let scoped = self.scoped(instance_id);
        let mut tx = scoped
            .pool()
            .begin()
            .await
            .context("begin tuple transaction")?;

        for tuple in request.writes.tuple_keys {
            let parsed = ParsedTupleKey::parse(tuple)?;
            if parsed.condition.is_some() {
                return Err(FgaError::Unsupported(
                    "conditional tuples are not supported by the embedded v1 server".into(),
                ));
            }
            model.validate_tuple(&parsed)?;
            let insert = match scoped.dialect() {
                Dialect::Sqlite => {
                    "INSERT OR IGNORE INTO fga_tuples (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)"
                }
                Dialect::Postgres => {
                    "INSERT INTO fga_tuples (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT DO NOTHING"
                }
            };
            let result = sqlx::query(insert)
                .bind(scoped.instance_id())
                .bind(store_id)
                .bind(&parsed.object.object_type)
                .bind(&parsed.object.object_id)
                .bind(&parsed.relation)
                .bind(parsed.user.user_type())
                .bind(parsed.user.user_id())
                .bind(parsed.user.relation_name().unwrap_or_default())
                .bind(parsed.object.as_raw())
                .bind(parsed.user.as_raw())
                .execute(&mut *tx)
                .await
                .context("insert fga tuple")?;
            if result.rows_affected() > 0 {
                sqlx::query(
                    "INSERT INTO fga_tuple_changes (instance_id, store_id, operation, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user, authorization_model_id) VALUES ($1, $2, 'WRITE', $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(scoped.instance_id())
                .bind(store_id)
                .bind(&parsed.object.object_type)
                .bind(&parsed.object.object_id)
                .bind(&parsed.relation)
                .bind(parsed.user.user_type())
                .bind(parsed.user.user_id())
                .bind(parsed.user.relation_name().unwrap_or_default())
                .bind(parsed.object.as_raw())
                .bind(parsed.user.as_raw())
                .bind(&model_id)
                .execute(&mut *tx)
                .await
                .context("insert tuple change")?;
            }
        }

        for tuple in request.deletes.tuple_keys {
            let parsed = ParsedTupleKey::parse(tuple)?;
            let result = sqlx::query(
                "DELETE FROM fga_tuples WHERE instance_id = $1 AND store_id = $2 AND object_type = $3 AND object_id = $4 AND relation = $5 AND user_type = $6 AND user_id = $7 AND user_relation = $8",
            )
            .bind(scoped.instance_id())
            .bind(store_id)
            .bind(&parsed.object.object_type)
            .bind(&parsed.object.object_id)
            .bind(&parsed.relation)
            .bind(parsed.user.user_type())
            .bind(parsed.user.user_id())
            .bind(parsed.user.relation_name().unwrap_or_default())
            .execute(&mut *tx)
            .await
            .context("delete fga tuple")?;
            if result.rows_affected() > 0 {
                sqlx::query(
                    "INSERT INTO fga_tuple_changes (instance_id, store_id, operation, object_type, object_id, relation, user_type, user_id, user_relation, raw_object, raw_user, authorization_model_id) VALUES ($1, $2, 'DELETE', $3, $4, $5, $6, $7, $8, $9, $10, $11)",
                )
                .bind(scoped.instance_id())
                .bind(store_id)
                .bind(&parsed.object.object_type)
                .bind(&parsed.object.object_id)
                .bind(&parsed.relation)
                .bind(parsed.user.user_type())
                .bind(parsed.user.user_id())
                .bind(parsed.user.relation_name().unwrap_or_default())
                .bind(parsed.object.as_raw())
                .bind(parsed.user.as_raw())
                .bind(&model_id)
                .execute(&mut *tx)
                .await
                .context("insert tuple delete change")?;
            }
        }

        tx.commit().await.context("commit tuple transaction")?;
        Ok(())
    }
}

#[async_trait]
impl ChangeRepository for FgaService {
    async fn read_changes(
        &self,
        instance_id: &str,
        store_id: &str,
        object_type: Option<&str>,
        page_size: u32,
        continuation_token: Option<&str>,
    ) -> Result<ReadChangesResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        let scoped = self.scoped(instance_id);
        let after_seq = decode_offset(continuation_token)?;
        let limit = page_size.clamp(1, 500) as i64;
        let rows = sqlx::query(
            "SELECT seq, operation, raw_user, relation, raw_object, CAST(created_at AS TEXT) AS created_at
             FROM fga_tuple_changes
             WHERE instance_id = $1
               AND store_id = $2
               AND seq > $3
               AND ($4 = '' OR object_type = $4)
             ORDER BY seq ASC
             LIMIT $5",
        )
        .bind(scoped.instance_id())
        .bind(store_id)
        .bind(after_seq)
        .bind(object_type.unwrap_or_default())
        .bind(limit)
        .fetch_all(scoped.pool())
        .await
        .context("read fga changes")?;

        let mut next = None;
        let mut changes = Vec::with_capacity(rows.len());
        for row in rows {
            let seq: i64 = row.get("seq");
            next = Some(seq.to_string());
            changes.push(TupleChangeRecord {
                tuple_key: TupleKey {
                    user: row.get("raw_user"),
                    relation: row.get("relation"),
                    object: row.get("raw_object"),
                    condition: None,
                },
                operation: row.get("operation"),
                timestamp: row.get("created_at"),
            });
        }
        if changes.len() < limit as usize {
            next = None;
        }
        Ok(ReadChangesResponse {
            changes,
            continuation_token: next,
        })
    }
}

#[async_trait]
impl Evaluator for FgaService {
    async fn check(
        &self,
        instance_id: &str,
        store_id: &str,
        request: CheckRequest,
    ) -> Result<CheckResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        if request.context.is_some() {
            return Err(FgaError::Unsupported(
                "request context is not supported by the embedded v1 server".into(),
            ));
        }
        let tuple = ParsedTupleKey::parse(request.tuple_key)?;
        let (_, model) = self
            .load_compiled_model(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let contextual = parse_contextual(request.contextual_tuples)?;
        let mut ctx = self.evaluate_internal(instance_id, store_id, &model, &contextual);
        let allowed = ctx
            .check(&tuple.user, &tuple.relation, &tuple.object, 0)
            .await?;
        Ok(CheckResponse { allowed })
    }

    async fn batch_check(
        &self,
        instance_id: &str,
        store_id: &str,
        request: BatchCheckRequest,
    ) -> Result<BatchCheckResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        if request.context.is_some() {
            return Err(FgaError::Unsupported(
                "request context is not supported by the embedded v1 server".into(),
            ));
        }
        let (_, model) = self
            .load_compiled_model(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let contextual = parse_contextual(request.contextual_tuples)?;
        let mut ctx = self.evaluate_internal(instance_id, store_id, &model, &contextual);
        let mut results = Vec::with_capacity(request.checks.len());
        for item in request.checks {
            let tuple = ParsedTupleKey::parse(item.tuple_key)?;
            let allowed = ctx
                .check(&tuple.user, &tuple.relation, &tuple.object, 0)
                .await?;
            results.push(BatchCheckResult {
                correlation_id: item.correlation_id,
                allowed,
            });
        }
        Ok(BatchCheckResponse { results })
    }

    async fn expand(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ExpandRequest,
    ) -> Result<ExpandResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        let object = ObjectRef::parse(&request.object)?;
        let (_, model) = self
            .load_compiled_model(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let contextual = parse_contextual(request.contextual_tuples)?;
        let mut ctx = self.evaluate_internal(instance_id, store_id, &model, &contextual);
        let tree = ctx.expand(&object, &request.relation, 0).await?;
        Ok(ExpandResponse { tree })
    }

    async fn list_objects(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ListObjectsRequest,
    ) -> Result<ListObjectsResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        let user = UserRef::parse(&request.user)?;
        let (_, model) = self
            .load_compiled_model(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let contextual = parse_contextual(request.contextual_tuples)?;
        let candidates = self
            .candidate_objects(instance_id, store_id, &request.object_type, &contextual)
            .await?;
        let mut ctx = self.evaluate_internal(instance_id, store_id, &model, &contextual);
        let mut objects = Vec::new();
        for object in candidates {
            if ctx.check(&user, &request.relation, &object, 0).await? {
                objects.push(object.as_raw());
            }
        }
        Ok(ListObjectsResponse { objects })
    }

    async fn list_users(
        &self,
        instance_id: &str,
        store_id: &str,
        request: ListUsersRequest,
    ) -> Result<ListUsersResponse, FgaError> {
        self.require_store(instance_id, store_id).await?;
        let object = ObjectRef::parse(&request.object)?;
        let (_, model) = self
            .load_compiled_model(
                instance_id,
                store_id,
                request.authorization_model_id.as_deref(),
            )
            .await?;
        let contextual = parse_contextual(request.contextual_tuples)?;
        let mut ctx = self.evaluate_internal(instance_id, store_id, &model, &contextual);
        let mut users = BTreeSet::new();
        for filter in &request.user_filters {
            for user in self
                .candidate_users(instance_id, store_id, filter, &contextual)
                .await?
            {
                if ctx.check(&user, &request.relation, &object, 0).await? {
                    users.insert(user.as_raw());
                }
            }
        }
        Ok(ListUsersResponse {
            users: users.into_iter().collect(),
        })
    }
}

#[async_trait]
impl FgaApi for FgaService {
    async fn legacy_model(&self, instance_id: &str) -> Result<LegacyModelResponse, FgaError> {
        let store = self.ensure_store_row(instance_id).await?;
        let model = self.read_model(instance_id, &store.id, None).await?;
        Ok(LegacyModelResponse {
            schema_version: model.schema_version,
            types: model
                .type_definitions
                .into_iter()
                .map(|type_def| LegacyModelType {
                    type_name: type_def.type_name,
                    relations: type_def.relations.keys().cloned().collect(),
                })
                .collect(),
        })
    }

    async fn legacy_model_graph(&self, instance_id: &str) -> Result<ModelGraphResponse, FgaError> {
        let store = self.ensure_store_row(instance_id).await?;
        let (_, compiled) = self
            .load_compiled_model(instance_id, &store.id, None)
            .await?;
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        for (type_name, type_def) in &compiled.types {
            let mut permissions = Vec::new();
            for (relation, expr) in &type_def.relations {
                if !matches!(expr, RelationExpr::This) {
                    permissions.push(relation.clone());
                }
                collect_graph_edges(type_name, relation, expr, &mut edges);
            }
            nodes.push(ModelGraphNode {
                id: type_name.clone(),
                relations: type_def.relations.keys().cloned().collect(),
                permissions,
            });
        }
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        edges.sort_by(|a, b| {
            a.from
                .cmp(&b.from)
                .then(a.to.cmp(&b.to))
                .then(a.relation.cmp(&b.relation))
        });
        Ok(ModelGraphResponse { nodes, edges })
    }
}

fn collect_graph_edges(
    type_name: &str,
    relation: &str,
    expr: &RelationExpr,
    edges: &mut Vec<ModelGraphEdge>,
) {
    match expr {
        RelationExpr::This => {}
        RelationExpr::ComputedUserset { relation: target } => {
            edges.push(ModelGraphEdge {
                from: type_name.to_string(),
                to: type_name.to_string(),
                relation: format!("{relation} -> {target}"),
                kind: "computed_userset".into(),
            });
        }
        RelationExpr::TupleToUserset {
            tupleset,
            computed_userset,
        } => edges.push(ModelGraphEdge {
            from: type_name.to_string(),
            to: type_name.to_string(),
            relation: format!("{tupleset}->{computed_userset}"),
            kind: "tuple_to_userset".into(),
        }),
        RelationExpr::Union { children } | RelationExpr::Intersection { children } => {
            for child in children {
                collect_graph_edges(type_name, relation, child, edges);
            }
        }
        RelationExpr::Difference { base, subtract } => {
            collect_graph_edges(type_name, relation, base, edges);
            collect_graph_edges(type_name, relation, subtract, edges);
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct ObjectRef {
    object_type: String,
    object_id: String,
}

impl ObjectRef {
    fn parse(raw: &str) -> Result<Self, FgaError> {
        let Some((object_type, object_id)) = raw.split_once(':') else {
            return Err(FgaError::BadRequest(format!(
                "invalid object reference {raw}"
            )));
        };
        if object_type.is_empty() || object_id.is_empty() {
            return Err(FgaError::BadRequest(format!(
                "invalid object reference {raw}"
            )));
        }
        Ok(Self {
            object_type: object_type.to_string(),
            object_id: object_id.to_string(),
        })
    }

    fn as_raw(&self) -> String {
        format!("{}:{}", self.object_type, self.object_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum UserRef {
    Object(ObjectRef),
    Userset { object: ObjectRef, relation: String },
    Wildcard { object_type: String },
}

impl UserRef {
    fn parse(raw: &str) -> Result<Self, FgaError> {
        if let Some(prefix) = raw.strip_suffix(":*") {
            if prefix.is_empty() {
                return Err(FgaError::BadRequest(format!(
                    "invalid user reference {raw}"
                )));
            }
            return Ok(Self::Wildcard {
                object_type: prefix.to_string(),
            });
        }
        let (base, relation) = match raw.split_once('#') {
            Some((base, relation)) => (base, Some(relation.to_string())),
            None => (raw, None),
        };
        let object = ObjectRef::parse(base)?;
        Ok(match relation {
            Some(relation) => Self::Userset { object, relation },
            None => Self::Object(object),
        })
    }

    fn as_raw(&self) -> String {
        match self {
            Self::Object(object) => object.as_raw(),
            Self::Userset { object, relation } => format!("{}#{relation}", object.as_raw()),
            Self::Wildcard { object_type } => format!("{object_type}:*"),
        }
    }

    fn user_type(&self) -> &str {
        match self {
            Self::Object(object) | Self::Userset { object, .. } => &object.object_type,
            Self::Wildcard { object_type } => object_type,
        }
    }

    fn user_id(&self) -> &str {
        match self {
            Self::Object(object) | Self::Userset { object, .. } => &object.object_id,
            Self::Wildcard { .. } => "*",
        }
    }

    fn relation_name(&self) -> Option<&str> {
        match self {
            Self::Userset { relation, .. } => Some(relation.as_str()),
            _ => None,
        }
    }

    fn matches(&self, candidate: &UserRef) -> bool {
        match self {
            Self::Object(object) => matches!(candidate, Self::Object(other) if other == object),
            Self::Userset { object, relation } => {
                matches!(candidate, Self::Userset { object: other, relation: other_relation } if other == object && other_relation == relation)
            }
            Self::Wildcard { object_type } => candidate.user_type() == object_type,
        }
    }
}

fn stored_user_from_parts(user_type: &str, user_id: &str, user_relation: &str) -> UserRef {
    if user_id == "*" {
        return UserRef::Wildcard {
            object_type: user_type.to_string(),
        };
    }
    let object = ObjectRef {
        object_type: user_type.to_string(),
        object_id: user_id.to_string(),
    };
    if user_relation.is_empty() {
        UserRef::Object(object)
    } else {
        UserRef::Userset {
            object,
            relation: user_relation.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
struct ParsedTupleKey {
    user: UserRef,
    relation: String,
    object: ObjectRef,
    condition: Option<RelationshipCondition>,
}

impl ParsedTupleKey {
    fn parse(tuple: TupleKey) -> Result<Self, FgaError> {
        Ok(Self {
            user: UserRef::parse(&tuple.user)?,
            relation: tuple.relation,
            object: ObjectRef::parse(&tuple.object)?,
            condition: tuple.condition,
        })
    }
}

#[derive(Clone, Debug)]
struct StoredTuple {
    user: UserRef,
    raw_user: String,
}

#[derive(Clone, Debug)]
struct CompiledModel {
    schema_version: String,
    raw_types: HashMap<String, Value>,
    types: HashMap<String, CompiledType>,
}

impl CompiledModel {
    fn from_request(request: &AuthorizationModelWriteRequest) -> Result<Self, FgaError> {
        if request.schema_version != SCHEMA_VERSION_1_1 {
            return Err(FgaError::BadRequest(format!(
                "schema_version {} is not supported",
                request.schema_version
            )));
        }
        if !request.conditions.is_empty() {
            return Err(FgaError::Unsupported(
                "conditions are not supported by the embedded v1 server".into(),
            ));
        }

        let mut types = HashMap::new();
        let mut raw_types = HashMap::new();
        for type_def in &request.type_definitions {
            if raw_types.contains_key(&type_def.type_name) {
                return Err(FgaError::BadRequest(format!(
                    "duplicate type definition {}",
                    type_def.type_name
                )));
            }
            let mut relations = HashMap::new();
            for (relation, expr) in &type_def.relations {
                relations.insert(relation.clone(), parse_relation_expr(expr)?);
            }
            let metadata = parse_relation_metadata(type_def.metadata.as_ref())?;
            raw_types.insert(
                type_def.type_name.clone(),
                serde_json::to_value(type_def).context("serialize type definition")?,
            );
            types.insert(
                type_def.type_name.clone(),
                CompiledType {
                    relations,
                    metadata,
                },
            );
        }

        Ok(Self {
            schema_version: request.schema_version.clone(),
            raw_types,
            types,
        })
    }

    fn relation(&self, object_type: &str, relation: &str) -> Result<&RelationExpr, FgaError> {
        self.types
            .get(object_type)
            .and_then(|type_def| type_def.relations.get(relation))
            .ok_or_else(|| {
                FgaError::BadRequest(format!(
                    "relation {relation} is not defined on type {object_type}"
                ))
            })
    }

    fn validate_tuple(&self, tuple: &ParsedTupleKey) -> Result<(), FgaError> {
        let relation_expr = self.relation(&tuple.object.object_type, &tuple.relation)?;
        if matches!(relation_expr, RelationExpr::This) {
            let metadata = self
                .types
                .get(&tuple.object.object_type)
                .and_then(|type_def| type_def.metadata.get(&tuple.relation));
            if let Some(allowed) = metadata
                && !allowed
                    .iter()
                    .any(|candidate| candidate.matches(&tuple.user))
            {
                return Err(FgaError::BadRequest(format!(
                    "user {} cannot be directly related to {}#{}",
                    tuple.user.as_raw(),
                    tuple.object.as_raw(),
                    tuple.relation
                )));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct CompiledType {
    relations: HashMap<String, RelationExpr>,
    metadata: HashMap<String, Vec<AllowedDirectUser>>,
}

#[derive(Clone, Debug)]
struct AllowedDirectUser {
    user_type: String,
    relation: Option<String>,
    wildcard: bool,
}

impl AllowedDirectUser {
    fn matches(&self, user: &UserRef) -> bool {
        if self.user_type != user.user_type() {
            return false;
        }
        match user {
            UserRef::Object(_) => self.relation.is_none() && !self.wildcard,
            UserRef::Userset { relation, .. } => self.relation.as_deref() == Some(relation),
            UserRef::Wildcard { .. } => self.wildcard,
        }
    }
}

#[derive(Clone, Debug)]
enum RelationExpr {
    This,
    ComputedUserset {
        relation: String,
    },
    TupleToUserset {
        tupleset: String,
        computed_userset: String,
    },
    Union {
        children: Vec<RelationExpr>,
    },
    Intersection {
        children: Vec<RelationExpr>,
    },
    Difference {
        base: Box<RelationExpr>,
        subtract: Box<RelationExpr>,
    },
}

struct EvaluatorContext<'a> {
    service: &'a FgaService,
    instance_id: &'a str,
    store_id: &'a str,
    model: &'a CompiledModel,
    contextual: &'a [ParsedTupleKey],
    tuple_cache: HashMap<(String, String), Vec<StoredTuple>>,
    decision_cache: HashMap<(String, String, String), bool>,
    active: HashSet<(String, String, String)>,
    max_depth: usize,
}

impl<'a> EvaluatorContext<'a> {
    async fn tuples_for(
        &mut self,
        object: &ObjectRef,
        relation: &str,
    ) -> Result<Vec<StoredTuple>, FgaError> {
        let key = (object.as_raw(), relation.to_string());
        if let Some(cached) = self.tuple_cache.get(&key) {
            return Ok(cached.clone());
        }
        let tuples = self
            .service
            .load_direct_tuples(
                self.instance_id,
                self.store_id,
                object,
                relation,
                self.contextual,
            )
            .await?;
        self.tuple_cache.insert(key, tuples.clone());
        Ok(tuples)
    }

    async fn check(
        &mut self,
        user: &UserRef,
        relation: &str,
        object: &ObjectRef,
        depth: usize,
    ) -> Result<bool, FgaError> {
        if depth > self.max_depth {
            return Ok(false);
        }
        let key = (user.as_raw(), relation.to_string(), object.as_raw());
        if let Some(cached) = self.decision_cache.get(&key) {
            return Ok(*cached);
        }
        if !self.active.insert(key.clone()) {
            return Ok(false);
        }
        let expr = self.model.relation(&object.object_type, relation)?.clone();
        let allowed = self.eval_expr(user, relation, object, &expr, depth).await?;
        self.active.remove(&key);
        self.decision_cache.insert(key, allowed);
        Ok(allowed)
    }

    #[async_recursion]
    async fn eval_expr(
        &mut self,
        user: &UserRef,
        relation: &str,
        object: &ObjectRef,
        expr: &RelationExpr,
        depth: usize,
    ) -> Result<bool, FgaError> {
        match expr {
            RelationExpr::This => {
                for tuple in self.tuples_for(object, relation).await? {
                    match &tuple.user {
                        UserRef::Object(_) | UserRef::Wildcard { .. } => {
                            if tuple.user.matches(user) {
                                return Ok(true);
                            }
                        }
                        UserRef::Userset {
                            object: user_object,
                            relation: user_relation,
                        } => {
                            if self
                                .check(user, user_relation, user_object, depth + 1)
                                .await?
                            {
                                return Ok(true);
                            }
                        }
                    }
                }
                Ok(false)
            }
            RelationExpr::ComputedUserset { relation } => {
                self.check(user, relation, object, depth + 1).await
            }
            RelationExpr::TupleToUserset {
                tupleset,
                computed_userset,
            } => {
                for tuple in self.tuples_for(object, tupleset).await? {
                    if let UserRef::Object(target) = &tuple.user
                        && self
                            .check(user, computed_userset, target, depth + 1)
                            .await?
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            RelationExpr::Union { children } => {
                for child in children {
                    if self
                        .eval_expr(user, relation, object, child, depth + 1)
                        .await?
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
            RelationExpr::Intersection { children } => {
                for child in children {
                    if !self
                        .eval_expr(user, relation, object, child, depth + 1)
                        .await?
                    {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            RelationExpr::Difference { base, subtract } => Ok(self
                .eval_expr(user, relation, object, base, depth + 1)
                .await?
                && !self
                    .eval_expr(user, relation, object, subtract, depth + 1)
                    .await?),
        }
    }

    #[async_recursion]
    async fn expand(
        &mut self,
        object: &ObjectRef,
        relation: &str,
        depth: usize,
    ) -> Result<ExpandNode, FgaError> {
        if depth > self.max_depth {
            return Ok(ExpandNode {
                name: format!("{}#{relation}", object.as_raw()),
                children: Vec::new(),
                users: vec!["depth_limit".into()],
            });
        }
        let expr = self.model.relation(&object.object_type, relation)?.clone();
        self.expand_expr(object, relation, &expr, depth).await
    }

    #[async_recursion]
    async fn expand_expr(
        &mut self,
        object: &ObjectRef,
        relation: &str,
        expr: &RelationExpr,
        depth: usize,
    ) -> Result<ExpandNode, FgaError> {
        let name = format!("{}#{relation}", object.as_raw());
        match expr {
            RelationExpr::This => {
                let tuples = self.tuples_for(object, relation).await?;
                let mut children = Vec::new();
                let mut users = Vec::new();
                for tuple in tuples {
                    match &tuple.user {
                        UserRef::Userset {
                            object: user_object,
                            relation: user_relation,
                        } => {
                            children
                                .push(self.expand(user_object, user_relation, depth + 1).await?);
                        }
                        _ => users.push(tuple.raw_user),
                    }
                }
                Ok(ExpandNode {
                    name,
                    children,
                    users,
                })
            }
            RelationExpr::ComputedUserset { relation: target } => Ok(ExpandNode {
                name,
                children: vec![self.expand(object, target, depth + 1).await?],
                users: Vec::new(),
            }),
            RelationExpr::TupleToUserset {
                tupleset,
                computed_userset,
            } => {
                let tuples = self.tuples_for(object, tupleset).await?;
                let mut children = Vec::new();
                for tuple in tuples {
                    if let UserRef::Object(target) = &tuple.user {
                        children.push(self.expand(target, computed_userset, depth + 1).await?);
                    }
                }
                Ok(ExpandNode {
                    name,
                    children,
                    users: Vec::new(),
                })
            }
            RelationExpr::Union { children } => {
                let mut expanded = Vec::new();
                for child in children {
                    expanded.push(self.expand_expr(object, relation, child, depth + 1).await?);
                }
                Ok(ExpandNode {
                    name,
                    children: expanded,
                    users: Vec::new(),
                })
            }
            RelationExpr::Intersection { children } => {
                let mut expanded = Vec::new();
                for child in children {
                    expanded.push(self.expand_expr(object, relation, child, depth + 1).await?);
                }
                Ok(ExpandNode {
                    name,
                    children: expanded,
                    users: Vec::new(),
                })
            }
            RelationExpr::Difference { base, subtract } => Ok(ExpandNode {
                name,
                children: vec![
                    self.expand_expr(object, relation, base, depth + 1).await?,
                    self.expand_expr(object, relation, subtract, depth + 1)
                        .await?,
                ],
                users: Vec::new(),
            }),
        }
    }
}

fn parse_contextual(contextual: Option<ContextualTuples>) -> Result<Vec<ParsedTupleKey>, FgaError> {
    contextual
        .map(|tuples| {
            tuples
                .tuple_keys
                .into_iter()
                .map(ParsedTupleKey::parse)
                .collect()
        })
        .transpose()
        .map(|value| value.unwrap_or_default())
}

fn decode_offset(token: Option<&str>) -> Result<i64, FgaError> {
    match token {
        None | Some("") => Ok(0),
        Some(token) => token
            .parse::<i64>()
            .map_err(|_| FgaError::BadRequest("invalid continuation token".into())),
    }
}

fn validate_duplicate_request_tuples(request: &WriteRequest) -> Result<(), FgaError> {
    let mut seen = HashSet::new();
    for tuple in &request.writes.tuple_keys {
        let key = (&tuple.user, &tuple.relation, &tuple.object);
        if !seen.insert((key.0.clone(), key.1.clone(), key.2.clone(), "write")) {
            return Err(FgaError::BadRequest("duplicate tuple in writes".into()));
        }
    }
    let mut deletes = HashSet::new();
    for tuple in &request.deletes.tuple_keys {
        let key = (&tuple.user, &tuple.relation, &tuple.object);
        if !deletes.insert((key.0.clone(), key.1.clone(), key.2.clone(), "delete")) {
            return Err(FgaError::BadRequest("duplicate tuple in deletes".into()));
        }
        if seen.contains(&(key.0.clone(), key.1.clone(), key.2.clone(), "write")) {
            return Err(FgaError::BadRequest(
                "cannot write and delete the same tuple in one request".into(),
            ));
        }
    }
    Ok(())
}

fn parse_relation_metadata(
    metadata: Option<&Value>,
) -> Result<HashMap<String, Vec<AllowedDirectUser>>, FgaError> {
    let Some(metadata) = metadata else {
        return Ok(HashMap::new());
    };
    let Some(relations) = metadata.get("relations").and_then(Value::as_object) else {
        return Ok(HashMap::new());
    };
    let mut parsed = HashMap::new();
    for (relation, metadata) in relations {
        let mut allowed = Vec::new();
        if let Some(types) = metadata
            .get("directly_related_user_types")
            .and_then(Value::as_array)
        {
            for type_def in types {
                let Some(object) = type_def.as_object() else {
                    continue;
                };
                let user_type = object
                    .get("type")
                    .and_then(Value::as_str)
                    .ok_or_else(|| FgaError::BadRequest("metadata type must be a string".into()))?;
                if object.get("condition").is_some() {
                    return Err(FgaError::Unsupported(
                        "conditional relation metadata is not supported".into(),
                    ));
                }
                allowed.push(AllowedDirectUser {
                    user_type: user_type.to_string(),
                    relation: object
                        .get("relation")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    wildcard: object.get("wildcard").is_some(),
                });
            }
        }
        parsed.insert(relation.clone(), allowed);
    }
    Ok(parsed)
}

fn parse_relation_expr(value: &Value) -> Result<RelationExpr, FgaError> {
    let Some(object) = value.as_object() else {
        return Err(FgaError::BadRequest(
            "relation definition must be an object".into(),
        ));
    };
    if object.contains_key("this") {
        return Ok(RelationExpr::This);
    }
    if let Some(computed) = object.get("computedUserset") {
        let relation = computed
            .get("relation")
            .and_then(Value::as_str)
            .ok_or_else(|| FgaError::BadRequest("computedUserset.relation is required".into()))?;
        return Ok(RelationExpr::ComputedUserset {
            relation: relation.to_string(),
        });
    }
    if let Some(ttu) = object.get("tupleToUserset") {
        let tupleset = ttu
            .get("tupleset")
            .and_then(Value::as_object)
            .and_then(|value| value.get("relation"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                FgaError::BadRequest("tupleToUserset.tupleset.relation is required".into())
            })?;
        let computed_userset = ttu
            .get("computedUserset")
            .and_then(Value::as_object)
            .and_then(|value| value.get("relation"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                FgaError::BadRequest("tupleToUserset.computedUserset.relation is required".into())
            })?;
        return Ok(RelationExpr::TupleToUserset {
            tupleset: tupleset.to_string(),
            computed_userset: computed_userset.to_string(),
        });
    }
    if let Some(union) = object.get("union") {
        let children = union
            .get("child")
            .and_then(Value::as_array)
            .ok_or_else(|| FgaError::BadRequest("union.child must be an array".into()))?
            .iter()
            .map(parse_relation_expr)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(RelationExpr::Union { children });
    }
    if let Some(intersection) = object.get("intersection") {
        let children = intersection
            .get("child")
            .and_then(Value::as_array)
            .ok_or_else(|| FgaError::BadRequest("intersection.child must be an array".into()))?
            .iter()
            .map(parse_relation_expr)
            .collect::<Result<Vec<_>, _>>()?;
        return Ok(RelationExpr::Intersection { children });
    }
    if let Some(difference) = object.get("difference") {
        let base = difference
            .get("base")
            .ok_or_else(|| FgaError::BadRequest("difference.base is required".into()))?;
        let subtract = difference
            .get("subtract")
            .ok_or_else(|| FgaError::BadRequest("difference.subtract is required".into()))?;
        return Ok(RelationExpr::Difference {
            base: Box::new(parse_relation_expr(base)?),
            subtract: Box::new(parse_relation_expr(subtract)?),
        });
    }
    Err(FgaError::BadRequest(
        "unsupported userset rewrite in relation definition".into(),
    ))
}

fn core_authorization_model() -> AuthorizationModelWriteRequest {
    AuthorizationModelWriteRequest {
        schema_version: SCHEMA_VERSION_1_1.into(),
        type_definitions: vec![
            direct_type("user", &[]),
            direct_type("instance", &["owner", "admin", "viewer", "parent"]),
            direct_type("org", &["owner", "admin", "member", "viewer"]),
            direct_type("group", &["member", "admin"]),
            direct_type("project", &["owner", "admin", "member"]),
            direct_type("app", &["admin", "viewer"]),
            direct_type("settings", &["admin", "viewer"]),
            direct_type("session", &["owner"]),
        ],
        conditions: Map::new(),
    }
}

fn direct_type(type_name: &str, relations: &[&str]) -> TypeDefinition {
    let relation_map = relations
        .iter()
        .map(|relation| (relation.to_string(), json!({ "this": {} })))
        .collect::<Map<String, Value>>();
    let metadata_relations = relations
        .iter()
        .map(|relation| {
            (
                relation.to_string(),
                json!({
                    "directly_related_user_types": [
                        { "type": "user" }
                    ]
                }),
            )
        })
        .collect::<Map<String, Value>>();
    TypeDefinition {
        type_name: type_name.into(),
        relations: relation_map,
        metadata: Some(json!({ "relations": metadata_relations })),
    }
}

fn validate_sealed_core(model: &CompiledModel) -> Result<(), FgaError> {
    let core = core_authorization_model();
    let core_compiled = CompiledModel::from_request(&core)?;
    for (type_name, expected) in core_compiled.raw_types {
        let actual = model
            .raw_types
            .get(&type_name)
            .ok_or_else(|| FgaError::BadRequest(format!("sealed type {type_name} is missing")))?;
        if actual != &expected {
            return Err(FgaError::BadRequest(format!(
                "sealed type {type_name} cannot be modified"
            )));
        }
    }
    Ok(())
}

fn extract_custom_fragment(request: &AuthorizationModelWriteRequest) -> Value {
    let sealed: HashSet<String> = core_authorization_model()
        .type_definitions
        .into_iter()
        .map(|type_def| type_def.type_name)
        .collect();
    json!({
        "schema_version": request.schema_version,
        "type_definitions": request
            .type_definitions
            .iter()
            .filter(|type_def| !sealed.contains(&type_def.type_name))
            .collect::<Vec<_>>(),
        "conditions": request.conditions,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Evaluator;

    use zitadel_db::{DEFAULT_INSTANCE_ID, migrate};

    async fn test_service() -> FgaService {
        let db = Db::open("").await.unwrap();
        migrate::migrate(&db).await.unwrap();
        zitadel_db::bootstrap::bootstrap(&db).await.unwrap();
        FgaService::new(db)
    }

    #[tokio::test]
    async fn initializes_singleton_store_and_default_model() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        assert_eq!(store.id, DEFAULT_INSTANCE_ID);
        let model = service
            .read_model(DEFAULT_INSTANCE_ID, &store.id, None)
            .await
            .unwrap();
        assert_eq!(model.schema_version, SCHEMA_VERSION_1_1);
        assert!(
            model
                .type_definitions
                .iter()
                .any(|type_def| type_def.type_name == "org")
        );
    }

    #[tokio::test]
    async fn write_and_check_direct_relation() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        service
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: vec![TupleKey {
                            user: "user:anne".into(),
                            relation: "member".into(),
                            object: "group:engineering".into(),
                            condition: None,
                        }],
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
            .unwrap();

        let allowed = service
            .check(
                DEFAULT_INSTANCE_ID,
                &store.id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: "user:anne".into(),
                        relation: "member".into(),
                        object: "group:engineering".into(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await
            .unwrap();
        assert!(allowed.allowed);
    }

    #[tokio::test]
    async fn supports_tuple_to_userset_and_difference() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let model = AuthorizationModelWriteRequest {
            schema_version: SCHEMA_VERSION_1_1.into(),
            type_definitions: {
                let mut types = core_authorization_model().type_definitions;
                types.push(TypeDefinition {
                    type_name: "document".into(),
                    relations: Map::from_iter([
                        ("parent".into(), json!({ "this": {} })),
                        (
                            "viewer".into(),
                            json!({
                                "union": {
                                    "child": [
                                        { "this": {} },
                                        { "tupleToUserset": {
                                            "tupleset": { "relation": "parent" },
                                            "computedUserset": { "relation": "viewer" }
                                        }}
                                    ]
                                }
                            }),
                        ),
                        ("blocked".into(), json!({ "this": {} })),
                        (
                            "effective_viewer".into(),
                            json!({
                                "difference": {
                                    "base": { "computedUserset": { "relation": "viewer" } },
                                    "subtract": { "computedUserset": { "relation": "blocked" } }
                                }
                            }),
                        ),
                    ]),
                    metadata: Some(json!({
                        "relations": {
                            "parent": { "directly_related_user_types": [{ "type": "document" }] },
                            "viewer": { "directly_related_user_types": [{ "type": "user" }] },
                            "blocked": { "directly_related_user_types": [{ "type": "user" }] },
                            "effective_viewer": { "directly_related_user_types": [] }
                        }
                    })),
                });
                types
            },
            conditions: Map::new(),
        };
        service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap();
        service
            .write_tuples(
                DEFAULT_INSTANCE_ID,
                &store.id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: vec![
                            TupleKey {
                                user: "document:folder".into(),
                                relation: "parent".into(),
                                object: "document:file".into(),
                                condition: None,
                            },
                            TupleKey {
                                user: "user:anne".into(),
                                relation: "viewer".into(),
                                object: "document:folder".into(),
                                condition: None,
                            },
                            TupleKey {
                                user: "user:bob".into(),
                                relation: "viewer".into(),
                                object: "document:file".into(),
                                condition: None,
                            },
                            TupleKey {
                                user: "user:bob".into(),
                                relation: "blocked".into(),
                                object: "document:file".into(),
                                condition: None,
                            },
                        ],
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
            .unwrap();

        let anne = service
            .check(
                DEFAULT_INSTANCE_ID,
                &store.id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: "user:anne".into(),
                        relation: "effective_viewer".into(),
                        object: "document:file".into(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await
            .unwrap();
        assert!(anne.allowed);

        let bob = service
            .check(
                DEFAULT_INSTANCE_ID,
                &store.id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: "user:bob".into(),
                        relation: "effective_viewer".into(),
                        object: "document:file".into(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await
            .unwrap();
        assert!(!bob.allowed);
    }

    #[tokio::test]
    async fn rejects_sealed_core_mutation() {
        let service = test_service().await;
        let store = service
            .initialize_instance(DEFAULT_INSTANCE_ID)
            .await
            .unwrap();
        let mut model = core_authorization_model();
        model.type_definitions[1]
            .relations
            .insert("superadmin".into(), json!({ "this": {} }));
        let err = service
            .write_model(DEFAULT_INSTANCE_ID, &store.id, model)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("sealed type instance"));
    }
}
