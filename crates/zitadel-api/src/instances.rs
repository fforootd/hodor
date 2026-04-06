use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use zitadel_app::{FeatureMap, feature_enabled};
use zitadel_app::{
    instances::{CreateInstanceCommand, UpdateInstanceCommand},
    repo::{InstanceRecord, ListParams as AppListParams},
};

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/instances", get(list).post(create))
        .route(
            "/instances/{id}",
            get(get_one).patch(update).delete(delete_one),
        )
        .route(
            "/instances/{id}/domains",
            get(list_domains).post(add_domain),
        )
        .route(
            "/instances/{id}/domains/{domain}",
            axum::routing::delete(
                |state: State<ApiState>,
                 identity: Extension<Identity>,
                 path: Path<(String, String)>| async move {
                    remove_domain(state, identity, path).await
                },
            ),
        )
}

#[derive(Deserialize)]
struct CreateRequest {
    #[serde(default)]
    #[allow(dead_code)]
    instance_id: String,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    owner_org_id: String,
    #[serde(default)]
    placement_mode: String,
    #[serde(default)]
    region_key: String,
    #[serde(default)]
    kind: String,
}

#[derive(Deserialize)]
struct UpdateRequest {
    #[serde(default)]
    #[allow(dead_code)]
    state: String,
    #[serde(default)]
    placement_mode: String,
    #[serde(default)]
    region_key: String,
    #[serde(default)]
    feature_overrides: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct InstanceResponse {
    instance_id: String,
    primary_domain: Option<String>,
    state: String,
    kind: String,
    placement_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    region_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_org_id: Option<String>,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    feature_overrides: serde_json::Value,
    created_at: String,
    updated_at: String,
}

impl From<InstanceRecord> for InstanceResponse {
    fn from(r: InstanceRecord) -> Self {
        Self {
            instance_id: r.instance_id,
            primary_domain: r.primary_domain,
            state: r.state,
            kind: r.kind,
            placement_mode: r.placement_mode,
            region_key: r.region_key,
            owner_org_id: r.owner_org_id,
            feature_overrides: r.feature_overrides,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Deserialize)]
struct AddDomainRequest {
    #[serde(default)]
    domain: String,
}

pub(crate) struct ManagementAccess {
    pub(crate) parent_instance_id: String,
    pub(crate) owner_org_id: String,
    pub(crate) operator_admin: bool,
}

// owner_filter() removed — FGA checks via require_instance_relation handle authorization now.
// TODO(CLAUDE-4): Revisit if instance use cases need owner-scoped filtering.

async fn create(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<CreateRequest>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };

    if req.domain.is_empty() {
        return response::bad_request("domain is required");
    }

    let owner_org_id = if req.owner_org_id.is_empty() {
        identity.org_id.clone()
    } else {
        req.owner_org_id
    };
    if !access.operator_admin && owner_org_id != access.owner_org_id {
        return response::error(
            StatusCode::FORBIDDEN,
            "cannot create instances for another parent org",
        );
    }
    if !access.operator_admin {
        match parent_relation_allowed(
            &s,
            &access.parent_instance_id,
            &identity.principal_ref,
            "admin",
            &format!("org:{owner_org_id}"),
        )
        .await
        {
            Ok(true) => {}
            Ok(false) => {
                return response::error(
                    StatusCode::FORBIDDEN,
                    "you are not allowed to create instances for this org",
                );
            }
            Err(response) => return response,
        }
    }

    let placement_mode = if req.placement_mode.is_empty() {
        "global".to_string()
    } else {
        req.placement_mode
    };
    let kind = if req.kind.is_empty() {
        "managed".to_string()
    } else {
        req.kind
    };

    if kind != "managed" && kind != "federated" {
        return response::bad_request("kind must be 'managed' or 'federated'");
    }
    if placement_mode != "global" && placement_mode != "regional" {
        return response::bad_request("placement_mode must be 'global' or 'regional'");
    }

    let ctx = response::build_actor_context(&identity);
    let cmd = CreateInstanceCommand {
        kind,
        placement_mode,
        region_key: if req.region_key.is_empty() {
            None
        } else {
            Some(req.region_key)
        },
        owner_org_id,
        feature_overrides: serde_json::Value::Object(Default::default()),
        primary_domain: Some(req.domain),
    };

    match s
        .app
        .runner
        .run(&ctx, "instance.create", || {
            s.app.create_instance.execute(&ctx, cmd)
        })
        .await
    {
        Ok(instance) => {
            if let Err(resp) = reconcile_after_mutation(&s, &access.parent_instance_id).await {
                return resp;
            }
            response::json_created(InstanceResponse::from(instance))
        }
        Err(e) => response::app_error(e),
    }
}

async fn get_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(id): Path<String>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.principal_ref, "viewer", &id).await {
        Ok(false) => return response::not_found("instance not found"),
        Ok(true) => {}
        Err(response) => return response,
    }
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "instance.get", || {
            s.app.get_instance.execute(&ctx, &id)
        })
        .await
    {
        Ok(instance) => response::json_ok(InstanceResponse::from(instance)),
        Err(e) => response::app_error(e),
    }
}

async fn list(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(p): Query<response::PaginationParams>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };

    // Operator admins bypass FGA filtering entirely.
    if access.operator_admin {
        let ctx = response::build_actor_context(&identity);
        let params = AppListParams {
            limit: Some(p.limit.min(200) as u32),
            cursor: p.cursor,
            search: None,
        };
        return match s.app.list_instances.execute(&ctx, &params).await {
            Ok(result) => {
                let items: Vec<InstanceResponse> = result
                    .items
                    .into_iter()
                    .map(InstanceResponse::from)
                    .collect();
                response::json_ok(response::ListResponse {
                    items,
                    next_cursor: result.next_cursor,
                    total: result.total_count.map(|c| c as i64),
                })
            }
            Err(e) => response::app_error(e),
        };
    }

    // Loop through DB pages and batch-filter each page via FGA
    // (1 batch call per page instead of N individual checks per item).
    let ctx = response::build_actor_context(&identity);
    let requested_limit = p.limit.min(200) as usize;
    let mut cursor = p.cursor;
    let mut filtered = Vec::new();
    let mut total = None;

    loop {
        let params = AppListParams {
            limit: Some(requested_limit as u32),
            cursor: cursor.clone(),
            search: None,
        };
        let result = match s.app.list_instances.execute(&ctx, &params).await {
            Ok(result) => result,
            Err(e) => return response::app_error(e),
        };
        if total.is_none() {
            total = result.total_count.map(|c| c as i64);
        }

        let page_next_cursor = result.next_cursor.clone();

        // Batch-check FGA visibility for all items in this page.
        let instance_ids: Vec<String> = result
            .items
            .iter()
            .map(|item| item.instance_id.clone())
            .collect();

        let visible_ids = match s
            .fga
            .parent_batch_filter(
                &access.parent_instance_id,
                &identity.principal_ref,
                "viewer",
                "instance",
                &instance_ids,
            )
            .await
        {
            Ok(ids) => ids,
            Err(error) => return response::internal(error),
        };

        for item in result.items {
            if visible_ids.contains(&item.instance_id) {
                filtered.push(item);
                if filtered.len() == requested_limit {
                    let next_cursor = filtered.last().map(|r| r.instance_id.clone());
                    let items = filtered.into_iter().map(InstanceResponse::from).collect();
                    return response::json_ok(response::ListResponse {
                        items,
                        next_cursor,
                        total,
                    });
                }
            }
        }

        let Some(next_cursor) = page_next_cursor else {
            let items = filtered.into_iter().map(InstanceResponse::from).collect();
            return response::json_ok(response::ListResponse {
                items,
                next_cursor: None,
                total,
            });
        };
        cursor = Some(next_cursor);
    }
}

async fn update(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(id): Path<String>,
    Json(req): Json<UpdateRequest>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.principal_ref, "admin", &id).await {
        Ok(false) => return response::not_found("instance not found"),
        Ok(true) => {}
        Err(response) => return response,
    }

    let ctx = response::build_actor_context(&identity);
    let cmd = UpdateInstanceCommand {
        instance_id: id,
        placement_mode: if req.placement_mode.is_empty() {
            None
        } else {
            Some(req.placement_mode)
        },
        region_key: if req.region_key.is_empty() {
            None
        } else {
            Some(req.region_key)
        },
        feature_overrides: req.feature_overrides,
    };
    match s
        .app
        .runner
        .run(&ctx, "instance.update", || {
            s.app.update_instance.execute(&ctx, cmd)
        })
        .await
    {
        Ok(instance) => {
            if let Err(resp) = reconcile_after_mutation(&s, &access.parent_instance_id).await {
                return resp;
            }
            response::json_ok(InstanceResponse::from(instance))
        }
        Err(e) => response::app_error(e),
    }
}

async fn delete_one(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(id): Path<String>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.principal_ref, "admin", &id).await {
        Ok(false) => return response::not_found("instance not found"),
        Ok(true) => {}
        Err(response) => return response,
    }
    let ctx = response::build_actor_context(&identity);
    match s
        .app
        .runner
        .run(&ctx, "instance.deprovision", || {
            s.app.deprovision_instance.execute(&ctx, &id)
        })
        .await
    {
        Ok(()) => {
            if let Err(resp) = reconcile_after_mutation(&s, &access.parent_instance_id).await {
                return resp;
            }
            response::no_content()
        }
        Err(e) => response::app_error(e),
    }
}

// ─── Domain management ──────────────────────────────────────

async fn list_domains(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(id): Path<String>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.principal_ref, "viewer", &id).await {
        Ok(false) => response::not_found("instance not found"),
        Ok(true) => {
            let ctx = response::build_actor_context(&identity);
            match s
                .app
                .runner
                .run(&ctx, "instance.list_domains", || {
                    s.app.list_domains.execute(&ctx, &id)
                })
                .await
            {
                Ok(items) => response::json_ok(serde_json::json!({ "items": items })),
                Err(e) => response::app_error(e),
            }
        }
        Err(response) => response,
    }
}

async fn add_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(id): Path<String>,
    Json(req): Json<AddDomainRequest>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.principal_ref, "admin", &id).await {
        Ok(false) => response::not_found("instance not found"),
        Ok(true) => {
            let ctx = response::build_actor_context(&identity);
            let cmd = zitadel_app::instances::AddDomainCommand {
                instance_id: id,
                domain: req.domain,
            };
            match s
                .app
                .runner
                .run(&ctx, "instance.add_domain", || {
                    s.app.add_domain.execute(&ctx, cmd)
                })
                .await
            {
                Ok(domain) => response::json_created(domain),
                Err(e) => response::app_error(e),
            }
        }
        Err(response) => response,
    }
}

async fn remove_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path((id, domain)): Path<(String, String)>,
) -> Response {
    let access = match require_parent_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.principal_ref, "admin", &id).await {
        Ok(false) => response::not_found("instance not found"),
        Ok(true) => {
            let ctx = response::build_actor_context(&identity);
            match s
                .app
                .runner
                .run(&ctx, "instance.remove_domain", || {
                    s.app.remove_domain.execute(&ctx, &id, &domain)
                })
                .await
            {
                Ok(zitadel_app::repo::DomainRemoveResult::Deleted) => response::no_content(),
                Ok(zitadel_app::repo::DomainRemoveResult::NotFound) => {
                    response::not_found("domain not found")
                }
                Ok(zitadel_app::repo::DomainRemoveResult::PrimaryDomain) => {
                    response::bad_request("cannot remove primary domain")
                }
                Err(e) => response::app_error(e),
            }
        }
        Err(response) => response,
    }
}

// ── Parent management helpers (transport-level authorization) ──

/// Verify that the current request targets a parent-capable instance and build
/// a `ManagementAccess` token.
pub(crate) async fn require_parent_management(
    state: &ApiState,
    identity: &Identity,
) -> Result<ManagementAccess, Response> {
    let current_id = zitadel_db::current_instance_id();
    let ctx = response::build_actor_context(identity);
    let instance = state
        .app
        .get_instance
        .execute(&ctx, current_id.as_ref())
        .await
        .map_err(|e| response::app_error(e))?;

    let default_features =
        FeatureMap::from([("instance_management".into(), instance.kind == "root")]);
    let parent_management_enabled = feature_enabled(
        &default_features,
        &instance.feature_overrides,
        &["instance_management"],
        "instance_management",
    )
    .unwrap_or(instance.kind == "root")
        || instance
            .feature_overrides
            .get("instance_management")
            .and_then(|value| value.as_bool())
            .unwrap_or(false);

    if !parent_management_enabled {
        return Err(response::error(
            StatusCode::FORBIDDEN,
            "instance management is only available from a parent instance",
        ));
    }

    Ok(ManagementAccess {
        parent_instance_id: instance.instance_id,
        owner_org_id: identity.org_id.clone(),
        operator_admin: identity.operator_admin,
    })
}

/// Reconcile the FGA hierarchy after a write operation.
pub(crate) async fn reconcile_after_mutation(
    state: &ApiState,
    parent_instance_id: &str,
) -> Result<(), Response> {
    let _ = parent_instance_id;
    state
        .fga
        .rebuild_platform_store()
        .await
        .map_err(|error| response::internal(error))
}

pub(crate) async fn require_instance_relation(
    state: &ApiState,
    access: &ManagementAccess,
    principal_ref: &str,
    relation: &str,
    instance_id: &str,
) -> Result<bool, Response> {
    if access.operator_admin {
        return Ok(true);
    }
    parent_relation_allowed(
        state,
        &access.parent_instance_id,
        principal_ref,
        relation,
        &format!("instance:{instance_id}"),
    )
    .await
}

async fn parent_relation_allowed(
    state: &ApiState,
    parent_instance_id: &str,
    principal_ref: &str,
    relation: &str,
    object: &str,
) -> Result<bool, Response> {
    state
        .fga
        .parent_relation_allowed(parent_instance_id, principal_ref, relation, object)
        .await
        .map_err(|error| response::internal(error))
}
