use crate::{ApiState, extractors::ResourceId, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_app::{
    orgs::{CreateOrgCommand, UpdateOrgCommand},
    repo::{ListParams as AppListParams, OrgRecord},
};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/orgs", get(list).post(create))
        .route("/orgs/{id}", get(get_one).patch(update).delete(delete_one))
        .merge(crate::generic_named_resource::membership_routes("org", "orgs", "org_id"))
}

#[derive(Deserialize)]
pub struct OrgRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub state: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Serialize)]
pub struct OrgResponse {
    pub id: String,
    pub name: String,
    pub state: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

impl From<OrgRecord> for OrgResponse {
    fn from(r: OrgRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            state: r.state,
            metadata: r.metadata,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

async fn create(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<OrgRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = CreateOrgCommand {
        name: req.name,
        metadata: req.metadata,
    };
    match s.app.runner.run_fn(&ctx, "org.create", || s.app.create_org.execute(&ctx, cmd)).await {
        Ok(org) => response::json_created(OrgResponse::from(org)),
        Err(e) => response::app_error(e),
    }
}

async fn get_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run_fn(&ctx, "org.get", || s.app.get_org.execute(&ctx, &id)).await {
        Ok(org) => response::json_ok(OrgResponse::from(org)),
        Err(e) => response::app_error(e),
    }
}

async fn list(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(p): Query<response::PaginationParams>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let params = AppListParams {
        limit: Some((p.limit.min(200)) as u32),
        cursor: p.cursor,
        search: None,
    };
    match s.app.runner.run_fn(&ctx, "org.list", || s.app.list_orgs.execute(&ctx, &params)).await {
        Ok(result) => {
            let items: Vec<OrgResponse> = result.items.into_iter().map(OrgResponse::from).collect();
            response::json_ok(response::ListResponse {
                items,
                next_cursor: result.next_cursor,
                total: result.total_count.map(|c| c as i64),
            })
        }
        Err(e) => response::app_error(e),
    }
}

async fn update(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
    Json(req): Json<OrgRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = UpdateOrgCommand {
        org_id: id,
        name: if req.name.is_empty() {
            None
        } else {
            Some(req.name)
        },
        metadata: if req.metadata.is_null() {
            None
        } else {
            Some(req.metadata)
        },
    };
    match s.app.runner.run_fn(&ctx, "org.update", || s.app.update_org.execute(&ctx, cmd)).await {
        Ok(org) => response::json_ok(OrgResponse::from(org)),
        Err(e) => response::app_error(e),
    }
}

async fn delete_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.runner.run_fn(&ctx, "org.delete", || s.app.delete_org.execute(&ctx, &id)).await {
        Ok(()) => response::no_content(),
        Err(e) => response::app_error(e),
    }
}
