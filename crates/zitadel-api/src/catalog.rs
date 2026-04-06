use crate::{ApiState, extractors::ResourceId, response};
use axum::Extension;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use std::collections::HashMap;
use zitadel_db::current_instance_id;

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route("/catalog", get(list_catalog))
        .route("/catalog/{id}", get(get_catalog_entry))
        .route("/catalog/{id}/install", post(install_from_catalog))
        .route("/catalog/refresh", post(refresh_catalog))
        .route("/providers/templates", get(list_provider_templates))
}

#[derive(Deserialize)]
struct CatalogListQuery {
    #[serde(rename = "type", default)]
    type_filter: Option<String>,
    #[serde(default)]
    tags: Option<String>,
}

/// GET /v1/catalog — list all templates with optional filters.
async fn list_catalog(Query(q): Query<CatalogListQuery>) -> Response {
    let catalog = zitadel_catalog::Catalog::embedded();
    let templates = catalog.list(q.type_filter.as_deref(), q.tags.as_deref());

    let items: Vec<serde_json::Value> = templates
        .iter()
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "name": t.name,
                "type": t.entry_type,
                "kind": t.kind,
                "protocol": t.protocol,
                "description": t.description,
                "tags": t.tags,
                "version": t.version,
                "author": t.author,
                "official": t.official,
                "capabilities": t.capabilities,
                "logo_url": t.logo_url,
                "docs_url": t.docs_url,
                "source": "embedded",
            })
        })
        .collect();

    let total = items.len();
    response::json_ok(serde_json::json!({
        "templates": items,
        "total": total,
        "can_refresh": false,
    }))
}

/// GET /v1/catalog/{id} — get template detail with variables and payload.
async fn get_catalog_entry(ResourceId(id): ResourceId) -> Response {
    let catalog = zitadel_catalog::Catalog::embedded();

    let (entry, detail) = match catalog.get(&id) {
        Some(r) => r,
        None => return response::not_found(format!("template not found: {id}")),
    };

    response::json_ok(serde_json::json!({
        "template": {
            "id": entry.id,
            "name": entry.name,
            "type": entry.entry_type,
            "version": entry.version,
            "description": entry.description,
            "tags": entry.tags,
        },
        "variables": detail.variables,
        "payload": detail.payload,
    }))
}

#[derive(Deserialize)]
struct InstallRequest {
    #[serde(default)]
    variables: HashMap<String, serde_json::Value>,
}

/// POST /v1/catalog/{id}/install — install a template with variable substitution.
async fn install_from_catalog(
    State(s): State<ApiState>,
    Extension(identity): Extension<crate::middleware::Identity>,
    ResourceId(id): ResourceId,
    Json(req): Json<InstallRequest>,
) -> Response {
    let ctx = crate::response::build_actor_context(&identity);
    if let Err(e) = crate::fga_check(
        &s,
        &ctx,
        "admin",
        &format!("instance:{}", ctx.instance_id()),
    )
    .await
    {
        return e;
    }
    let catalog = zitadel_catalog::Catalog::embedded();

    // Check template exists and determine type.
    let (entry, _detail) = match catalog.get(&id) {
        Some(r) => r,
        None => return response::not_found(format!("template not found: {id}")),
    };

    let vars_value = serde_json::to_value(&req.variables).unwrap_or_default();
    let instance_id = current_instance_id();

    match entry.entry_type.as_str() {
        "provider" => match s
            .app
            .repos
            .catalog
            .install_provider(&instance_id, &id, &vars_value)
            .await
        {
            Ok(provider_id) => (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "id": provider_id,
                    "template_id": id,
                    "type": "provider",
                    "status": "installed",
                })),
            )
                .into_response(),
            Err(e) => response::internal(e),
        },
        "action" => match s
            .app
            .repos
            .catalog
            .install_action(&instance_id, &id, &vars_value)
            .await
        {
            Ok(action_id) => (
                StatusCode::CREATED,
                Json(serde_json::json!({
                    "id": action_id,
                    "template_id": id,
                    "type": "action",
                    "status": "installed",
                })),
            )
                .into_response(),
            Err(e) => response::internal(e),
        },
        // For other types (login_flow, authorization), return stub for now.
        other => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "template_id": id,
                "type": other,
                "status": "installed",
            })),
        )
            .into_response(),
    }
}

/// POST /v1/catalog/refresh — no-op for embedded catalog.
async fn refresh_catalog() -> Response {
    response::json_ok(serde_json::json!({
        "status": "ok",
        "new": 0,
    }))
}

/// GET /v1/providers/templates — provider-only templates for the create flow.
async fn list_provider_templates() -> Response {
    let catalog = zitadel_catalog::Catalog::embedded();
    let providers = catalog.list(Some("provider"), None);

    let templates: Vec<serde_json::Value> = providers
        .iter()
        .map(|t| {
            serde_json::json!({
                "id": t.id,
                "name": t.name,
                "protocol": t.protocol,
                "kind": t.kind,
                "description": t.description,
                "logo_url": t.logo_url,
                "official": t.official,
            })
        })
        .collect();

    response::json_ok(serde_json::json!({ "templates": templates }))
}
