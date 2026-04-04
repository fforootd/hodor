mod client;

use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};
use reqwest::Method;
use serde_json::{Value, json};

use crate::client::{
    CommandOutput, RemoteOverrides, parse_json_input, parse_key_value_pairs, parse_params_input,
};

#[derive(Parser)]
#[command(name = "zitadel", about = "Zitadel identity platform", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Server runtime commands.
    Server {
        #[command(subcommand)]
        action: ServerAction,
    },

    /// Database and migration commands.
    Db {
        #[command(subcommand)]
        action: DbAction,
    },

    /// Manage declarative seed files.
    Seed {
        #[command(subcommand)]
        action: SeedAction,
    },

    /// Config and reference helpers.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },

    /// Authenticate a remote CLI profile.
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Manage users through the remote API.
    #[command(visible_alias = "user")]
    Users {
        #[command(subcommand)]
        action: UsersAction,
    },

    /// Inspect schemas through the remote API.
    #[command(visible_alias = "schema")]
    Schemas {
        #[command(subcommand)]
        action: SchemasAction,
    },

    /// Call the remote API directly.
    Api {
        #[command(subcommand)]
        action: ApiAction,
    },

    /// Export OpenAPI 3.1 spec to stdout.
    Openapi {
        #[command(subcommand)]
        action: OpenapiAction,
    },

    /// Run performance harnesses and summaries.
    Perf {
        #[command(subcommand)]
        action: PerfAction,
    },

    /// Compatibility alias for `zitadel server start`.
    Start(StartArgs),

    /// Compatibility alias for `zitadel db migrate`.
    Migrate(MigrateArgs),

    /// Compatibility alias for `zitadel openapi export`.
    OpenapiExport(OpenapiExportArgs),
}

#[derive(Subcommand)]
enum ServerAction {
    /// Run the HTTP server.
    Start(StartArgs),
}

#[derive(Subcommand)]
enum DbAction {
    /// Run pending migrations and optionally bootstrap.
    Migrate(MigrateArgs),
    /// Print migration status and exit.
    Status(ConfigArg),
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Print the reference server configuration.
    PrintReference,
}

#[derive(Subcommand)]
enum OpenapiAction {
    /// Export OpenAPI 3.1 spec to stdout.
    Export(OpenapiExportArgs),
}

#[derive(Subcommand)]
enum PerfAction {
    /// Database performance scenarios.
    Db {
        #[command(subcommand)]
        action: PerfDbAction,
    },
}

#[derive(Subcommand)]
enum PerfDbAction {
    /// Run the database perf harness.
    Run(PerfDbRunArgs),
    /// Render a markdown summary from JSON reports.
    Summarize(PerfDbSummaryArgs),
}

#[derive(Subcommand)]
enum AuthAction {
    /// Run OIDC browser login for the selected profile.
    Login(LoginArgs),
    /// Bearer token and PAT helpers.
    Token {
        #[command(subcommand)]
        action: AuthTokenAction,
    },
    /// Compatibility alias for `zitadel auth token set`.
    #[command(hide = true)]
    TokenSet(TokenSetArgs),
    /// Clear the stored session for the selected profile.
    Logout(RemoteArgs),
    /// Show local profile and auth status.
    Status(RemoteArgs),
    /// Return the current authenticated identity.
    Whoami(RemoteArgs),
}

#[derive(Subcommand)]
enum AuthTokenAction {
    /// Store a bearer token or PAT for the selected profile.
    Set(TokenSetArgs),
}

#[derive(Subcommand)]
enum UsersAction {
    Create(UserCreateArgs),
    Get(UserGetArgs),
    List(UserListArgs),
    Update(UserUpdateArgs),
    Delete(UserDeleteArgs),
}

#[derive(Subcommand)]
enum SchemasAction {
    Inspect(SchemaInspectArgs),
}

#[derive(Subcommand)]
enum ApiAction {
    Call(ApiCallArgs),
}

#[derive(Args, Clone)]
struct ConfigArg {
    /// Path to TOML config file.
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(Args, Clone)]
struct StartArgs {
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
}

#[derive(Args, Clone)]
struct MigrateArgs {
    /// Path to TOML config file.
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Print current schema version and exit.
    #[arg(long)]
    status: bool,

    /// Bootstrap the default org/admin after migrations.
    #[arg(long)]
    bootstrap: bool,
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

#[derive(Args, Clone, Default)]
struct RemoteArgs {
    /// Named client profile to use.
    #[arg(long)]
    profile: Option<String>,

    /// Path to the remote client profile TOML file.
    #[arg(long)]
    profile_path: Option<PathBuf>,

    /// OIDC issuer URL for remote login.
    #[arg(long)]
    issuer: Option<String>,

    /// API base URL for remote calls.
    #[arg(long)]
    api_url: Option<String>,

    /// OIDC client id for browser login.
    #[arg(long)]
    client_id: Option<String>,

    /// Loopback redirect URI for browser login.
    #[arg(long)]
    redirect_uri: Option<String>,

    /// Override the stored bearer token for this invocation.
    #[arg(long)]
    token: Option<String>,
}

#[derive(Args, Clone)]
struct LoginArgs {
    #[command(flatten)]
    remote: RemoteArgs,

    /// Do not try to open a browser automatically.
    #[arg(long)]
    no_browser: bool,
}

#[derive(Args, Clone)]
struct TokenSetArgs {
    #[command(flatten)]
    remote: RemoteArgs,

    /// Bearer token or PAT to store for the selected profile.
    #[arg(long)]
    token_value: String,
}

#[derive(Args, Clone)]
struct UserCreateArgs {
    #[command(flatten)]
    remote: RemoteArgs,

    /// Full JSON payload or @path to a JSON file.
    #[arg(long)]
    json: Option<String>,

    /// Set request fields as key=value pairs.
    #[arg(long = "set")]
    set: Vec<String>,

    /// Convenience field for the user identifier.
    #[arg(long)]
    identifier: Option<String>,

    /// Convenience field for the display name.
    #[arg(long)]
    display_name: Option<String>,

    /// Convenience field for the schema id.
    #[arg(long)]
    schema_id: Option<String>,

    /// Validate and print the request instead of sending it.
    #[arg(long)]
    dry_run: bool,
}

#[derive(Args, Clone)]
struct UserGetArgs {
    #[command(flatten)]
    remote: RemoteArgs,
    id: String,
}

#[derive(Args, Clone)]
struct UserListArgs {
    #[command(flatten)]
    remote: RemoteArgs,

    #[arg(long, default_value_t = 50)]
    limit: i64,

    #[arg(long)]
    cursor: Option<String>,

    #[arg(long)]
    page_all: bool,

    #[arg(long)]
    stream_ndjson: bool,
}

#[derive(Args, Clone)]
struct UserUpdateArgs {
    #[command(flatten)]
    remote: RemoteArgs,
    id: String,

    /// Full JSON payload or @path to a JSON file.
    #[arg(long)]
    json: Option<String>,

    /// Set request fields as key=value pairs.
    #[arg(long = "set")]
    set: Vec<String>,

    #[arg(long)]
    dry_run: bool,
}

#[derive(Args, Clone)]
struct UserDeleteArgs {
    #[command(flatten)]
    remote: RemoteArgs,
    id: String,

    #[arg(long)]
    dry_run: bool,
}

#[derive(Args, Clone)]
struct SchemaInspectArgs {
    #[command(flatten)]
    remote: RemoteArgs,

    /// Return the embedded schema meta-catalog.
    #[arg(long)]
    meta: bool,

    /// Optional schema id to fetch.
    id: Option<String>,
}

#[derive(Args, Clone)]
struct OpenapiExportArgs {
    /// Path to TOML config file.
    #[arg(short, long)]
    config: Option<PathBuf>,
}

#[derive(Args, Clone)]
struct ApiCallArgs {
    #[command(flatten)]
    remote: RemoteArgs,

    method: String,
    path: String,

    /// Add query parameters as key=value pairs.
    #[arg(long = "param")]
    param: Vec<String>,

    /// Full JSON params object or @path to a JSON file.
    #[arg(long)]
    params: Option<String>,

    /// Full JSON payload or @path to a JSON file.
    #[arg(long)]
    json: Option<String>,

    /// Send the request without an Authorization header.
    #[arg(long)]
    no_auth: bool,

    #[arg(long)]
    dry_run: bool,
}

#[derive(Args, Clone)]
struct PerfDbRunArgs {
    /// Backend to benchmark.
    #[arg(long, value_parser = ["sqlite", "postgres"])]
    backend: String,

    /// Benchmark profile to run.
    #[arg(long, default_value = "ci", value_parser = ["ci"])]
    profile: String,

    /// Optional explicit database URL. Defaults to a temp SQLite file or a local Postgres DSN.
    #[arg(long)]
    database_url: Option<String>,

    /// Output format.
    #[arg(long, default_value = "json", value_parser = ["json"])]
    format: String,

    /// Write the report to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

#[derive(Args, Clone)]
struct PerfDbSummaryArgs {
    /// Current run JSON report(s).
    #[arg(long = "report", required = true)]
    reports: Vec<PathBuf>,

    /// Previous run JSON report(s) for comparison.
    #[arg(long = "previous-report")]
    previous_reports: Vec<PathBuf>,

    /// Write the markdown summary to a file instead of stdout.
    #[arg(long)]
    output: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Server { action } => match action {
            ServerAction::Start(args) => run_start(args)?,
        },
        Commands::Db { action } => match action {
            DbAction::Migrate(args) => run_migrate(args)?,
            DbAction::Status(args) => run_db_status(args)?,
        },
        Commands::Seed { action } => match action {
            SeedAction::Apply { config, file } => run_seed_apply(config, file)?,
            SeedAction::Validate { file } => run_seed_validate(file)?,
        },
        Commands::Config { action } => match action {
            ConfigAction::PrintReference => print_reference(),
        },
        Commands::Auth { action } => match action {
            AuthAction::Login(args) => {
                let rt = tokio::runtime::Runtime::new()?;
                print_output(rt.block_on(client::auth_login(
                    &remote_overrides(&args.remote),
                    args.no_browser,
                ))?)?;
            }
            AuthAction::Token { action } => match action {
                AuthTokenAction::Set(args) => {
                    print_output(client::auth_token_set(
                        &remote_overrides(&args.remote),
                        args.token_value,
                    )?)?;
                }
            },
            AuthAction::TokenSet(args) => {
                print_output(client::auth_token_set(
                    &remote_overrides(&args.remote),
                    args.token_value,
                )?)?;
            }
            AuthAction::Logout(args) => {
                print_output(client::auth_logout(&remote_overrides(&args))?)?;
            }
            AuthAction::Status(args) => {
                print_output(client::auth_status(&remote_overrides(&args))?)?;
            }
            AuthAction::Whoami(args) => {
                let rt = tokio::runtime::Runtime::new()?;
                print_output(rt.block_on(client::auth_whoami(&remote_overrides(&args)))?)?;
            }
        },
        Commands::Users { action } => match action {
            UsersAction::Create(args) => {
                let body = build_user_body(
                    &args.json,
                    &args.set,
                    &args.identifier,
                    &args.display_name,
                    &args.schema_id,
                )?;
                let rt = tokio::runtime::Runtime::new()?;
                print_output(rt.block_on(client::api_call(
                    &remote_overrides(&args.remote),
                    Method::POST,
                    "/v1/users",
                    &[],
                    Some(body),
                    args.dry_run,
                    true,
                ))?)?;
            }
            UsersAction::Get(args) => {
                client::validate_identifier(&args.id)?;
                let rt = tokio::runtime::Runtime::new()?;
                let path = format!("/v1/users/{}", args.id);
                print_output(rt.block_on(client::api_call(
                    &remote_overrides(&args.remote),
                    Method::GET,
                    &path,
                    &[],
                    None,
                    false,
                    true,
                ))?)?;
            }
            UsersAction::List(args) => {
                let rt = tokio::runtime::Runtime::new()?;
                let overrides = remote_overrides(&args.remote);
                let output = rt.block_on(fetch_all_list(
                    &overrides,
                    "/v1/users",
                    args.limit,
                    args.cursor,
                    args.page_all,
                ))?;
                if args.stream_ndjson {
                    print_output(list_to_ndjson(output)?)?;
                } else {
                    print_output(output)?;
                }
            }
            UsersAction::Update(args) => {
                client::validate_identifier(&args.id)?;
                let body = build_update_body(&args.json, &args.set)?;
                let rt = tokio::runtime::Runtime::new()?;
                let path = format!("/v1/users/{}", args.id);
                print_output(rt.block_on(client::api_call(
                    &remote_overrides(&args.remote),
                    Method::PATCH,
                    &path,
                    &[],
                    Some(body),
                    args.dry_run,
                    true,
                ))?)?;
            }
            UsersAction::Delete(args) => {
                client::validate_identifier(&args.id)?;
                let rt = tokio::runtime::Runtime::new()?;
                let path = format!("/v1/users/{}", args.id);
                print_output(rt.block_on(client::api_call(
                    &remote_overrides(&args.remote),
                    Method::DELETE,
                    &path,
                    &[],
                    None,
                    args.dry_run,
                    true,
                ))?)?;
            }
        },
        Commands::Schemas { action } => match action {
            SchemasAction::Inspect(args) => {
                let rt = tokio::runtime::Runtime::new()?;
                print_output(rt.block_on(client::schema_inspect(
                    &remote_overrides(&args.remote),
                    args.id,
                    args.meta,
                ))?)?;
            }
        },
        Commands::Api { action } => match action {
            ApiAction::Call(args) => {
                let method = Method::from_bytes(args.method.as_bytes())
                    .map_err(|_| anyhow::anyhow!("invalid HTTP method {}", args.method))?;
                let params = parse_api_params(args.params.as_deref(), &args.param)?;
                let body = parse_json_input(args.json.as_deref())?;
                let rt = tokio::runtime::Runtime::new()?;
                print_output(rt.block_on(client::api_call(
                    &remote_overrides(&args.remote),
                    method,
                    &args.path,
                    &params,
                    body,
                    args.dry_run,
                    !args.no_auth,
                ))?)?;
            }
        },
        Commands::Openapi { action } => match action {
            OpenapiAction::Export(args) => run_openapi_export(args)?,
        },
        Commands::Perf { action } => match action {
            PerfAction::Db { action } => match action {
                PerfDbAction::Run(args) => run_perf_db_run(args)?,
                PerfDbAction::Summarize(args) => run_perf_db_summarize(args)?,
            },
        },
        Commands::Start(args) => run_start(args)?,
        Commands::Migrate(args) => run_migrate(args)?,
        Commands::OpenapiExport(args) => run_openapi_export(args)?,
    }

    Ok(())
}

fn run_start(args: StartArgs) -> anyhow::Result<()> {
    let mut cfg = load_config(args.config.as_deref())?;
    resolve_paths(&mut cfg, args.config.as_deref());

    if args.mock_oidc {
        cfg.dev.mock_oidc = true;
    }
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

fn run_migrate(args: MigrateArgs) -> anyhow::Result<()> {
    let mut cfg = load_config(args.config.as_deref())?;
    resolve_paths(&mut cfg, args.config.as_deref());
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let db = zitadel_db::Db::open_with_config(&cfg.storage.stateful.url, &cfg.storage.stateful)
            .await?;
        let _observability =
            zitadel_observability::install(&cfg.observability, Some(db.clone())).await?;

        if args.status {
            zitadel_db::migrate::check_version(&db).await?;
            tracing::info!("schema is up to date");
        } else {
            zitadel_db::migrate::migrate(&db).await?;
            zitadel_storage::prepare_postgres_role_databases(&cfg.storage, &db).await?;
            if args.bootstrap {
                let changed = zitadel_db::bootstrap::bootstrap(&db).await?;
                tracing::info!(bootstrapped = changed, "migration command completed");
            } else {
                tracing::info!("migration command completed");
            }
        }
        db.close().await;
        anyhow::Ok(())
    })?;
    Ok(())
}

fn run_db_status(args: ConfigArg) -> anyhow::Result<()> {
    run_migrate(MigrateArgs {
        config: args.config,
        status: true,
        bootstrap: false,
    })
}

fn run_seed_apply(config: Option<PathBuf>, file: PathBuf) -> anyhow::Result<()> {
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

fn run_seed_validate(file: PathBuf) -> anyhow::Result<()> {
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

fn print_reference() {
    print!("{}", zitadel_config::reference_toml());
}

fn run_openapi_export(args: OpenapiExportArgs) -> anyhow::Result<()> {
    let mut cfg = load_config(args.config.as_deref())?;
    resolve_paths(&mut cfg, args.config.as_deref());
    let rt = tokio::runtime::Runtime::new()?;
    let document = rt.block_on(async move {
        let db = zitadel_db::Db::open_with_config(&cfg.storage.stateful.url, &cfg.storage.stateful)
            .await?;
        let document = zitadel_api::openapi::document(&db, &public_origin(&cfg)).await?;
        db.close().await;
        anyhow::Ok(document)
    })?;
    println!("{}", serde_json::to_string_pretty(&document)?);
    Ok(())
}

fn run_perf_db_run(args: PerfDbRunArgs) -> anyhow::Result<()> {
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

fn run_perf_db_summarize(args: PerfDbSummaryArgs) -> anyhow::Result<()> {
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

fn remote_overrides(args: &RemoteArgs) -> RemoteOverrides {
    RemoteOverrides {
        profile: args.profile.clone(),
        profile_path: args.profile_path.clone(),
        issuer: args.issuer.clone(),
        api_url: args.api_url.clone(),
        client_id: args.client_id.clone(),
        redirect_uri: args.redirect_uri.clone(),
        access_token: args.token.clone(),
    }
}

fn parse_api_params(
    params_json: Option<&str>,
    items: &[String],
) -> anyhow::Result<Vec<(String, String)>> {
    let mut out = parse_params_input(params_json)?;
    for item in items {
        let (key, value) = item
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("expected key=value, got {item}"))?;
        client::validate_identifier(key)?;
        client::reject_control_chars(value)?;
        out.push((key.to_string(), value.to_string()));
    }
    Ok(out)
}

fn build_user_body(
    json_arg: &Option<String>,
    set: &[String],
    identifier: &Option<String>,
    display_name: &Option<String>,
    schema_id: &Option<String>,
) -> anyhow::Result<Value> {
    if let Some(value) = parse_json_input(json_arg.as_deref())? {
        return Ok(value);
    }

    let mut body = parse_key_value_pairs(set)?;
    if let Some(identifier) = identifier {
        body.insert("identifier".into(), Value::String(identifier.clone()));
    }
    if let Some(display_name) = display_name {
        body.insert("display_name".into(), Value::String(display_name.clone()));
    }
    if let Some(schema_id) = schema_id {
        body.insert("schema_id".into(), Value::String(schema_id.clone()));
    }
    if !body.contains_key("identifier") {
        return Err(anyhow::anyhow!(
            "identifier is required unless you provide --json"
        ));
    }
    Ok(Value::Object(body))
}

fn build_update_body(json_arg: &Option<String>, set: &[String]) -> anyhow::Result<Value> {
    if let Some(value) = parse_json_input(json_arg.as_deref())? {
        return Ok(value);
    }
    let body = parse_key_value_pairs(set)?;
    if body.is_empty() {
        return Err(anyhow::anyhow!(
            "no update fields provided; use --json or --set key=value"
        ));
    }
    Ok(Value::Object(body))
}

fn list_to_ndjson(output: CommandOutput) -> anyhow::Result<CommandOutput> {
    match output {
        CommandOutput::Json(Value::Object(map)) => {
            let items = map
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("response is not a list payload"))?;
            Ok(CommandOutput::Ndjson(items))
        }
        other => Ok(other),
    }
}

async fn fetch_all_list(
    overrides: &RemoteOverrides,
    path: &str,
    limit: i64,
    cursor: Option<String>,
    page_all: bool,
) -> anyhow::Result<CommandOutput> {
    if !page_all {
        let mut params = vec![("limit".to_string(), limit.to_string())];
        if let Some(cursor) = cursor {
            params.push(("cursor".into(), cursor));
        }
        return client::api_call(overrides, Method::GET, path, &params, None, false, true).await;
    }

    let mut next_cursor = cursor;
    let mut all_items = Vec::new();

    loop {
        let mut params = vec![("limit".to_string(), limit.to_string())];
        if let Some(cursor) = next_cursor.clone() {
            params.push(("cursor".into(), cursor));
        }
        let output =
            client::api_call(overrides, Method::GET, path, &params, None, false, true).await?;
        let (items, next) = unpack_list_payload(output)?;
        all_items.extend(items);
        if next.is_none() {
            return Ok(CommandOutput::Json(json!({
                "items": all_items,
                "next_cursor": Value::Null,
                "total": Value::Null,
            })));
        }
        next_cursor = next;
    }
}

fn unpack_list_payload(output: CommandOutput) -> anyhow::Result<(Vec<Value>, Option<String>)> {
    match output {
        CommandOutput::Json(Value::Object(map)) => {
            let items = map
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("response is not a list payload"))?;
            let next_cursor = map
                .get("next_cursor")
                .and_then(Value::as_str)
                .map(ToString::to_string);
            Ok((items, next_cursor))
        }
        _ => Err(anyhow::anyhow!("response is not a list payload")),
    }
}

fn print_output(output: CommandOutput) -> anyhow::Result<()> {
    match output {
        CommandOutput::Json(value) => {
            println!("{}", serde_json::to_string_pretty(&value)?);
        }
        CommandOutput::Ndjson(values) => {
            for value in values {
                println!("{}", serde_json::to_string(&value)?);
            }
        }
        CommandOutput::Text(text) => {
            println!("{text}");
        }
    }
    Ok(())
}

fn load_config(path: Option<&Path>) -> anyhow::Result<zitadel_config::Config> {
    let cfg = zitadel_config::Config::load(path)?;
    Ok(cfg)
}

fn public_origin(cfg: &zitadel_config::Config) -> String {
    if !cfg.server.public_origin.is_empty() {
        return cfg.server.public_origin.trim_end_matches('/').to_string();
    }
    format!("http://{}:{}", cfg.server.external_domain, cfg.server.port)
}

/// Resolve relative paths (storage URLs, seed file, cache path) relative to the config file directory.
fn resolve_paths(cfg: &mut zitadel_config::Config, config_path: Option<&Path>) {
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
fn normalize_path(path: &Path) -> PathBuf {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_namespaced_start_command() {
        let cli = Cli::try_parse_from(["zitadel", "server", "start"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Server {
                action: ServerAction::Start(_)
            }
        ));
    }

    #[test]
    fn parses_singular_user_alias() {
        let cli = Cli::try_parse_from([
            "zitadel",
            "user",
            "get",
            "abc123",
            "--api-url",
            "https://example.com",
            "--token",
            "tok",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Users {
                action: UsersAction::Get(_)
            }
        ));
    }

    #[test]
    fn parses_nested_auth_token_set() {
        let cli = Cli::try_parse_from([
            "zitadel",
            "auth",
            "token",
            "set",
            "--api-url",
            "https://example.com",
            "--token-value",
            "tok",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Auth {
                action: AuthAction::Token { .. }
            }
        ));
    }

    #[test]
    fn parses_perf_db_run_command() {
        let cli = Cli::try_parse_from([
            "zitadel",
            "perf",
            "db",
            "run",
            "--backend",
            "sqlite",
            "--profile",
            "ci",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Perf {
                action: PerfAction::Db {
                    action: PerfDbAction::Run(_)
                }
            }
        ));
    }
}
