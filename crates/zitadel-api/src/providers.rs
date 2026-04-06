use crate::{ApiState, extractors::ResourceId, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::State,
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_app::{
    providers::{CreateProviderCommand, UpdateProviderCommand},
    repo::{ListParams as AppListParams, ProviderRecord},
};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/providers", get(list).post(create))
        .route(
        "/providers/{id}",
        get(get_one).patch(update).delete(
            |state: State<ApiState>, identity: Extension<Identity>, path: ResourceId| async move {
                delete_one(state, identity, path).await
            },
        ),
    )
}

#[derive(Deserialize)]
pub struct ProviderRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub protocol: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Serialize)]
pub struct ProviderResponse {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub state: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub config: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ProviderRecord> for ProviderResponse {
    fn from(r: ProviderRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            protocol: r.protocol,
            state: r.state,
            config: r.config,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

async fn create(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<ProviderRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = CreateProviderCommand {
        name: req.name,
        protocol: req.protocol,
        config: req.config,
    };
    match s.app.runner.run(&ctx, "provider.create", || s.app.create_provider.execute(&ctx, cmd)).await {
        Ok(provider) => response::json_created(ProviderResponse::from(provider)),
        Err(e) => response::app_error(e),
    }
}

async fn list(State(s): State<ApiState>, Extension(identity): Extension<Identity>) -> Response {
    let ctx = response::build_actor_context(&identity);
    let params = AppListParams {
        limit: Some(200),
        cursor: None,
        search: None,
    };
    match s.app.runner.run(&ctx, "provider.list", || s.app.list_providers.execute(&ctx, &params)).await {
        Ok(result) => {
            let items: Vec<ProviderResponse> = result
                .items
                .into_iter()
                .map(ProviderResponse::from)
                .collect();
            let total = items.len();
            response::json_ok(serde_json::json!({
                "providers": items,
                "items": items,
                "total": total,
            }))
        }
        Err(e) => response::app_error(e),
    }
}

async fn get_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run(&ctx, "provider.get", || s.app.get_provider.execute(&ctx, &id)).await {
        Ok(provider) => response::json_ok(ProviderResponse::from(provider)),
        Err(e) => response::app_error(e),
    }
}

async fn update(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
    Json(req): Json<ProviderRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = UpdateProviderCommand {
        provider_id: id,
        name: if req.name.is_empty() {
            None
        } else {
            Some(req.name)
        },
        config: if req.config.is_null() {
            None
        } else {
            Some(req.config)
        },
    };
    match s.app.runner.run(&ctx, "provider.update", || s.app.update_provider.execute(&ctx, cmd)).await {
        Ok(provider) => response::json_ok(ProviderResponse::from(provider)),
        Err(e) => response::app_error(e),
    }
}

async fn delete_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run(&ctx, "provider.delete", || s.app.delete_provider.execute(&ctx, &id)).await {
        Ok(()) => response::no_content(),
        Err(e) => response::app_error(e),
    }
}
