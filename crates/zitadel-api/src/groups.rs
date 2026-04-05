use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_app::{
    groups::{CreateGroupCommand, UpdateGroupCommand},
    repo::{GroupRecord, ListParams as AppListParams},
};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/groups", get(list).post(create))
        .route(
            "/groups/{id}",
            get(get_one).patch(update).delete(delete_one),
        )
}

#[derive(Deserialize)]
pub struct CreateRequest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Serialize)]
pub struct ItemResponse {
    pub id: String,
    pub name: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<GroupRecord> for ItemResponse {
    fn from(r: GroupRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            state: r.state,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

async fn create(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<CreateRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = CreateGroupCommand {
        name: req.name,
        org_id: identity.org_id.clone(),
        metadata: req.metadata,
    };
    match s.app.create_group.execute(&ctx, cmd).await {
        Ok(group) => response::json_created(ItemResponse::from(group)),
        Err(e) => response::app_error(e),
    }
}

async fn get_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(id): Path<String>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s.app.get_group.execute(&ctx, &id).await {
        Ok(group) => response::json_ok(ItemResponse::from(group)),
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
        limit: Some(p.limit.min(200) as u32),
        cursor: p.cursor,
        search: None,
    };
    match s.app.list_groups.execute(&ctx, None, &params).await {
        Ok(result) => {
            let items: Vec<ItemResponse> =
                result.items.into_iter().map(ItemResponse::from).collect();
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
    Path(id): Path<String>,
    Json(req): Json<CreateRequest>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cmd = UpdateGroupCommand {
        group_id: id,
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
    match s.app.update_group.execute(&ctx, cmd).await {
        Ok(group) => response::json_ok(ItemResponse::from(group)),
        Err(e) => response::app_error(e),
    }
}

async fn delete_one(State(s): State<ApiState>, Path(id): Path<String>) -> Response {
    // No delete_group use case — keep direct DB call.
    // TODO(CLAUDE-4): Add DeleteGroup use case.
    match zitadel_db::delete_instance_row(
        &s.db,
        zitadel_db::current_instance_id().as_ref(),
        "groups",
        &id,
    )
    .await
    {
        Ok(true) => response::no_content(),
        Ok(false) => response::not_found("group not found"),
        Err(e) => response::internal_error(format!("{e}")),
    }
}
