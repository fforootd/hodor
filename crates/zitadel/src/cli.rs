use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "zitadel", about = "Zitadel identity platform", version)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
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
pub(crate) enum ServerAction {
    /// Run the HTTP server.
    Start(StartArgs),
}

#[derive(Subcommand)]
pub(crate) enum DbAction {
    /// Run pending migrations and optionally bootstrap.
    Migrate(MigrateArgs),
    /// Print migration status and exit.
    Status(ConfigArg),
}

#[derive(Subcommand)]
pub(crate) enum ConfigAction {
    /// Print the reference server configuration.
    PrintReference,
}

#[derive(Subcommand)]
pub(crate) enum OpenapiAction {
    /// Export OpenAPI 3.1 spec to stdout.
    Export(OpenapiExportArgs),
}

#[derive(Subcommand)]
pub(crate) enum PerfAction {
    /// Database performance scenarios.
    Db {
        #[command(subcommand)]
        action: PerfDbAction,
    },
}

#[derive(Subcommand)]
pub(crate) enum PerfDbAction {
    /// Run the database perf harness.
    Run(PerfDbRunArgs),
    /// Render a markdown summary from JSON reports.
    Summarize(PerfDbSummaryArgs),
}

#[derive(Subcommand)]
pub(crate) enum AuthAction {
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
pub(crate) enum AuthTokenAction {
    /// Store a bearer token or PAT for the selected profile.
    Set(TokenSetArgs),
}

#[derive(Subcommand)]
pub(crate) enum UsersAction {
    Create(UserCreateArgs),
    Get(UserGetArgs),
    List(UserListArgs),
    Update(UserUpdateArgs),
    Delete(UserDeleteArgs),
}

#[derive(Subcommand)]
pub(crate) enum SchemasAction {
    Inspect(SchemaInspectArgs),
}

#[derive(Subcommand)]
pub(crate) enum ApiAction {
    Call(ApiCallArgs),
}

#[derive(Args, Clone)]
pub(crate) struct ConfigArg {
    /// Path to TOML config file.
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

#[derive(Args, Clone)]
pub(crate) struct StartArgs {
    /// Path to TOML config file.
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Path to YAML seed file loaded on startup.
    #[arg(long)]
    pub seed: Option<PathBuf>,

    /// Skip automatic database migration.
    #[arg(long)]
    pub skip_migrate: bool,
}

#[derive(Args, Clone)]
pub(crate) struct MigrateArgs {
    /// Path to TOML config file.
    #[arg(short, long)]
    pub config: Option<PathBuf>,

    /// Print current schema version and exit.
    #[arg(long)]
    pub status: bool,

    /// Bootstrap the default org/admin after migrations.
    #[arg(long)]
    pub bootstrap: bool,
}

#[derive(Subcommand)]
pub(crate) enum SeedAction {
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
pub(crate) struct RemoteArgs {
    /// Named client profile to use.
    #[arg(long)]
    pub profile: Option<String>,

    /// Path to the remote client profile TOML file.
    #[arg(long)]
    pub profile_path: Option<PathBuf>,

    /// OIDC issuer URL for remote login.
    #[arg(long)]
    pub issuer: Option<String>,

    /// API base URL for remote calls.
    #[arg(long)]
    pub api_url: Option<String>,

    /// OIDC client id for browser login.
    #[arg(long)]
    pub client_id: Option<String>,

    /// Loopback redirect URI for browser login.
    #[arg(long)]
    pub redirect_uri: Option<String>,

    /// Override the stored bearer token for this invocation.
    #[arg(long)]
    pub token: Option<String>,
}

#[derive(Args, Clone)]
pub(crate) struct LoginArgs {
    #[command(flatten)]
    pub remote: RemoteArgs,

    /// Do not try to open a browser automatically.
    #[arg(long)]
    pub no_browser: bool,
}

#[derive(Args, Clone)]
pub(crate) struct TokenSetArgs {
    #[command(flatten)]
    pub remote: RemoteArgs,

    /// Bearer token or PAT to store for the selected profile.
    #[arg(long)]
    pub token_value: String,
}

#[derive(Args, Clone)]
pub(crate) struct UserCreateArgs {
    #[command(flatten)]
    pub remote: RemoteArgs,

    /// Full JSON payload or @path to a JSON file.
    #[arg(long)]
    pub json: Option<String>,

    /// Set request fields as key=value pairs.
    #[arg(long = "set")]
    pub set: Vec<String>,

    /// Convenience field for the user identifier.
    #[arg(long)]
    pub identifier: Option<String>,

    /// Convenience field for the display name.
    #[arg(long)]
    pub display_name: Option<String>,

    /// Convenience field for the schema id.
    #[arg(long)]
    pub schema_id: Option<String>,

    /// Validate and print the request instead of sending it.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Clone)]
pub(crate) struct UserGetArgs {
    #[command(flatten)]
    pub remote: RemoteArgs,
    pub id: String,
}

#[derive(Args, Clone)]
pub(crate) struct UserListArgs {
    #[command(flatten)]
    pub remote: RemoteArgs,

    #[arg(long, default_value_t = 50)]
    pub limit: i64,

    #[arg(long)]
    pub cursor: Option<String>,

    #[arg(long)]
    pub page_all: bool,

    #[arg(long)]
    pub stream_ndjson: bool,
}

#[derive(Args, Clone)]
pub(crate) struct UserUpdateArgs {
    #[command(flatten)]
    pub remote: RemoteArgs,
    pub id: String,

    /// Full JSON payload or @path to a JSON file.
    #[arg(long)]
    pub json: Option<String>,

    /// Set request fields as key=value pairs.
    #[arg(long = "set")]
    pub set: Vec<String>,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Clone)]
pub(crate) struct UserDeleteArgs {
    #[command(flatten)]
    pub remote: RemoteArgs,
    pub id: String,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Clone)]
pub(crate) struct SchemaInspectArgs {
    #[command(flatten)]
    pub remote: RemoteArgs,

    /// Return the embedded schema meta-catalog.
    #[arg(long)]
    pub meta: bool,

    /// Optional schema id to fetch.
    pub id: Option<String>,
}

#[derive(Args, Clone)]
pub(crate) struct OpenapiExportArgs {
    /// Path to TOML config file.
    #[arg(short, long)]
    pub config: Option<PathBuf>,
}

#[derive(Args, Clone)]
pub(crate) struct ApiCallArgs {
    #[command(flatten)]
    pub remote: RemoteArgs,

    pub method: String,
    pub path: String,

    /// Add query parameters as key=value pairs.
    #[arg(long = "param")]
    pub param: Vec<String>,

    /// Full JSON params object or @path to a JSON file.
    #[arg(long)]
    pub params: Option<String>,

    /// Full JSON payload or @path to a JSON file.
    #[arg(long)]
    pub json: Option<String>,

    /// Send the request without an Authorization header.
    #[arg(long)]
    pub no_auth: bool,

    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args, Clone)]
pub(crate) struct PerfDbRunArgs {
    /// Backend to benchmark.
    #[arg(long, value_parser = ["sqlite", "postgres"])]
    pub backend: String,

    /// Benchmark profile to run.
    #[arg(long, default_value = "ci", value_parser = ["ci"])]
    pub profile: String,

    /// Optional explicit database URL. Defaults to a temp SQLite file or a local Postgres DSN.
    #[arg(long)]
    pub database_url: Option<String>,

    /// Output format.
    #[arg(long, default_value = "json", value_parser = ["json"])]
    pub format: String,

    /// Write the report to a file instead of stdout.
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args, Clone)]
pub(crate) struct PerfDbSummaryArgs {
    /// Current run JSON report(s).
    #[arg(long = "report", required = true)]
    pub reports: Vec<PathBuf>,

    /// Previous run JSON report(s) for comparison.
    #[arg(long = "previous-report")]
    pub previous_reports: Vec<PathBuf>,

    /// Write the markdown summary to a file instead of stdout.
    #[arg(long)]
    pub output: Option<PathBuf>,
}
