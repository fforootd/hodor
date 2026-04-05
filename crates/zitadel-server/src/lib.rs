pub mod assets;
pub mod health;
mod jobs;
pub mod openapi;
pub mod repo_bridge;
pub mod routing;

use axum::Router;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::net::TcpListener;
use zitadel_app::{ApplicationServices, HookPipeline};
use zitadel_config::Config;
use zitadel_db::DEFAULT_INSTANCE_ID;
use zitadel_fga::{FgaService, StoreResolver};

/// Shared server state accessible from handlers.
pub struct AppState {
    pub config: Config,
    pub db: zitadel_db::Db,
    pub secret_box: Arc<zitadel_crypto::SecretBox>,
    pub ready: AtomicBool,
    pub instance_resolver: Arc<routing::InstanceResolver>,
}

/// Build the full axum Router with all routes registered.
pub fn build_router(
    state: Arc<AppState>,
    api_state: zitadel_api::ApiState,
    oidc_state: zitadel_oidc::OidcState,
    login_state: zitadel_login::LoginState,
) -> Router {
    Router::new()
        // Runtime OpenAPI document
        .merge(openapi::routes(state.clone()))
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
        .layer(routing::InstanceContextLayer::new(
            state.instance_resolver.clone(),
        ))
        .layer(axum::middleware::from_fn(
            zitadel_observability::request_context_middleware,
        ))
}

/// Start the HTTP server and block until shutdown signal.
pub async fn run(config: Config) -> anyhow::Result<()> {
    // Open database.
    let db =
        zitadel_db::Db::open_with_config(&config.storage.stateful.url, &config.storage.stateful)
            .await?;
    run_with_db(config, db).await
}

pub async fn run_with_db(config: Config, db: zitadel_db::Db) -> anyhow::Result<()> {
    let port = config.server.port;
    tracing::info!(
        backend = %db.backend(),
        dialect = %db.dialect(),
        url = %config.storage.stateful.url,
        "database connected"
    );

    // Run migrations based on mode.
    match config.storage.stateful.resolve_migrate_mode() {
        "auto" => zitadel_db::migrate::migrate(&db).await?,
        "check" => zitadel_db::migrate::check_version(&db).await?,
        "skip" => tracing::info!("migration skipped"),
        _ => zitadel_db::migrate::migrate(&db).await?,
    }

    // Bootstrap (create default org + admin if empty).
    if config.storage.stateful.resolve_bootstrap_mode() == "auto" {
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

    zitadel_storage::prepare_postgres_role_databases(&config.storage, &db).await?;

    // TODO(CLAUDE-4): Pass Arc<HookPipeline> built from ActionRepository once
    // repo impls are merged. For now, pass None to disable the event consumer.
    jobs::start(&config, db.clone(), None).await?;

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
    let oidc_state = zitadel_oidc::OidcState::new_with_config(
        db.clone(),
        issuer.clone(),
        login_path,
        &config.oidc,
    );
    let storage = zitadel_storage::StorageRuntime::from_config(
        &config.storage,
        db.clone(),
        config.session.max_age_secs,
    )
    .await?;
    let fga = Arc::new(FgaService::new(db.clone()));
    fga.initialize_instance(DEFAULT_INSTANCE_ID).await?;
    fga.reconcile_root_hierarchy(DEFAULT_INSTANCE_ID).await?;

    // ── ADR-032: Build application layer ──
    // Build repository implementations from existing infrastructure.
    let repos = Arc::new(repo_bridge::build_repositories(
        db.clone(),
        storage.stateful.clone(),
        storage.transient.clone(),
        fga.clone(),
    ));

    // Build hook pipeline (empty for now — CLAUDE-3 will populate from ActionRepository).
    let hooks = Arc::new(HookPipeline::empty());

    // Build application services (all use cases wired to repos + hooks).
    let app = Arc::new(ApplicationServices::new(repos, hooks));
    tracing::info!("application services initialized (ADR-032)");

    let passwords = Arc::new(passwords);
    let cookie_config = Arc::new(cookie_config);

    let api_state = zitadel_api::ApiState {
        db: db.clone(),
        fga,
        stateful: storage.stateful.clone(),
        transient: storage.transient.clone(),
        analytics: storage.analytics.clone(),
        oidc: oidc_state.clone(),
        passwords: passwords.clone(),
        cookie_config: cookie_config.clone(),
        is_dev: config.is_dev(),
        app: app.clone(),
    };

    let login_state = zitadel_login::LoginState {
        db: db.clone(),
        stateful: storage.stateful.clone(),
        transient: storage.transient.clone(),
        passwords: passwords.clone(),
        cookie_config: cookie_config.clone(),
        public_origin: Arc::new(issuer.clone()),
        conformance_login_html: config.dev.conformance_login_html,
        rp: Arc::new(zitadel_oidc::rp::RpService::new(
            zitadel_oidc::rp::ReqwestHttpClient::new(),
            zitadel_oidc::rp::InMemoryIssuerMetadataCache::default(),
        )),
        pow_secret: config.server.management_secret.clone(),
        app: app.clone(),
    };

    let state = Arc::new(AppState {
        instance_resolver: Arc::new(
            routing::InstanceResolver::from_config(&config, db.clone()).await?,
        ),
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

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
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
