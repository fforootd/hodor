mod cli;
mod client;
mod commands;

use clap::Parser;
use reqwest::Method;
use serde_json::{Value, json};

use crate::cli::*;
use crate::client::{
    CommandOutput, RemoteOverrides, parse_json_input, parse_key_value_pairs, parse_params_input,
};
use crate::commands::*;

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

// ─── Dispatcher utilities ──────────────────────────────────

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
