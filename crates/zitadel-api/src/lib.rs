pub mod actions;
pub mod analytics;
pub mod apps;
pub mod catalog;
pub mod console;
pub mod events;
pub mod fga;
pub mod groups;
pub mod login_flows;
pub mod middleware;
pub mod orgs;
pub mod pats;
pub mod projects;
pub mod providers;
pub mod response;
pub mod schemas;
pub mod search;
pub mod sessions;
pub mod settings;
pub mod telemetry;
pub mod users;

use axum::Router;
use std::sync::Arc;
use zitadel_auth::cookie::CookieConfig;
use zitadel_db::Db;
use zitadel_storage::{DefaultAnalyticsStorage, DefaultStatefulStorage, DefaultTransientStorage};

/// Shared API state.
#[derive(Clone)]
pub struct ApiState {
    pub db: Db,
    pub stateful: Arc<DefaultStatefulStorage>,
    pub transient: Arc<DefaultTransientStorage>,
    pub analytics: Arc<DefaultAnalyticsStorage>,
    pub passwords: Arc<zitadel_auth::password::Passwords>,
    pub cookie_config: Arc<CookieConfig>,
    pub is_dev: bool,
}

/// Build the REST API router with all /v1/* routes.
pub fn routes(state: ApiState) -> Router {
    let authed = Router::new()
        // Users
        .merge(users::routes())
        // Orgs
        .merge(orgs::routes())
        // Groups
        .merge(groups::routes())
        // Projects
        .merge(projects::routes())
        // Apps
        .merge(apps::routes())
        // Sessions (admin)
        .merge(sessions::routes())
        // PATs
        .merge(pats::routes())
        // Events
        .merge(events::routes())
        // Search
        .merge(search::routes())
        // Settings
        .merge(settings::routes())
        // Providers
        .merge(providers::routes())
        // Console bootstrap + counts
        .merge(console::routes())
        // Schemas + meta-schema
        .merge(schemas::routes())
        // Login flows
        .merge(login_flows::routes())
        // FGA / authorization
        .merge(fga::routes())
        // Analytics (query + schema browser)
        .merge(analytics::routes())
        // Actions
        .merge(actions::routes())
        // Telemetry (fingerprints)
        .merge(telemetry::routes())
        // Catalog / marketplace
        .merge(catalog::routes())
        // Auth middleware — validates Bearer token or session cookie.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth_gate,
        ));

    Router::new().nest("/v1", authed).with_state(state)
}
