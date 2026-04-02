pub mod assets;
pub mod health;

use axum::Router;
use zitadel_config::Config;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

/// Shared server state accessible from handlers.
pub struct AppState {
    pub config: Config,
    pub db: zitadel_db::Db,
    pub ready: AtomicBool,
}

/// Build the full axum Router with all routes registered.
pub fn build_router(state: Arc<AppState>, api_state: zitadel_api::ApiState, oidc_state: zitadel_oidc::OidcState, login_state: zitadel_login::LoginState) -> Router {
    Router::new()
        // Health probes
        .merge(health::routes(state.clone()))
        // OIDC provider (discovery, authorize, token, userinfo, JWKS)
        .merge(zitadel_oidc::routes(oidc_state))
        // Login flows (public, no auth required)
        .merge(zitadel_login::routes(login_state))
        // REST API (/v1/*)
        .merge(zitadel_api::routes(api_state))
        // Static frontend assets + SPA fallback
        .merge(assets::routes())
        // Middleware
        .layer(TraceLayer::new_for_http())
}

/// Start the HTTP server and block until shutdown signal.
pub async fn run(config: Config) -> anyhow::Result<()> {
    let port = config.server.port;

    // Open database.
    let db = zitadel_db::Db::open_with_config(&config.database.url, &config.database).await?;
    tracing::info!(dialect = %db.dialect(), url = %config.database.url, "database connected");

    // Run migrations based on mode.
    match config.database.resolve_migrate_mode() {
        "auto" => zitadel_db::migrate::migrate(&db).await?,
        "check" => zitadel_db::migrate::check_version(&db).await?,
        "skip" => tracing::info!("migration skipped"),
        _ => zitadel_db::migrate::migrate(&db).await?,
    }

    // Bootstrap (create default org + admin if empty).
    if config.database.resolve_bootstrap_mode() == "auto" {
        zitadel_db::bootstrap::bootstrap(&db).await?;
    }

    // Apply seed file if configured.
    if !config.dev.seed_file.is_empty() {
        let seed_path = std::path::Path::new(&config.dev.seed_file);
        if seed_path.exists() {
            zitadel_db::seed::apply(&db, seed_path).await?;
        } else {
            tracing::warn!(path = %config.dev.seed_file, "seed file not found, skipping");
        }
    }

    // Build cookie config.
    let cookie_config = zitadel_auth::cookie::CookieConfig::new(
        config.server.cookie_secrets.clone(),
        &config.server.external_domain,
    );

    // Build password hasher.
    let passwords = if config.is_dev() {
        zitadel_auth::password::Passwords::new_dev()
    } else {
        zitadel_auth::password::Passwords::new()
    };

    // OIDC provider.
    let issuer = format!("http://{}:{}", config.server.external_domain, config.server.port);
    let oidc_state = zitadel_oidc::OidcState::new(db.clone(), issuer).await?;

    let api_state = zitadel_api::ApiState {
        db: db.clone(),
        passwords: Arc::new(passwords),
        cookie_config: Arc::new(cookie_config),
        is_dev: config.is_dev(),
    };

    let login_state = zitadel_login::LoginState {
        db: db.clone(),
        passwords: api_state.passwords.clone(),
        cookie_config: api_state.cookie_config.clone(),
    };

    let state = Arc::new(AppState {
        config,
        db,
        ready: AtomicBool::new(false),
    });

    let app = build_router(state.clone(), api_state, oidc_state, login_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;

    // Mark ready after binding.
    state.ready.store(true, Ordering::SeqCst);

    tracing::info!("Zitadel server listening on http://{}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("shutdown signal received, starting graceful shutdown");
}
