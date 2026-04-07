use std::path::{Path, PathBuf};

use crate::cli::*;

pub(crate) fn run_start(args: StartArgs) -> anyhow::Result<()> {
    let mut cfg = load_config(args.config.as_deref())?;
    resolve_paths(&mut cfg, args.config.as_deref());

    if let Some(seed_path) = args.seed {
        cfg.dev.seed_file = seed_path.to_string_lossy().into_owned();
    }
    if args.skip_migrate {
        cfg.storage.stateful.migrate = "skip".into();
    }

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let db = zitadel_db::Db::open_with_config(&cfg.storage.stateful.url, &cfg.storage.stateful)
            .await?;
        let _observability =
            zitadel_observability::install(&cfg.observability, Some(db.clone())).await?;
        tracing::info!(
            port = cfg.server.port,
            db = %cfg.storage.stateful.url,
            "starting zitadel server"
        );
        zitadel_server::run_with_db(cfg, db).await
    })?;
    Ok(())
}

pub(crate) fn run_migrate(args: MigrateArgs) -> anyhow::Result<()> {
    let mut cfg = load_config(args.config.as_deref())?;
    resolve_paths(&mut cfg, args.config.as_deref());
    let backend = zitadel_db::BackendKind::parse(cfg.storage.stateful.resolve_backend())?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        if backend == zitadel_db::BackendKind::Spanner && !args.status {
            zitadel_db::migrate::prepare_spanner(&cfg.storage.stateful).await?;
        }
        let db = zitadel_db::Db::open_with_config(&cfg.storage.stateful.url, &cfg.storage.stateful)
            .await?;
        let _observability =
            zitadel_observability::install(&cfg.observability, Some(db.clone())).await?;

        if args.status {
            zitadel_db::migrate::check_version(&db).await?;
            tracing::info!("schema is up to date");
        } else {
            if backend != zitadel_db::BackendKind::Spanner {
                zitadel_db::migrate::migrate(&db).await?;
            }
            let seeded_roles = zitadel_db::seed_builtin_role_definitions(&db).await?;
            zitadel_storage::prepare_postgres_role_databases(&cfg.storage, &db).await?;
            if args.bootstrap {
                let ext_domain =
                    Some(cfg.server.external_domain.as_str()).filter(|d| !d.is_empty());
                let result = zitadel_db::bootstrap::bootstrap(&db, ext_domain).await?;
                tracing::info!(
                    bootstrapped = result.changed,
                    seeded_roles,
                    "migration command completed"
                );
            } else {
                tracing::info!(seeded_roles, "migration command completed");
            }
        }
        db.close().await;
        anyhow::Ok(())
    })?;
    Ok(())
}

pub(crate) fn run_db_status(args: ConfigArg) -> anyhow::Result<()> {
    run_migrate(MigrateArgs {
        config: args.config,
        status: true,
        bootstrap: false,
    })
}

pub(crate) fn run_seed_apply(config: Option<PathBuf>, file: PathBuf) -> anyhow::Result<()> {
    let mut cfg = load_config(config.as_deref())?;
    resolve_paths(&mut cfg, config.as_deref());
    let rt = tokio::runtime::Runtime::new()?;
    let file = if file.is_absolute() {
        file
    } else {
        std::env::current_dir()?.join(file)
    };
    rt.block_on(async move {
        let db = zitadel_db::Db::open_with_config(&cfg.storage.stateful.url, &cfg.storage.stateful)
            .await?;
        let _observability =
            zitadel_observability::install(&cfg.observability, Some(db.clone())).await?;
        zitadel_db::seed::apply(&db, &file).await?;
        db.close().await;
        tracing::info!(file = %file.display(), "seed applied");
        anyhow::Ok(())
    })?;
    Ok(())
}

pub(crate) fn run_seed_validate(file: PathBuf) -> anyhow::Result<()> {
    let file = if file.is_absolute() {
        file
    } else {
        std::env::current_dir()?.join(file)
    };
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let cfg = zitadel_config::Config::default();
        let _observability = zitadel_observability::install(&cfg.observability, None).await?;
        let seed = zitadel_db::seed::validate(&file)?;
        tracing::info!(file = %file.display(), users = seed.users.len(), "seed valid");
        anyhow::Ok(())
    })?;
    Ok(())
}

pub(crate) fn print_reference() {
    print!("{}", zitadel_config::reference_toml());
}

pub(crate) fn run_openapi_export(args: OpenapiExportArgs) -> anyhow::Result<()> {
    let mut cfg = load_config(args.config.as_deref())?;
    resolve_paths(&mut cfg, args.config.as_deref());
    let rt = tokio::runtime::Runtime::new()?;
    let document = rt.block_on(async move {
        let db = zitadel_db::Db::open_with_config(&cfg.storage.stateful.url, &cfg.storage.stateful)
            .await?;
        let schema_registry = zitadel_server::repo_bridge::schema_registry_repo(db.clone());
        let document =
            zitadel_api::openapi::document(schema_registry.as_ref(), &public_origin(&cfg)).await?;
        db.close().await;
        anyhow::Ok(document)
    })?;
    println!("{}", serde_json::to_string_pretty(&document)?);
    Ok(())
}

pub(crate) fn run_perf_db_run(args: PerfDbRunArgs) -> anyhow::Result<()> {
    let backend = match args.backend.as_str() {
        "sqlite" => zitadel_perf::PerfBackend::Sqlite,
        "postgres" => zitadel_perf::PerfBackend::Postgres,
        other => return Err(anyhow::anyhow!("unsupported perf backend {other}")),
    };
    let profile = match args.profile.as_str() {
        "ci" => zitadel_perf::BenchmarkProfile::Ci,
        other => return Err(anyhow::anyhow!("unsupported perf profile {other}")),
    };
    if args.format != "json" {
        return Err(anyhow::anyhow!(
            "unsupported perf output format {}; expected json",
            args.format
        ));
    }

    let rt = tokio::runtime::Runtime::new()?;
    let report = rt.block_on(zitadel_perf::run_db_benchmark(zitadel_perf::RunOptions {
        backend,
        profile,
        database_url: args.database_url,
    }))?;

    if let Some(output) = args.output {
        zitadel_perf::write_report(&output, &report)?;
    } else {
        println!("{}", serde_json::to_string_pretty(&report)?);
    }
    Ok(())
}

pub(crate) fn run_perf_db_summarize(args: PerfDbSummaryArgs) -> anyhow::Result<()> {
    let markdown = zitadel_perf::summarize_report_files(&args.reports, &args.previous_reports)?;
    if let Some(output) = args.output {
        if let Some(parent) = output.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(output, markdown)?;
    } else {
        println!("{markdown}");
    }
    Ok(())
}

// ─── Config utilities ──────────────────────────────────────

pub(crate) fn load_config(path: Option<&Path>) -> anyhow::Result<zitadel_config::Config> {
    let cfg = zitadel_config::Config::load(path)?;
    Ok(cfg)
}

pub(crate) fn public_origin(cfg: &zitadel_config::Config) -> String {
    if !cfg.server.public_origin.is_empty() {
        return cfg.server.public_origin.trim_end_matches('/').to_string();
    }
    format!("http://{}:{}", cfg.server.external_domain, cfg.server.port)
}

/// Resolve relative paths (storage URLs, seed file, cache path) relative to the config file directory.
pub(crate) fn resolve_paths(cfg: &mut zitadel_config::Config, config_path: Option<&Path>) {
    let base_dir = config_path
        .and_then(|p| std::fs::canonicalize(p).ok())
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    resolve_sqlite_url(&mut cfg.storage.stateful.url, &base_dir);
    resolve_sqlite_url(&mut cfg.storage.read.url, &base_dir);
    resolve_sqlite_url(&mut cfg.storage.kv.url, &base_dir);
    resolve_sqlite_url(&mut cfg.storage.sink.url, &base_dir);
    resolve_sqlite_url(&mut cfg.storage.analytics.url, &base_dir);

    if !cfg.dev.seed_file.is_empty() && !Path::new(&cfg.dev.seed_file).is_absolute() {
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

    if !cfg.observability.cache_path.is_empty()
        && !Path::new(&cfg.observability.cache_path).is_absolute()
    {
        let resolved = base_dir.join(&cfg.observability.cache_path);
        cfg.observability.cache_path = resolved.to_string_lossy().into_owned();
    }
}

fn resolve_sqlite_url(url: &mut String, base_dir: &Path) {
    if let Some(path) = url.strip_prefix("sqlite://")
        && !path.is_empty()
        && path != ":memory:"
    {
        let p = Path::new(path);
        let joined = if p.is_absolute() {
            p.to_path_buf()
        } else {
            base_dir.join(path)
        };
        let resolved = normalize_path(&joined);
        if let Some(parent) = resolved.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        *url = format!("sqlite://{}", resolved.display());
    }
}

/// Normalize a path by resolving `.` and `..` components without requiring the path to exist.
pub(crate) fn normalize_path(path: &Path) -> PathBuf {
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

    let mut normalized = PathBuf::new();
    for component in components {
        normalized.push(component.as_os_str());
    }
    normalized
}
