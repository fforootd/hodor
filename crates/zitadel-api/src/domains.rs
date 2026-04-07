//! Customer-facing custom domain management endpoints.
//!
//! Instance-owned domains live at `/domains`.
//! Org-owned domains live at `/orgs/{org_id}/domains`.

use crate::{ApiState, middleware::Identity, response};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    response::Response,
    routing::get,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zitadel_app::{FeatureMap, feature_enabled};
use zitadel_db::{current_instance_id, load_instance_metadata};

const ALLOWED_INSTANCE_FEATURES: &[&str] = &["custom_domains"];

pub fn routes() -> Router<ApiState> {
    Router::new()
        .route(
            "/domains",
            get(list_instance_domains).post(add_instance_domain),
        )
        .route(
            "/domains/{domain}",
            get(get_instance_domain).delete(remove_instance_domain),
        )
        .route(
            "/domains/{domain}/verify",
            axum::routing::post(verify_instance_domain),
        )
        .route(
            "/orgs/{org_id}/domains",
            get(list_org_domains).post(add_org_domain),
        )
        .route(
            "/orgs/{org_id}/domains/{domain}",
            get(get_org_domain).delete(remove_org_domain),
        )
        .route(
            "/orgs/{org_id}/domains/{domain}/verify",
            axum::routing::post(verify_org_domain),
        )
}

#[derive(Deserialize)]
struct AddDomainRequest {
    domain: String,
    #[serde(default = "default_purpose")]
    purpose: String,
}

fn default_purpose() -> String {
    "served".to_string()
}

#[derive(Serialize)]
struct DomainResponse {
    instance_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    org_id: Option<String>,
    domain: String,
    purpose: String,
    is_primary: bool,
    state: String,
    verified: bool,
    verification_token: String,
    dns_challenge_host: String,
    dns_authorization_id: String,
    certificate_dns_record_name: String,
    certificate_dns_record_type: String,
    certificate_dns_record_value: String,
    certificate_state: String,
    certificate_id: String,
    certificate_map_entry: String,
    origin_trust_state: String,
    provisioning_error: String,
    created_at: String,
    updated_at: String,
}

impl From<zitadel_app::repo::DomainRecord> for DomainResponse {
    fn from(r: zitadel_app::repo::DomainRecord) -> Self {
        Self {
            instance_id: r.instance_id,
            org_id: r.org_id,
            domain: r.domain,
            purpose: r.purpose,
            is_primary: r.is_primary,
            state: r.state,
            verified: r.verified,
            verification_token: r.verification_token,
            dns_challenge_host: r.dns_challenge_host,
            dns_authorization_id: r.dns_authorization_id,
            certificate_dns_record_name: r.certificate_dns_record_name,
            certificate_dns_record_type: r.certificate_dns_record_type,
            certificate_dns_record_value: r.certificate_dns_record_value,
            certificate_state: r.certificate_state,
            certificate_id: r.certificate_id,
            certificate_map_entry: r.certificate_map_entry,
            origin_trust_state: r.origin_trust_state,
            provisioning_error: r.provisioning_error,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

async fn list_instance_domains(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
) -> Response {
    list_domains(&s, &identity, None).await
}

async fn list_org_domains(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    list_domains(&s, &identity, Some(required_path_param(&params, "org_id"))).await
}

async fn get_instance_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    get_domain(&s, &identity, None, required_path_param(&params, "domain")).await
}

async fn get_org_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    get_domain(
        &s,
        &identity,
        Some(required_path_param(&params, "org_id")),
        required_path_param(&params, "domain"),
    )
    .await
}

async fn add_instance_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Json(req): Json<AddDomainRequest>,
) -> Response {
    add_domain(&s, &identity, None, req).await
}

async fn add_org_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(params): Path<HashMap<String, String>>,
    Json(req): Json<AddDomainRequest>,
) -> Response {
    add_domain(
        &s,
        &identity,
        Some(required_path_param(&params, "org_id")),
        req,
    )
    .await
}

async fn verify_instance_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    verify_domain(&s, &identity, None, required_path_param(&params, "domain")).await
}

async fn verify_org_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    verify_domain(
        &s,
        &identity,
        Some(required_path_param(&params, "org_id")),
        required_path_param(&params, "domain"),
    )
    .await
}

async fn remove_instance_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    remove_domain(&s, &identity, None, required_path_param(&params, "domain")).await
}

async fn remove_org_domain(
    State(s): State<ApiState>,
    Extension(identity): Extension<Identity>,
    Path(params): Path<HashMap<String, String>>,
) -> Response {
    remove_domain(
        &s,
        &identity,
        Some(required_path_param(&params, "org_id")),
        required_path_param(&params, "domain"),
    )
    .await
}

async fn list_domains(state: &ApiState, identity: &Identity, org_id: Option<String>) -> Response {
    if let Err(response) = require_custom_domains_enabled(state).await {
        return response;
    }
    let ctx = response::build_actor_context(identity);
    let instance_id = current_instance_id();
    match state
        .app
        .list_custom_domains
        .execute(&ctx, &instance_id, org_id.as_deref())
        .await
    {
        Ok(items) => {
            let items: Vec<DomainResponse> = items.into_iter().map(DomainResponse::from).collect();
            response::json_ok(serde_json::json!({ "items": items }))
        }
        Err(e) => response::app_error(e),
    }
}

async fn get_domain(
    state: &ApiState,
    identity: &Identity,
    org_id: Option<String>,
    domain: String,
) -> Response {
    if let Err(response) = require_custom_domains_enabled(state).await {
        return response;
    }
    let ctx = response::build_actor_context(identity);
    let instance_id = current_instance_id();
    match state
        .app
        .get_custom_domain
        .execute(&ctx, &instance_id, org_id.as_deref(), &domain)
        .await
    {
        Ok(Some(record)) => response::json_ok(DomainResponse::from(record)),
        Ok(None) => response::not_found("domain not found"),
        Err(e) => response::app_error(e),
    }
}

async fn add_domain(
    state: &ApiState,
    identity: &Identity,
    org_id: Option<String>,
    req: AddDomainRequest,
) -> Response {
    if let Err(response) = require_custom_domains_enabled(state).await {
        return response;
    }
    let ctx = response::build_actor_context(identity);
    let instance_id = current_instance_id();
    let cmd = zitadel_app::domains::AddCustomDomainCommand {
        domain: req.domain,
        purpose: req.purpose,
        org_id,
    };
    match state
        .app
        .runner
        .run(&ctx, "domain.add", || {
            state.app.add_custom_domain.execute(&ctx, &instance_id, cmd)
        })
        .await
    {
        Ok(record) => response::json_created(DomainResponse::from(record)),
        Err(e) => response::app_error(e),
    }
}

async fn verify_domain(
    state: &ApiState,
    identity: &Identity,
    org_id: Option<String>,
    domain: String,
) -> Response {
    if let Err(response) = require_custom_domains_enabled(state).await {
        return response;
    }
    let ctx = response::build_actor_context(identity);
    let instance_id = current_instance_id();
    match state
        .app
        .runner
        .run(&ctx, "domain.verify", || {
            state
                .app
                .verify_custom_domain
                .execute(&ctx, &instance_id, org_id.as_deref(), &domain)
        })
        .await
    {
        Ok(record) => response::json_ok(DomainResponse::from(record)),
        Err(e) => response::app_error(e),
    }
}

async fn remove_domain(
    state: &ApiState,
    identity: &Identity,
    org_id: Option<String>,
    domain: String,
) -> Response {
    if let Err(response) = require_custom_domains_enabled(state).await {
        return response;
    }
    let ctx = response::build_actor_context(identity);
    let instance_id = current_instance_id();
    match state
        .app
        .runner
        .run(&ctx, "domain.remove", || {
            state
                .app
                .remove_custom_domain
                .execute(&ctx, &instance_id, org_id.as_deref(), &domain)
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

async fn require_custom_domains_enabled(state: &ApiState) -> Result<(), Response> {
    let instance_id = current_instance_id().into_owned();
    let metadata = match load_instance_metadata(&state.db, &instance_id).await {
        Ok(Some(metadata)) => metadata,
        Ok(None) => return Err(response::not_found("instance not found")),
        Err(error) => return Err(response::internal(error)),
    };
    let feature_overrides: serde_json::Value =
        serde_json::from_str(&metadata.feature_overrides_json).unwrap_or_default();
    let defaults = FeatureMap::from([("custom_domains".into(), false)]);
    let enabled = feature_enabled(
        &defaults,
        &feature_overrides,
        ALLOWED_INSTANCE_FEATURES,
        "custom_domains",
    )
    .unwrap_or(false);
    if enabled {
        Ok(())
    } else {
        Err(response::forbidden(
            "custom domains not enabled for this instance",
        ))
    }
}

fn required_path_param(params: &HashMap<String, String>, key: &str) -> String {
    params
        .get(key)
        .cloned()
        .unwrap_or_else(|| panic!("missing required path parameter: {key}"))
}
