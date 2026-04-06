pub mod account;
pub mod actions;
pub mod analytics;
pub mod apps;
pub mod extractors;
pub mod generic_named_resource;
pub mod auth;
pub mod catalog;
pub mod console;
pub mod events;
pub mod fga;
pub mod groups;
pub mod instances;
pub mod jobs;
pub mod login_flows;
pub mod middleware;
pub mod observability;
pub mod openapi;
pub mod orgs;
pub mod pats;
pub mod projects;
pub mod providers;
pub mod response;
pub mod schemas;
pub mod search;
pub mod sessions;
pub mod settings;
pub mod support;
pub mod telemetry;
pub mod users;

use axum::Router;
use std::sync::Arc;
use zitadel_app::ApplicationServices;
use zitadel_authn::cookie::CookieConfig;
use zitadel_db::Db;
use zitadel_fga::FgaService;
use zitadel_oidc::OidcState;
use zitadel_storage::{DefaultAnalyticsStorage, DefaultStatefulStorage, DefaultTransientStorage};

/// Shared API state.
#[derive(Clone)]
pub struct ApiState {
    pub db: Db,
    pub app: Arc<ApplicationServices>,
    pub fga: Arc<FgaService>,
    pub stateful: Arc<DefaultStatefulStorage>,
    pub transient: Arc<DefaultTransientStorage>,
    pub analytics: Arc<DefaultAnalyticsStorage>,
    pub oidc: OidcState,
    pub passwords: Arc<zitadel_authn::password::Swapper>,
    pub cookie_config: Arc<CookieConfig>,
    pub support_grant_secret: Arc<String>,
    pub is_dev: bool,
}

/// Lightweight FGA permission check for handlers that bypass the use-case layer.
/// Returns `Ok(())` if allowed, `Err(Response)` if denied.
pub async fn fga_check(
    state: &ApiState,
    ctx: &zitadel_app::ActorContext,
    relation: &str,
    object: &str,
) -> Result<(), axum::response::Response> {
    zitadel_app::authz::require_permission(&state.app.repos, ctx, relation, object)
        .await
        .map_err(response::app_error)
}

/// Build the REST API router with all /v1/* routes.
///
/// Product routes are mounted both flat (`/v1/users`) and nested under
/// instances (`/v1/instances/{instanceId}/users`). The InstanceResolver
/// extracts the instance ID from the URL path and sets `current_instance_id()`.
pub fn routes(state: ApiState) -> Router {
    // Product handlers — mounted twice: flat (root/self-hosted) and instance-scoped.
    let product_handlers = Router::new()
        .merge(users::routes())
        .merge(orgs::routes())
        .merge(groups::routes())
        .merge(projects::routes())
        .merge(apps::routes())
        .merge(sessions::routes())
        .merge(events::routes())
        .merge(search::routes())
        .merge(providers::routes())
        .merge(console::routes())
        .merge(schemas::routes())
        .merge(login_flows::routes())
        .merge(fga::customer_routes())
        .merge(observability::routes())
        .merge(actions::routes())
        .merge(telemetry::routes())
        .merge(catalog::routes());

    let scoped_product_handlers = product_handlers
        .clone()
        .route_layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::require_scoped_instance_access,
        ));

    let authed = Router::new()
        // Instance-scoped: /v1/instances/{instanceId}/users, etc.
        .nest("/instances/{instanceId}", scoped_product_handlers)
        // Instance management CRUD (operates on parent)
        .merge(instances::routes())
        .merge(auth::routes())
        .merge(account::routes())
        .merge(pats::routes())
        .merge(jobs::routes())
        .merge(settings::routes())
        .merge(support::routes())
        .merge(fga::internal_platform_routes())
        .merge(analytics::routes())
        // Flat product routes (root / self-hosted single-instance)
        .merge(product_handlers)
        // Auth middleware
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth_gate,
        ));

    let public = Router::new().merge(telemetry::public_routes());

    let v1 = authed.merge(public);

    Router::new().nest("/v1", v1).with_state(state)
}
