pub mod assets;
pub mod health;

use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::TcpListener;
use zitadel_config::Config;

/// Shared server state accessible from handlers.
pub struct AppState {
    pub config: Config,
    pub db: zitadel_db::Db,
    pub secret_box: Arc<zitadel_crypto::SecretBox>,
    pub ready: AtomicBool,
}

/// Build the full axum Router with all routes registered.
pub fn build_router(
    state: Arc<AppState>,
    api_state: zitadel_api::ApiState,
    oidc_state: zitadel_oidc::OidcState,
    login_state: zitadel_login::LoginState,
) -> Router {
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
        .layer(axum::middleware::from_fn(
            zitadel_observability::request_context_middleware,
        ))
}

/// Start the HTTP server and block until shutdown signal.
pub async fn run(config: Config) -> anyhow::Result<()> {
    // Open database.
    let db = zitadel_db::Db::open_with_config(&config.database.url, &config.database).await?;
    run_with_db(config, db).await
}

pub async fn run_with_db(config: Config, db: zitadel_db::Db) -> anyhow::Result<()> {
    let port = config.server.port;
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

    // Build encryption secret box from config (plaintext passthrough if no keys configured).
    let encryption_keys: std::collections::HashMap<String, String> = config
        .encryption
        .keys
        .iter()
        .map(|k| (k.id.clone(), k.secret.clone()))
        .collect();
    let secret_box = Arc::new(
        zitadel_crypto::SecretBox::new(&config.encryption.active_key_id, &encryption_keys)
            .expect("invalid encryption key config"),
    );
    if secret_box.plaintext() {
        tracing::warn!("no encryption keys configured — secrets stored in plaintext (dev mode)");
    }

    // Build cookie config with session max age from config.
    let cookie_config = zitadel_authn::cookie::CookieConfig::new_with_max_age(
        config.server.cookie_secrets.clone(),
        &config.server.external_domain,
        config.server.force_insecure_cookies,
        config.session.max_age_secs as i64,
    );

    // Build password swapper from config (verifies any supported algorithm, re-hashes to preferred).
    let hasher_config = if config.is_dev() {
        zitadel_config::password::PasswordHasherConfig::dev_defaults()
    } else {
        config.password_hasher.clone()
    };
    let passwords = zitadel_authn::password::Swapper::from_config(&hasher_config);

    // OIDC provider.
    let issuer = if config.server.public_origin.is_empty() {
        format!(
            "http://{}:{}",
            config.server.external_domain, config.server.port
        )
    } else {
        config
            .server
            .public_origin
            .trim_end_matches('/')
            .to_string()
    };
    let login_path = if config.dev.conformance_login_html {
        "/conformance/login".to_string()
    } else {
        "/login".to_string()
    };
    let oidc_state =
        zitadel_oidc::OidcState::new_with_config(db.clone(), issuer.clone(), login_path, &config.oidc);

    let stateful = Arc::new(zitadel_storage::DefaultStatefulStorage::new(
        zitadel_storage::SqlStateDb::new(db.clone()),
        zitadel_storage::SqlEdgeReadDb::new(db.clone()),
    ));
    let transient = Arc::new(zitadel_storage::DefaultTransientStorage::new(
        zitadel_storage::SqlTransientCompatKv::new(db.clone()),
        zitadel_storage::NoopEdgeSink,
    ));
    let analytics = Arc::new(zitadel_storage::DefaultAnalyticsStorage::new(
        zitadel_storage::NoopAnalyticsSink,
        zitadel_storage::SqlAnalyticsQueryBackend::new(db.clone()),
    ));

    let api_state = zitadel_api::ApiState {
        db: db.clone(),
        stateful: stateful.clone(),
        transient: transient.clone(),
        analytics,
        oidc: oidc_state.clone(),
        passwords: Arc::new(passwords),
        cookie_config: Arc::new(cookie_config),
        is_dev: config.is_dev(),
    };

    let login_state = zitadel_login::LoginState {
        db: db.clone(),
        stateful,
        transient,
        passwords: api_state.passwords.clone(),
        cookie_config: api_state.cookie_config.clone(),
        public_origin: Arc::new(issuer.clone()),
        conformance_login_html: config.dev.conformance_login_html,
        rp: Arc::new(zitadel_oidc::rp::RpService::new(
            zitadel_oidc::rp::ReqwestHttpClient::new(),
            zitadel_oidc::rp::InMemoryIssuerMetadataCache::default(),
        )),
    };

    let state = Arc::new(AppState {
        config,
        db,
        secret_box,
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
