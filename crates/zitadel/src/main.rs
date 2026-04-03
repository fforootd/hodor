use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "zitadel", about = "Zitadel identity platform", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the HTTP server.
    Start {
        /// Path to TOML config file.
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Enable embedded mock OIDC provider.
        #[arg(long)]
        mock_oidc: bool,

        /// Path to YAML seed file loaded on startup.
        #[arg(long)]
        seed: Option<PathBuf>,

        /// Skip automatic database migration.
        #[arg(long)]
        skip_migrate: bool,
    },

    /// Run database migrations.
    Migrate {
        /// Path to TOML config file.
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Print current schema version and exit.
        #[arg(long)]
        status: bool,

        /// Bootstrap the default org/admin after migrations.
        #[arg(long)]
        bootstrap: bool,
    },

    /// Manage declarative seed files.
    Seed {
        #[command(subcommand)]
        action: SeedAction,
    },

    /// Export OpenAPI 3.1 spec to stdout.
    OpenapiExport,

    /// Print the JSON Schema for the TOML configuration file.
    ConfigSchema,

    /// Validate a config file and print the resolved configuration.
    ConfigValidate {
        /// Path to TOML config file.
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum SeedAction {
    /// Apply a seed file to the database.
    Apply {
        /// Path to TOML config file.
        #[arg(short, long)]
        config: Option<PathBuf>,

        /// Path to YAML seed file.
        #[arg(long)]
        file: PathBuf,
    },

    /// Validate a seed file without touching the database.
    Validate {
        /// Path to YAML seed file.
        #[arg(long)]
        file: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Start {
            config,
            mock_oidc,
            seed,
            skip_migrate,
        } => {
            let mut cfg = load_config(config.as_deref())?;

            // Resolve relative paths in config relative to the config file directory.
            resolve_paths(&mut cfg, config.as_deref());

            // CLI flag overrides.
            if mock_oidc {
                cfg.dev.mock_oidc = true;
            }
            if let Some(seed_path) = seed {
                cfg.dev.seed_file = seed_path.to_string_lossy().into_owned();
            }
            if skip_migrate {
                cfg.database.migrate = "skip".into();
            }

            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async move {
                let db = zitadel_db::Db::open_with_config(&cfg.database.url, &cfg.database).await?;
                let _observability =
                    zitadel_observability::install(&cfg.observability, Some(db.clone())).await?;
                tracing::info!(
                    port = cfg.server.port,
                    db = %cfg.database.url,
                    "starting zitadel server"
                );
                zitadel_server::run_with_db(cfg, db).await
            })?;
        }

        Commands::Migrate {
            config,
            status,
            bootstrap,
        } => {
            let mut cfg = load_config(config.as_deref())?;
            resolve_paths(&mut cfg, config.as_deref());
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(async move {
                let db = zitadel_db::Db::open_with_config(&cfg.database.url, &cfg.database).await?;
                let _observability =
                    zitadel_observability::install(&cfg.observability, Some(db.clone())).await?;

                if status {
                    zitadel_db::migrate::check_version(&db).await?;
                    tracing::info!("schema is up to date");
                } else {
                    zitadel_db::migrate::migrate(&db).await?;
                    if bootstrap {
                        let changed = zitadel_db::bootstrap::bootstrap(&db).await?;
                        tracing::info!(bootstrapped = changed, "migration command completed");
                    } else {
                        tracing::info!("migration command completed");
                    }
                }
                db.close().await;
                anyhow::Ok(())
            })?;
        }

        Commands::Seed { action } => match action {
            SeedAction::Apply { config, file } => {
                let mut cfg = load_config(config.as_deref())?;
                resolve_paths(&mut cfg, config.as_deref());
                let rt = tokio::runtime::Runtime::new()?;
                let file = if file.is_absolute() {
                    file
                } else {
                    std::env::current_dir()?.join(file)
                };
                rt.block_on(async move {
                    let db =
                        zitadel_db::Db::open_with_config(&cfg.database.url, &cfg.database).await?;
                    let _observability =
                        zitadel_observability::install(&cfg.observability, Some(db.clone()))
                            .await?;
                    zitadel_db::seed::apply(&db, &file).await?;
                    db.close().await;
                    tracing::info!(file = %file.display(), "seed applied");
                    anyhow::Ok(())
                })?;
            }
            SeedAction::Validate { file } => {
                let file = if file.is_absolute() {
                    file
                } else {
                    std::env::current_dir()?.join(file)
                };
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(async move {
                    let cfg = zitadel_config::Config::default();
                    let _observability =
                        zitadel_observability::install(&cfg.observability, None).await?;
                    let seed = zitadel_db::seed::validate(&file)?;
                    tracing::info!(file = %file.display(), users = seed.users.len(), "seed valid");
                    anyhow::Ok(())
                })?;
            }
        },

        Commands::OpenapiExport => {
            eprintln!("OpenAPI export not yet implemented");
        }

        Commands::ConfigSchema => {
            println!("{}", zitadel_config::Config::json_schema_string());
        }

        Commands::ConfigValidate { config } => {
            let mut cfg = load_config(config.as_deref())?;
            resolve_paths(&mut cfg, config.as_deref());
            let json = serde_json::to_string_pretty(&cfg)?;
            println!("{json}");
            eprintln!("Config is valid.");
            eprintln!("  Database:        {}", cfg.database.url);
            eprintln!("  Server:          {}:{}", cfg.server.external_domain, cfg.server.port);
            eprintln!("  Encryption:      {}", if cfg.encryption.keys.is_empty() { "plaintext (no keys)" } else { "active" });
            eprintln!("  Password hasher: {:?}", cfg.password_hasher.algorithm);
            eprintln!("  OIDC signing:    {:?}", cfg.oidc.signing_algorithm);
            eprintln!("  Session max age: {}s", cfg.session.max_age_secs);
            eprintln!("  Dev mode:        {}", cfg.is_dev());
        }
    }

    Ok(())
}

fn load_config(path: Option<&std::path::Path>) -> anyhow::Result<zitadel_config::Config> {
    let cfg = zitadel_config::Config::load(path)?;
    Ok(cfg)
}

/// Resolve relative paths (database URL, seed file, cache path) relative to the config file directory.
fn resolve_paths(cfg: &mut zitadel_config::Config, config_path: Option<&std::path::Path>) {
    let base_dir = config_path
        .and_then(|p| std::fs::canonicalize(p).ok())
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    // Resolve sqlite:// relative paths.
    if let Some(path) = cfg.database.url.strip_prefix("sqlite://") {
        if !path.is_empty() && path != ":memory:" {
            let p = std::path::Path::new(path);
            let joined = if p.is_absolute() {
                p.to_path_buf()
            } else {
                base_dir.join(path)
            };
            // Normalize away any ".." components.
            let resolved = normalize_path(&joined);
            if let Some(parent) = resolved.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            cfg.database.url = format!("sqlite://{}", resolved.display());
        }
    }

    // Resolve seed file path.
    if !cfg.dev.seed_file.is_empty() && !std::path::Path::new(&cfg.dev.seed_file).is_absolute() {
        // Try CWD first, then config dir.
        let cwd_path = std::env::current_dir()
            .unwrap_or_default()
            .join(&cfg.dev.seed_file);
        if cwd_path.exists() {
            cfg.dev.seed_file = cwd_path.to_string_lossy().into_owned();
        } else {
            let config_path = base_dir.join(&cfg.dev.seed_file);
            cfg.dev.seed_file = config_path.to_string_lossy().into_owned();
        }
    }

    // Resolve cache path.
    if !cfg.observability.cache_path.is_empty()
        && !std::path::Path::new(&cfg.observability.cache_path).is_absolute()
    {
        let resolved = base_dir.join(&cfg.observability.cache_path);
        cfg.observability.cache_path = resolved.to_string_lossy().into_owned();
    }
}

/// Normalize a path by resolving `.` and `..` components without requiring the path to exist.
fn normalize_path(path: &std::path::Path) -> PathBuf {
    use std::path::Component;
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                components.pop();
            }
            Component::CurDir => {}
            c => components.push(c),
        }
    }
    components.iter().collect()
}
