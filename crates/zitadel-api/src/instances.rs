use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use zitadel_db::{
    CreateManagedInstanceInput, DomainDeleteOutcome, ManagedInstancePatch, add_instance_domain,
    create_managed_instance, deprovision_managed_instance, delete_instance_domain,
    get_managed_instance, instance_visible, list_instance_domains, list_managed_instances,
    load_instance_metadata, update_managed_instance, validate_feature_overrides,
};

const ALLOWED_INSTANCE_FEATURES: &[&str] = &["instance_management", "billing"];

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

impl ManagementAccess {
    fn owner_filter(&self) -> Option<&str> {
        if self.operator_admin {
            None
        } else {
            Some(self.owner_org_id.as_str())
        }
    }
}

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

    let instance_id = if req.instance_id.is_empty() {
        Uuid::new_v4().to_string()
    } else {
        req.instance_id
    };
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

    let region_key = if req.region_key.is_empty() {
        None
    } else {
        Some(req.region_key)
    };

    let input = CreateManagedInstanceInput {
        instance_id,
        root_instance_id: access.root_instance_id.clone(),
        owner_org_id,
        primary_domain: req.domain,
        kind,
        placement_mode,
        region_key,
    };

    match create_managed_instance(&s.db, &input).await {
        Ok(instance) => {
            if let Err(error) = s.fga.reconcile_root_hierarchy(&access.root_instance_id).await {
                return response::internal_error(format!("{error}"));
            }
            response::json_created(instance_response(instance))
        }
        Err(error) => response::bad_request(format!("{error}")),
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
    match get_managed_instance(&s.db, &id, &access.root_instance_id, access.owner_filter()).await {
        Ok(Some(instance)) => match require_instance_relation(
            &s,
            &access,
            &identity.user_id,
            "viewer",
            &id,
        )
        .await
        {
            Ok(true) => response::json_ok(instance_response(instance)),
            Ok(false) => response::not_found("instance not found"),
            Err(response) => response,
        },
        Ok(None) => response::not_found("instance not found"),
        Err(error) => response::internal_error(format!("{error}")),
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
    let cursor = p.cursor.unwrap_or_default();
    let limit = p.limit.min(200);
    match list_managed_instances(
        &s.db,
        &access.root_instance_id,
        access.owner_filter(),
        &cursor,
        limit + 1,
    )
    .await
    {
        Ok(rows) => {
            let mut filtered = Vec::new();
            for row in rows {
                match require_instance_relation(
                    &s,
                    &access,
                    &identity.user_id,
                    "viewer",
                    &row.instance_id,
                )
                .await
                {
                    Ok(true) => filtered.push(row),
                    Ok(false) => {}
                    Err(response) => return response,
                }
            }
            let has_more = filtered.len() as i64 > limit;
            let items: Vec<InstanceResponse> = filtered
                .into_iter()
                .take(limit as usize)
                .map(instance_response)
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
    if !req.placement_mode.is_empty()
        && req.placement_mode != "global"
        && req.placement_mode != "regional"
    {
        return response::bad_request("placement_mode must be 'global' or 'regional'");
    }
    let mut patch = ManagedInstancePatch::default();
    if !req.state.is_empty() {
        patch.state = Some(req.state);
    }
    if !req.placement_mode.is_empty() {
        patch.placement_mode = Some(req.placement_mode);
    }
    if !req.region_key.is_empty() {
        patch.region_key = Some(req.region_key);
    }
    if let Some(ref overrides) = req.feature_overrides {
        if let Err(error) = validate_feature_overrides(overrides, ALLOWED_INSTANCE_FEATURES) {
            return response::bad_request(error.to_string());
        }
        patch.feature_overrides_json = Some(response::to_json_string(overrides));
    }

    if patch == ManagedInstancePatch::default() {
        return response::bad_request("no fields to update");
    }
    match require_instance_relation(&s, &access, &identity.user_id, "admin", &id).await {
        Ok(false) => return response::not_found("instance not found"),
        Ok(true) => {}
        Err(response) => return response,
    }

    match update_managed_instance(
        &s.db,
        &id,
        &access.root_instance_id,
        access.owner_filter(),
        &patch,
    )
    .await
    {
        Ok(false) => response::not_found("instance not found"),
        Ok(true) => match get_managed_instance(
            &s.db,
            &id,
            &access.root_instance_id,
            access.owner_filter(),
        )
        .await
        {
            Ok(Some(instance)) => {
                if let Err(error) = s.fga.reconcile_root_hierarchy(&access.root_instance_id).await
                {
                    return response::internal_error(format!("{error}"));
                }
                response::json_ok(instance_response(instance))
            }
            Ok(None) => response::not_found("instance not found"),
            Err(error) => response::internal_error(format!("{error}")),
        },
        Err(error) => response::internal_error(format!("{error}")),
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
    match deprovision_managed_instance(&s.db, &id, &access.root_instance_id, access.owner_filter()).await {
        Ok(false) => response::not_found("instance not found"),
        Ok(true) => {
            if let Err(error) = s.fga.reconcile_root_hierarchy(&access.root_instance_id).await {
                return response::internal_error(format!("{error}"));
            }
            response::no_content()
        }
        Err(error) => response::internal_error(format!("{error}")),
    }
}

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
        Ok(true) => match instance_visible(&s.db, &id, &access.root_instance_id, access.owner_filter()).await {
            Ok(false) => response::not_found("instance not found"),
            Ok(true) => match list_instance_domains(&s.db, &id).await {
                Ok(items) => response::json_ok(serde_json::json!({ "items": items })),
                Err(error) => response::internal_error(format!("{error}")),
            },
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
        Ok(true) => match instance_visible(&s.db, &id, &access.root_instance_id, access.owner_filter()).await {
            Ok(false) => response::not_found("instance not found"),
            Ok(true) => match add_instance_domain(&s.db, &id, &req.domain).await {
                Ok(domain) => response::json_created(domain),
                Err(error) => response::bad_request(format!("domain already taken: {error}")),
            },
            Err(error) => response::internal_error(format!("{error}")),
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
        Ok(true) => match instance_visible(&s.db, &id, &access.root_instance_id, access.owner_filter()).await {
            Ok(false) => response::not_found("instance not found"),
            Ok(true) => match delete_instance_domain(&s.db, &id, &domain).await {
                Ok(DomainDeleteOutcome::Deleted) => response::no_content(),
                Ok(DomainDeleteOutcome::NotFound) => response::not_found("domain not found"),
                Ok(DomainDeleteOutcome::PrimaryDomain) => {
                    response::bad_request("cannot remove primary domain")
                }
                Err(error) => response::internal_error(format!("{error}")),
            },
            Err(error) => response::internal_error(format!("{error}")),
        },
        Err(response) => response,
    }
}

async fn require_root_management(
    state: &ApiState,
    identity: &Identity,
) -> Result<ManagementAccess, Response> {
    match load_instance_metadata(&state.db, zitadel_db::current_instance_id().as_ref())
        .await
        .map_err(|error| response::internal_error(format!("{error}")))?
    {
        Some(instance) if instance.kind == "root" => {
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
        Some(_) => Err(response::error(
            StatusCode::FORBIDDEN,
            "instance management is only available from the root instance",
        )),
        None => Err(response::not_found("current instance not found")),
    }
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

fn instance_response(row: zitadel_db::ManagedInstanceRecord) -> InstanceResponse {
    InstanceResponse {
        instance_id: row.instance_id,
        state: row.state,
        kind: row.kind,
        placement_mode: row.placement_mode,
        region_key: row.region_key,
        owner_org_id: row.owner_org_id,
        feature_overrides: serde_json::from_str(&row.feature_overrides_json).unwrap_or_default(),
        created_at: row.created_at,
        updated_at: row.updated_at,
        primary_domain: row.primary_domain,
    }
}
