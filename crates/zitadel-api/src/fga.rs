use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;
use zitadel_db::current_instance_id;
use zitadel_fga::{
    AuthorizationModelWriteRequest, BatchCheckRequest, ChangeRepository, CheckRequest,
    CheckResponse, Evaluator, ExpandRequest, FgaApi, FgaError, ListObjectsRequest,
    ListUsersRequest, ModelRepository, PLATFORM_STORE_ID, ReadRequest, StoreResolver,
    TupleFilter, TupleKey, TupleKeySet, TupleRepository, WriteRequest,
};

use crate::extractors::{StoreId, StoreModelPath};
use crate::{ApiState, middleware, response};

pub fn customer_routes() -> Router<ApiState> {
    Router::new()
        .route("/fga/store", get(discover_customer_store))
        .route("/fga/check", post(legacy_check))
        .route(
            "/fga/tuples",
            get(legacy_read_tuples)
                .post(legacy_write_tuples)
                .delete(legacy_delete_tuples),
        )
        .route("/fga/list-objects", post(legacy_list_objects))
        .route("/fga/model", get(legacy_model).post(legacy_write_model))
        .route("/fga/model/graph", get(legacy_model_graph))
        .route("/fga/expand", post(legacy_expand))
        .route("/fga/test", post(legacy_batch_test))
        .route("/fga/stores/{store_id}/check", post(check_store))
        .route("/fga/stores/{store_id}/batch-check", post(batch_check_store))
        .route("/fga/stores/{store_id}/read", post(read_store))
        .route("/fga/stores/{store_id}/write", post(write_store))
        .route("/fga/stores/{store_id}/expand", post(expand_store))
        .route("/fga/stores/{store_id}/list-objects", post(list_objects_store))
        .route("/fga/stores/{store_id}/list-users", post(list_users_store))
        .route("/fga/stores/{store_id}/changes", get(read_changes_store))
        .route(
            "/fga/stores/{store_id}/authorization-models",
            get(read_authorization_models_store).post(write_authorization_model_store),
        )
        .route(
            "/fga/stores/{store_id}/authorization-models/{model_id}",
            get(read_authorization_model_store),
        )
}

pub fn internal_platform_routes() -> Router<ApiState> {
    Router::new()
        .route("/internal/fga/platform/store", get(discover_platform_store))
        .route("/internal/fga/platform/check", post(check_platform_store))
        .route("/internal/fga/platform/read", post(read_platform_store))
        .route("/internal/fga/platform/changes", get(read_platform_changes))
        .route(
            "/internal/fga/platform/authorization-models",
            get(read_platform_authorization_models),
        )
        .route(
            "/internal/fga/platform/authorization-models/{model_id}",
            get(read_platform_authorization_model),
        )
        .route_layer(axum::middleware::from_fn(middleware::require_fga_admin_pat))
}

fn fga_error_response(error: FgaError) -> Response {
    let (status, code, message, kind) = match error {
        FgaError::BadRequest(message) => (
            axum::http::StatusCode::BAD_REQUEST,
            "invalid_request",
            message,
            "configuration",
        ),
        FgaError::NotFound(message) => (
            axum::http::StatusCode::NOT_FOUND,
            "not_found",
            message,
            "internal",
        ),
        FgaError::Forbidden(message) => (
            axum::http::StatusCode::FORBIDDEN,
            "forbidden",
            message,
            "internal",
        ),
        FgaError::Unsupported(message) => (
            axum::http::StatusCode::NOT_IMPLEMENTED,
            "unsupported",
            message,
            "configuration",
        ),
        FgaError::Internal(error) => {
            tracing::error!(error = %error, "embedded fga request failed");
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "authorization engine error".to_string(),
                "internal",
            )
        }
    };

    (
        status,
        Json(json!({
            "error": {
                "message": message,
                "code": code,
                "retryable": false,
                "kind": kind,
            }
        })),
    )
        .into_response()
}

async fn require_customer_fga_read(
    state: &ApiState,
    identity: &middleware::Identity,
) -> Result<(), Response> {
    let ctx = response::build_actor_context(identity);
    crate::fga_check(
        state,
        &ctx,
        "viewer",
        &format!("instance:{}", ctx.instance_id()),
    )
    .await
}

async fn require_customer_fga_write(
    state: &ApiState,
    identity: &middleware::Identity,
) -> Result<(), Response> {
    let ctx = response::build_actor_context(identity);
    crate::fga_check(
        state,
        &ctx,
        "admin",
        &format!("instance:{}", ctx.instance_id()),
    )
    .await
}

fn validate_customer_target_instance(instance_id: &str) -> Result<(), FgaError> {
    if instance_id == PLATFORM_STORE_ID {
        return Err(FgaError::Forbidden(
            "platform store is not available on customer-facing routes".into(),
        ));
    }
    Ok(())
}

fn validate_customer_store_id(instance_id: &str, store_id: &str) -> Result<(), FgaError> {
    validate_customer_target_instance(instance_id)?;
    if store_id == PLATFORM_STORE_ID {
        return Err(FgaError::Forbidden(
            "platform store is not available on customer-facing routes".into(),
        ));
    }
    if store_id != instance_id {
        return Err(FgaError::Forbidden(
            "store_id must match the resolved target instance".into(),
        ));
    }
    Ok(())
}

async fn current_customer_store(state: &ApiState) -> Result<(String, String), FgaError> {
    let instance_id = current_instance_id().into_owned();
    validate_customer_target_instance(&instance_id)?;
    let store = state.fga.discover_store(&instance_id).await?;
    validate_customer_store_id(&instance_id, &store.id)?;
    Ok((instance_id, store.id))
}

async fn discover_customer_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    match current_customer_store(&state).await {
        Ok((instance_id, store_id)) => response::json_ok(json!({
            "store_id": store_id,
            "name": format!("zitadel-{instance_id}"),
            "instance_id": instance_id,
        })),
        Err(error) => fga_error_response(error),
    }
}

async fn discover_platform_store(State(state): State<ApiState>) -> Response {
    match state.fga.discover_platform_store().await {
        Ok(store) => response::json_ok(json!({
            "store_id": store.id,
            "name": store.name,
            "instance_id": PLATFORM_STORE_ID,
        })),
        Err(error) => fga_error_response(error),
    }
}

#[derive(Deserialize)]
struct LegacyCheckRequest {
    user: String,
    relation: String,
    object: String,
}

async fn legacy_check(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    Json(body): Json<LegacyCheckRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    match current_customer_store(&state).await {
        Ok((instance_id, store_id)) => match state
            .fga
            .check(
                &instance_id,
                &store_id,
                CheckRequest {
                    tuple_key: TupleKey {
                        user: body.user.clone(),
                        relation: body.relation.clone(),
                        object: body.object.clone(),
                        condition: None,
                    },
                    authorization_model_id: None,
                    contextual_tuples: None,
                    context: None,
                },
            )
            .await
        {
            Ok(CheckResponse { allowed }) => response::json_ok(json!({
                "allowed": allowed,
                "user": body.user,
                "relation": body.relation,
                "object": body.object,
            })),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

#[derive(Deserialize)]
struct LegacyTupleQuery {
    user: Option<String>,
    relation: Option<String>,
    object: Option<String>,
}

async fn legacy_read_tuples(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    Query(query): Query<LegacyTupleQuery>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    match current_customer_store(&state).await {
        Ok((instance_id, store_id)) => match state
            .fga
            .read_tuples(
                &instance_id,
                &store_id,
                ReadRequest {
                    tuple_key: Some(TupleFilter {
                        user: query.user,
                        relation: query.relation,
                        object: query.object,
                    }),
                    page_size: Some(100),
                    continuation_token: None,
                },
            )
            .await
        {
            Ok(result) => response::json_ok(json!({
                "tuples": result.tuples.into_iter().map(|tuple| tuple.key).collect::<Vec<_>>()
            })),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

#[derive(Deserialize)]
struct LegacyTupleWriteRequest {
    tuples: Vec<TupleKey>,
}

async fn legacy_write_tuples(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    Json(body): Json<LegacyTupleWriteRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_write(&state, &identity).await {
        return response;
    }
    let count = body.tuples.len();
    match current_customer_store(&state).await {
        Ok((instance_id, store_id)) => match state
            .fga
            .write_tuples(
                &instance_id,
                &store_id,
                WriteRequest {
                    writes: TupleKeySet {
                        tuple_keys: body.tuples,
                    },
                    deletes: TupleKeySet { tuple_keys: vec![] },
                    authorization_model_id: None,
                },
            )
            .await
        {
            Ok(()) => response::json_ok(json!({
                "status": "ok",
                "written": count,
            })),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

async fn legacy_delete_tuples(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    Json(body): Json<LegacyTupleWriteRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_write(&state, &identity).await {
        return response;
    }
    let count = body.tuples.len();
    match current_customer_store(&state).await {
        Ok((instance_id, store_id)) => match state
            .fga
            .write_tuples(
                &instance_id,
                &store_id,
                WriteRequest {
                    writes: TupleKeySet { tuple_keys: vec![] },
                    deletes: TupleKeySet {
                        tuple_keys: body.tuples,
                    },
                    authorization_model_id: None,
                },
            )
            .await
        {
            Ok(()) => response::json_ok(json!({
                "status": "ok",
                "deleted": count,
            })),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

async fn legacy_list_objects(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    Json(body): Json<ListObjectsRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    match current_customer_store(&state).await {
        Ok((instance_id, store_id)) => match state.fga.list_objects(&instance_id, &store_id, body).await {
            Ok(result) => response::json_ok(result),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

async fn legacy_model(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_target_instance(&instance_id) {
        return fga_error_response(error);
    }
    match state.fga.legacy_model(&instance_id).await {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn legacy_write_model(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    Json(body): Json<AuthorizationModelWriteRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_write(&state, &identity).await {
        return response;
    }
    match current_customer_store(&state).await {
        Ok((instance_id, store_id)) => match state.fga.write_model(&instance_id, &store_id, body).await {
            Ok(result) => response::json_ok(result),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

async fn legacy_model_graph(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_target_instance(&instance_id) {
        return fga_error_response(error);
    }
    match state.fga.legacy_model_graph(&instance_id).await {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn legacy_expand(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    Json(body): Json<ExpandRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    match current_customer_store(&state).await {
        Ok((instance_id, store_id)) => match state.fga.expand(&instance_id, &store_id, body).await {
            Ok(result) => response::json_ok(result),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

#[derive(Deserialize)]
struct LegacyBatchAssertion {
    user: String,
    relation: String,
    object: String,
    expected: bool,
}

#[derive(Deserialize)]
struct LegacyBatchRequest {
    assertions: Vec<LegacyBatchAssertion>,
}

async fn legacy_batch_test(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    Json(body): Json<LegacyBatchRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    match current_customer_store(&state).await {
        Ok((instance_id, store_id)) => {
            let request = BatchCheckRequest {
                checks: body
                    .assertions
                    .iter()
                    .enumerate()
                    .map(|(idx, assertion)| zitadel_fga::BatchCheckItem {
                        tuple_key: TupleKey {
                            user: assertion.user.clone(),
                            relation: assertion.relation.clone(),
                            object: assertion.object.clone(),
                            condition: None,
                        },
                        correlation_id: Some(idx.to_string()),
                    })
                    .collect(),
                authorization_model_id: None,
                contextual_tuples: None,
                context: None,
            };
            match state.fga.batch_check(&instance_id, &store_id, request).await {
                Ok(result) => {
                    let mut passed = 0usize;
                    let results = body
                        .assertions
                        .into_iter()
                        .zip(result.results)
                        .map(|(assertion, actual)| {
                            let pass = assertion.expected == actual.allowed;
                            if pass {
                                passed += 1;
                            }
                            json!({
                                "user": assertion.user,
                                "relation": assertion.relation,
                                "object": assertion.object,
                                "expected": assertion.expected,
                                "actual": actual.allowed,
                                "pass": pass,
                            })
                        })
                        .collect::<Vec<_>>();
                    response::json_ok(json!({
                        "total": results.len(),
                        "passed": passed,
                        "failed": results.len().saturating_sub(passed),
                        "results": results,
                    }))
                }
                Err(error) => fga_error_response(error),
            }
        }
        Err(error) => fga_error_response(error),
    }
}

async fn check_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreId(store_id): StoreId,
    Json(body): Json<CheckRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state.fga.check(&instance_id, &store_id, body).await {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn batch_check_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreId(store_id): StoreId,
    Json(body): Json<BatchCheckRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state.fga.batch_check(&instance_id, &store_id, body).await {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn read_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreId(store_id): StoreId,
    Json(body): Json<ReadRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state.fga.read_tuples(&instance_id, &store_id, body).await {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn write_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreId(store_id): StoreId,
    Json(body): Json<WriteRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_write(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state.fga.write_tuples(&instance_id, &store_id, body).await {
        Ok(()) => response::json_ok(json!({})),
        Err(error) => fga_error_response(error),
    }
}

async fn expand_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreId(store_id): StoreId,
    Json(body): Json<ExpandRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state.fga.expand(&instance_id, &store_id, body).await {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn list_objects_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreId(store_id): StoreId,
    Json(body): Json<ListObjectsRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state.fga.list_objects(&instance_id, &store_id, body).await {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn list_users_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreId(store_id): StoreId,
    Json(body): Json<ListUsersRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state.fga.list_users(&instance_id, &store_id, body).await {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

#[derive(Deserialize)]
struct ReadChangesQuery {
    #[serde(rename = "type")]
    object_type: Option<String>,
    page_size: Option<u32>,
    continuation_token: Option<String>,
}

async fn read_changes_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreId(store_id): StoreId,
    Query(query): Query<ReadChangesQuery>,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state
        .fga
        .read_changes(
            &instance_id,
            &store_id,
            query.object_type.as_deref(),
            query.page_size.unwrap_or(50),
            query.continuation_token.as_deref(),
        )
        .await
    {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn read_authorization_models_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreId(store_id): StoreId,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state.fga.read_models(&instance_id, &store_id).await {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn read_authorization_model_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreModelPath { store_id, model_id }: StoreModelPath,
) -> Response {
    if let Err(response) = require_customer_fga_read(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state
        .fga
        .read_model(&instance_id, &store_id, Some(&model_id))
        .await
    {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn write_authorization_model_store(
    State(state): State<ApiState>,
    Extension(identity): Extension<middleware::Identity>,
    StoreId(store_id): StoreId,
    Json(body): Json<AuthorizationModelWriteRequest>,
) -> Response {
    if let Err(response) = require_customer_fga_write(&state, &identity).await {
        return response;
    }
    let instance_id = current_instance_id().into_owned();
    if let Err(error) = validate_customer_store_id(&instance_id, &store_id) {
        return fga_error_response(error);
    }
    match state.fga.write_model(&instance_id, &store_id, body).await {
        Ok(result) => response::json_ok(result),
        Err(error) => fga_error_response(error),
    }
}

async fn check_platform_store(
    State(state): State<ApiState>,
    Json(body): Json<CheckRequest>,
) -> Response {
    match state.fga.discover_platform_store().await {
        Ok(store) => match state.fga.check(PLATFORM_STORE_ID, &store.id, body).await {
            Ok(result) => response::json_ok(result),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

async fn read_platform_store(
    State(state): State<ApiState>,
    Json(body): Json<ReadRequest>,
) -> Response {
    match state.fga.discover_platform_store().await {
        Ok(store) => match state.fga.read_tuples(PLATFORM_STORE_ID, &store.id, body).await {
            Ok(result) => response::json_ok(result),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

async fn read_platform_changes(
    State(state): State<ApiState>,
    Query(query): Query<ReadChangesQuery>,
) -> Response {
    match state.fga.discover_platform_store().await {
        Ok(store) => match state
            .fga
            .read_changes(
                PLATFORM_STORE_ID,
                &store.id,
                query.object_type.as_deref(),
                query.page_size.unwrap_or(50),
                query.continuation_token.as_deref(),
            )
            .await
        {
            Ok(result) => response::json_ok(result),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

async fn read_platform_authorization_models(State(state): State<ApiState>) -> Response {
    match state.fga.discover_platform_store().await {
        Ok(store) => match state.fga.read_models(PLATFORM_STORE_ID, &store.id).await {
            Ok(result) => response::json_ok(result),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}

async fn read_platform_authorization_model(
    State(state): State<ApiState>,
    axum::extract::Path(model_id): axum::extract::Path<String>,
) -> Response {
    match state.fga.discover_platform_store().await {
        Ok(store) => match state
            .fga
            .read_model(PLATFORM_STORE_ID, &store.id, Some(&model_id))
            .await
        {
            Ok(result) => response::json_ok(result),
            Err(error) => fga_error_response(error),
        },
        Err(error) => fga_error_response(error),
    }
}
