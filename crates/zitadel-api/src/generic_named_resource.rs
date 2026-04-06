//! Generic CRUD handlers for "named resources" — entities that share an
//! identical create/get/list/update/delete shape backed by app-layer use cases.
//!
//! Used by `apps` and `projects` (and any future resource with the same shape)
//! to avoid duplicating ~140 lines of boilerplate per resource kind.

use crate::{ApiState, extractors::ResourceId, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_app::resources::{
    CreateNamedResourceCommand, UpdateNamedResourceCommand,
};
use zitadel_app::repo::NamedResourceRecord;

// ─── Config ────────────────────────────────────────────────

/// Injected via `Extension` so handlers know which resource table to operate on.
#[derive(Clone)]
struct ResourceKind(&'static str);

/// Build a router for a named-resource CRUD family.
///
/// `kind` is the DB resource-type key (e.g. `"apps"`, `"projects"`).
/// `prefix` is the URL path segment (usually the same as `kind`).
pub fn routes(kind: &'static str, prefix: &str) -> Router<ApiState> {
    let list_path = format!("/{prefix}");
    let detail_path = format!("/{prefix}/{{id}}");

    Router::new()
        .route(&list_path, get(list).post(create))
        .route(
            &detail_path,
            get(get_one).patch(update).delete(delete_one),
        )
        .layer(Extension(ResourceKind(kind)))
}

// ─── DTOs ──────────────────────────────────────────────────

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

impl From<NamedResourceRecord> for ItemResponse {
    fn from(r: NamedResourceRecord) -> Self {
        Self {
            id: r.id,
            name: r.name,
            state: r.state,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

// ─── Handlers ──────────────────────────────────────────────

async fn create(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Extension(ResourceKind(kind)): Extension<ResourceKind>,
    Json(req): Json<CreateRequest>,
) -> Response {
    if req.name.is_empty() {
        return response::bad_request("name is required");
    }
    let ctx = response::build_actor_context(&identity);
    let cmd = CreateNamedResourceCommand {
        kind: kind.to_string(),
        name: req.name,
        org_id: identity.org_id.clone(),
    };
    match s
        .app
        .runner
        .run_fn(&ctx, "resource.create", || {
            s.app.create_named_resource.execute(&ctx, cmd)
        })
        .await
    {
        Ok(record) => response::json_created(ItemResponse::from(record)),
        Err(e) => response::app_error(e),
    }
}

async fn get_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Extension(ResourceKind(kind)): Extension<ResourceKind>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run_fn(&ctx, "resource.get", || {
            s.app.get_named_resource.execute(&ctx, kind, &id)
        })
        .await
    {
        Ok(r) => response::json_ok(ItemResponse::from(r)),
        Err(e) => response::app_error(e),
    }
}

async fn list(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Extension(ResourceKind(kind)): Extension<ResourceKind>,
    Query(p): Query<response::PaginationParams>,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    let cursor = p.cursor.unwrap_or_default();
    let limit = p.limit.min(200);
    match s
        .app
        .runner
        .run_fn(&ctx, "resource.list", || {
            s.app
                .list_named_resources
                .execute(&ctx, kind, &cursor, limit)
        })
        .await
    {
        Ok(rows) => {
            let items: Vec<ItemResponse> = rows.into_iter().map(ItemResponse::from).collect();
            response::json_ok(response::ListResponse {
                items,
                next_cursor: None,
                total: None,
            })
        }
        Err(e) => response::app_error(e),
    }
}

async fn update(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Extension(ResourceKind(kind)): Extension<ResourceKind>,
    ResourceId(id): ResourceId,
    Json(req): Json<CreateRequest>,
) -> Response {
    if req.name.is_empty() {
        return response::bad_request("name required");
    }
    let ctx = response::build_actor_context(&identity);
    let cmd = UpdateNamedResourceCommand {
        kind: kind.to_string(),
        id: id.clone(),
        name: req.name,
    };
    match s
        .app
        .runner
        .run_fn(&ctx, "resource.update", || {
            s.app.update_named_resource.execute(&ctx, cmd)
        })
        .await
    {
        Ok(true) => response::json_ok(serde_json::json!({"id": id, "updated": true})),
        Ok(false) => response::not_found("not found"),
        Err(e) => response::app_error(e),
    }
}

async fn delete_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Extension(ResourceKind(kind)): Extension<ResourceKind>,
    ResourceId(id): ResourceId,
) -> Response {
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run_fn(&ctx, "resource.delete", || {
            s.app.delete_named_resource.execute(&ctx, kind, &id)
        })
        .await
    {
        Ok(true) => response::no_content(),
        Ok(false) => response::not_found("not found"),
        Err(e) => response::app_error(e),
    }
}

// ─── Membership sub-resource ───────────────────────────────

pub use membership::routes as membership_routes;

pub mod membership {
    //! Generic membership handlers for entities that have `/{entity_id}/members`
    //! sub-resources (orgs, groups, etc.).

    use super::*;
    use std::collections::HashMap;
    use axum::extract::Path;
    use zitadel_db::current_instance_id;

    /// Injected via `Extension` so handlers know which entity type / path param to use.
    #[derive(Clone)]
    struct MembershipConfig {
        entity_type: &'static str,
        id_param: &'static str,
    }

    /// Build membership sub-routes to be merged into a parent router.
    ///
    /// `entity_type` is the DB membership type key (e.g. `"org"`, `"group"`).
    /// `prefix` is the URL path segment for the parent (e.g. `"orgs"`, `"groups"`).
    /// `id_param` is the path parameter name for the parent ID (e.g. `"org_id"`, `"group_id"`).
    pub fn routes(
        entity_type: &'static str,
        prefix: &str,
        id_param: &'static str,
    ) -> Router<ApiState> {
        let config = MembershipConfig {
            entity_type,
            id_param,
        };
        let list_path = format!("/{prefix}/{{{id_param}}}/members");
        let member_path = format!("/{prefix}/{{{id_param}}}/members/{{user_id}}");

        Router::new()
            .route(&list_path, get(list_members).post(add_member))
            .route(&member_path, axum::routing::delete(remove_member))
            .layer(Extension(config))
    }

    #[derive(Serialize)]
    struct MemberResponse {
        user_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        display_name: Option<String>,
        role: String,
        added_at: String,
    }

    #[derive(Deserialize)]
    struct AddMemberRequest {
        user_id: String,
        #[serde(default = "default_role")]
        role: String,
    }

    fn default_role() -> String {
        "member".to_string()
    }

    async fn list_members(
        State(s): State<ApiState>,
        Extension(identity): Extension<Identity>,
        Extension(config): Extension<MembershipConfig>,
        Path(params): Path<HashMap<String, String>>,
    ) -> Response {
        let instance_id = current_instance_id();
        let entity_id = match params.get(config.id_param) {
            Some(id) => id.as_str(),
            None => return response::bad_request(format!("missing {}", config.id_param)),
        };
        let ctx = response::build_actor_context(&identity);
        let object = format!("{}:{}", config.entity_type, entity_id);
        if let Err(e) = crate::fga_check(&s, &ctx, "viewer", &object).await {
            return e;
        }
        match zitadel_db::list_memberships(&s.db, &instance_id, config.entity_type, entity_id)
            .await
        {
            Ok(rows) => {
                let items: Vec<MemberResponse> = rows
                    .into_iter()
                    .map(|r| MemberResponse {
                        user_id: r.user_id,
                        display_name: r.display_name,
                        role: r.role,
                        added_at: r.added_at,
                    })
                    .collect();
                response::json_ok(response::ListResponse {
                    items,
                    next_cursor: None,
                    total: None,
                })
            }
            Err(e) => response::internal(e),
        }
    }

    async fn add_member(
        State(s): State<ApiState>,
        Extension(identity): Extension<Identity>,
        Extension(config): Extension<MembershipConfig>,
        Path(params): Path<HashMap<String, String>>,
        Json(req): Json<AddMemberRequest>,
    ) -> Response {
        let instance_id = current_instance_id();
        let entity_id = match params.get(config.id_param) {
            Some(id) => id.as_str(),
            None => return response::bad_request(format!("missing {}", config.id_param)),
        };
        let ctx = response::build_actor_context(&identity);
        let object = format!("{}:{}", config.entity_type, entity_id);
        if let Err(e) = crate::fga_check(&s, &ctx, "admin", &object).await {
            return e;
        }
        match zitadel_db::add_membership(
            &s.db,
            &instance_id,
            config.entity_type,
            entity_id,
            &req.user_id,
            &req.role,
        )
        .await
        {
            Ok(()) => {
                // Emit membership event (best-effort — don't fail the request)
                let _ = s.app.repos.events.append(
                    &instance_id,
                    &zitadel_app::DomainEvent::MembershipChanged {
                        entity_type: config.entity_type.to_string(),
                        entity_id: entity_id.to_string(),
                        user_id: req.user_id.clone(),
                        action: "added".to_string(),
                        role: req.role.clone(),
                        actor_id: ctx.user_id().to_string(),
                    },
                    None, None, None,
                ).await;
                response::json_created(MemberResponse {
                    user_id: req.user_id,
                    display_name: None,
                    role: req.role,
                    added_at: String::new(),
                })
            }
            Err(e) => response::internal(e),
        }
    }

    async fn remove_member(
        State(s): State<ApiState>,
        Extension(identity): Extension<Identity>,
        Extension(config): Extension<MembershipConfig>,
        Path(params): Path<HashMap<String, String>>,
    ) -> Response {
        let instance_id = current_instance_id();
        let entity_id = match params.get(config.id_param) {
            Some(id) => id.as_str(),
            None => return response::bad_request(format!("missing {}", config.id_param)),
        };
        let user_id = match params.get("user_id") {
            Some(id) => id.as_str(),
            None => return response::bad_request("missing user_id"),
        };
        let ctx = response::build_actor_context(&identity);
        let object = format!("{}:{}", config.entity_type, entity_id);
        if let Err(e) = crate::fga_check(&s, &ctx, "admin", &object).await {
            return e;
        }
        match zitadel_db::remove_membership(
            &s.db,
            &instance_id,
            config.entity_type,
            entity_id,
            user_id,
        )
        .await
        {
            Ok(()) => {
                let _ = s.app.repos.events.append(
                    &instance_id,
                    &zitadel_app::DomainEvent::MembershipChanged {
                        entity_type: config.entity_type.to_string(),
                        entity_id: entity_id.to_string(),
                        user_id: user_id.to_string(),
                        action: "removed".to_string(),
                        role: String::new(),
                        actor_id: ctx.user_id().to_string(),
                    },
                    None, None, None,
                ).await;
                response::no_content()
            }
            Err(e) => response::internal(e),
        }
    }
}
