pub mod middleware;
pub mod users;
pub mod orgs;
pub mod groups;
pub mod projects;
pub mod apps;
pub mod sessions;
pub mod pats;
pub mod events;
pub mod search;
pub mod response;

use axum::Router;
use hodor_auth::cookie::CookieConfig;
use hodor_db::Db;
use std::sync::Arc;

/// Shared API state.
#[derive(Clone)]
pub struct ApiState {
    pub db: Db,
    pub passwords: Arc<hodor_auth::password::Passwords>,
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
        // Auth middleware — validates Bearer token or session cookie.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::auth_gate,
        ));

    Router::new()
        .nest("/v1", authed)
        .with_state(state)
}
