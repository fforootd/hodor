use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
