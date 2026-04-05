use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
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
    owner_org_id: String,
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

struct ManagementAccess {
    root_instance_id: String,
    owner_org_id: String,
    operator_admin: bool,
}

// owner_filter() removed — FGA checks via require_instance_relation handle authorization now.
// TODO(CLAUDE-4): Revisit if instance use cases need owner-scoped filtering.

async fn create(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<CreateRequest>,
) -> Response {
    let access = match require_root_management(&s, &identity).await {
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
            "cannot create instances for another root org",
        );
    }
    if !access.operator_admin {
        match root_relation_allowed(
            &s,
            &access.root_instance_id,
            &identity.user_id,
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

    match s.app.create_instance.execute(&ctx, cmd).await {
        Ok(instance) => {
            // FGA reconcile after create — the use case doesn't handle this yet.
            if let Err(error) = s
                .fga
                .reconcile_root_hierarchy(&access.root_instance_id)
                .await
            {
                return response::internal_error(format!("{error}"));
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
    let access = match require_root_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.user_id, "viewer", &id).await {
        Ok(false) => return response::not_found("instance not found"),
        Ok(true) => {}
        Err(response) => return response,
    }
    let ctx = response::build_actor_context(&identity);
    match s.app.get_instance.execute(&ctx, &id).await {
        Ok(instance) => response::json_ok(InstanceResponse::from(instance)),
        Err(e) => response::app_error(e),
    }
}

async fn list(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Query(p): Query<response::PaginationParams>,
) -> Response {
    let access = match require_root_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    let ctx = response::build_actor_context(&identity);
    let params = AppListParams {
        limit: Some(p.limit.min(200) as u32),
        cursor: p.cursor,
        search: None,
    };
    match s.app.list_instances.execute(&ctx, &params).await {
        Ok(result) => {
            // Filter by FGA relation — each instance is checked individually.
            // TODO: Push FGA filtering into the use case or repository layer.
            let mut filtered = Vec::new();
            for item in result.items {
                match require_instance_relation(
                    &s,
                    &access,
                    &identity.user_id,
                    "viewer",
                    &item.instance_id,
                )
                .await
                {
                    Ok(true) => filtered.push(item),
                    Ok(false) => {}
                    Err(resp) => return resp,
                }
            }
            let items: Vec<InstanceResponse> =
                filtered.into_iter().map(InstanceResponse::from).collect();
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
    Json(req): Json<UpdateRequest>,
) -> Response {
    let access = match require_root_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.user_id, "admin", &id).await {
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
    match s.app.update_instance.execute(&ctx, cmd).await {
        Ok(instance) => {
            // FGA reconcile after update — the use case doesn't handle this yet.
            if let Err(error) = s
                .fga
                .reconcile_root_hierarchy(&access.root_instance_id)
                .await
            {
                return response::internal_error(format!("{error}"));
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
    let access = match require_root_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.user_id, "admin", &id).await {
        Ok(false) => return response::not_found("instance not found"),
        Ok(true) => {}
        Err(response) => return response,
    }
    let ctx = response::build_actor_context(&identity);
    match s.app.deprovision_instance.execute(&ctx, &id).await {
        Ok(()) => {
            // FGA reconcile after deprovision — the use case doesn't handle this yet.
            if let Err(error) = s
                .fga
                .reconcile_root_hierarchy(&access.root_instance_id)
                .await
            {
                return response::internal_error(format!("{error}"));
            }
            response::no_content()
        }
        Err(e) => response::app_error(e),
    }
}

// ── Domain management (still uses direct DB — no domain use cases yet) ──
// TODO: Create domain use cases in zitadel-app and migrate these endpoints.

async fn list_domains(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(id): Path<String>,
) -> Response {
    let access = match require_root_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.user_id, "viewer", &id).await {
        Ok(false) => response::not_found("instance not found"),
        Ok(true) => match zitadel_db::list_instance_domains(&s.db, &id).await {
            Ok(items) => response::json_ok(serde_json::json!({ "items": items })),
            Err(error) => response::internal_error(format!("{error}")),
        },
        Err(response) => response,
    }
}

async fn add_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(id): Path<String>,
    Json(req): Json<AddDomainRequest>,
) -> Response {
    let access = match require_root_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    if req.domain.is_empty() {
        return response::bad_request("domain is required");
    }
    match require_instance_relation(&s, &access, &identity.user_id, "admin", &id).await {
        Ok(false) => response::not_found("instance not found"),
        Ok(true) => match zitadel_db::add_instance_domain(&s.db, &id, &req.domain).await {
            Ok(domain) => response::json_created(domain),
            Err(error) => response::bad_request(format!("domain already taken: {error}")),
        },
        Err(response) => response,
    }
}

async fn remove_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path((id, domain)): Path<(String, String)>,
) -> Response {
    let access = match require_root_management(&s, &identity).await {
        Ok(access) => access,
        Err(response) => return response,
    };
    match require_instance_relation(&s, &access, &identity.user_id, "admin", &id).await {
        Ok(false) => response::not_found("instance not found"),
        Ok(true) => match zitadel_db::delete_instance_domain(&s.db, &id, &domain).await {
            Ok(zitadel_db::DomainDeleteOutcome::Deleted) => response::no_content(),
            Ok(zitadel_db::DomainDeleteOutcome::NotFound) => {
                response::not_found("domain not found")
            }
            Ok(zitadel_db::DomainDeleteOutcome::PrimaryDomain) => {
                response::bad_request("cannot remove primary domain")
            }
            Err(error) => response::internal_error(format!("{error}")),
        },
        Err(response) => response,
    }
}

// ── Root management helpers (transport-level authorization) ──

/// Verify that the current request targets the root instance and build
/// a `ManagementAccess` token. Uses `get_instance` use case to look up
/// the current instance metadata.
async fn require_root_management(
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

    if instance.kind != "root" {
        return Err(response::error(
            StatusCode::FORBIDDEN,
            "instance management is only available from the root instance",
        ));
    }

    state
        .fga
        .reconcile_root_hierarchy(&instance.instance_id)
        .await
        .map_err(|error| response::internal_error(format!("{error}")))?;

    Ok(ManagementAccess {
        root_instance_id: instance.instance_id,
        owner_org_id: identity.org_id.clone(),
        operator_admin: identity.operator_admin,
    })
}

async fn require_instance_relation(
    state: &ApiState,
    access: &ManagementAccess,
    user_id: &str,
    relation: &str,
    instance_id: &str,
) -> Result<bool, Response> {
    if access.operator_admin {
        return Ok(true);
    }
    root_relation_allowed(
        state,
        &access.root_instance_id,
        user_id,
        relation,
        &format!("instance:{instance_id}"),
    )
    .await
}

async fn root_relation_allowed(
    state: &ApiState,
    root_instance_id: &str,
    user_id: &str,
    relation: &str,
    object: &str,
) -> Result<bool, Response> {
    state
        .fga
        .root_relation_allowed(root_instance_id, user_id, relation, object)
        .await
        .map_err(|error| response::internal_error(format!("{error}")))
}
