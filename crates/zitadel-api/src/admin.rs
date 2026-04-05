use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Router,
    extract::{Query, State},
    http::StatusCode,
    response::Response,
    routing::get,
};
use serde::Serialize;
use zitadel_db::{list_admin_instances, load_instance_metadata};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/admin/instances", get(list_instances))
        .route("/admin/whoami", get(whoami))
}

#[derive(Serialize)]
struct AdminInstanceResponse {
    instance_id: String,
    owner_org_id: String,
    kind: String,
    state: String,
    placement_mode: String,
    region_key: Option<String>,
    primary_domain: Option<String>,
}

#[derive(Serialize)]
struct AdminWhoAmIResponse {
    operator_admin: bool,
    instance_id: String,
    is_root: bool,
}

async fn list_instances(
    State(state): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(p): Query<response::PaginationParams>,
) -> Response {
    let root_instance_id = match require_operator_root(&state, &identity).await {
        Ok(root_instance_id) => root_instance_id,
        Err(response) => return response,
    };
    let limit = p.limit.min(200);
    let cursor = p.cursor.unwrap_or_default();
    match list_admin_instances(&state.db, &root_instance_id, &cursor, limit + 1).await {
        Ok(rows) => {
            let has_more = rows.len() as i64 > limit;
            let items: Vec<AdminInstanceResponse> = rows
                .into_iter()
                .take(limit as usize)
                .map(|row| AdminInstanceResponse {
                    instance_id: row.instance_id,
                    owner_org_id: row.owner_org_id,
                    kind: row.kind,
                    state: row.state,
                    placement_mode: row.placement_mode,
                    region_key: row.region_key,
                    primary_domain: row.primary_domain,
                })
                .collect();
            let next_cursor = if has_more {
                items.last().map(|item| item.instance_id.clone())
            } else {
                None
            };
            response::json_ok(response::ListResponse {
                items,
                next_cursor,
                total: None,
            })
        }
        Err(error) => response::internal_error(format!("{error}")),
    }
}

async fn whoami(
    State(state): State<ApiState>,
    Extension(identity): Extension<Identity>,
) -> Response {
    let is_root = match current_instance_kind(&state).await {
        Ok(kind) => kind == "root",
        Err(response) => return response,
    };
    response::json_ok(AdminWhoAmIResponse {
        operator_admin: identity.operator_admin,
        instance_id: zitadel_db::current_instance_id().into_owned(),
        is_root,
    })
}

async fn require_operator_root(state: &ApiState, identity: &Identity) -> Result<String, Response> {
    if !identity.operator_admin {
        return Err(response::error(
            StatusCode::FORBIDDEN,
            "operator admin required",
        ));
    }
    match current_instance_kind(state).await {
        Ok(kind) if kind == "root" => Ok(zitadel_db::current_instance_id().into_owned()),
        Ok(_) => Err(response::error(
            StatusCode::FORBIDDEN,
            "operator admin is only available from the root instance",
        )),
        Err(response) => Err(response),
    }
}

async fn current_instance_kind(state: &ApiState) -> Result<String, Response> {
    load_instance_metadata(&state.db, zitadel_db::current_instance_id().as_ref())
        .await
        .map_err(|error| response::internal_error(format!("{error}")))?
        .map(|row| row.kind)
        .ok_or_else(|| response::not_found("current instance not found"))
}
