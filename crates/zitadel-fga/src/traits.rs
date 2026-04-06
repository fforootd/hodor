use async_trait::async_trait;

use crate::dto::*;
use crate::error::FgaError;

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
