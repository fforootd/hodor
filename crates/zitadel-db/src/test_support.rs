use std::env;

use anyhow::Context;
use google_cloud_gax::{conn::Environment, grpc::Code};
use google_cloud_googleapis::spanner::admin::{
    database::v1::{CreateDatabaseRequest, DatabaseDialect, GetDatabaseRequest},
    instance::v1::{CreateInstanceRequest, GetInstanceRequest, Instance},
};
use google_cloud_spanner::admin::{AdminClientConfig, client::Client as AdminClient};
use uuid::Uuid;
use zitadel_config::StatefulStorageConfig;

use crate::{Db, ParsedDatabaseName};

pub const TEST_SPANNER_EMULATOR_HOST_ENV: &str = "ZITADEL_TEST_SPANNER_EMULATOR_HOST";
pub const TEST_SPANNER_PROJECT_ENV: &str = "ZITADEL_TEST_SPANNER_PROJECT";
pub const TEST_SPANNER_INSTANCE_ENV: &str = "ZITADEL_TEST_SPANNER_INSTANCE";
pub const TEST_SPANNER_DATABASE_PREFIX_ENV: &str = "ZITADEL_TEST_SPANNER_DATABASE_PREFIX";
pub const TEST_SPANNER_DATABASE_ENV: &str = "ZITADEL_TEST_SPANNER_DATABASE";

enum SpannerTestTarget {
    Managed {
        emulator_host: String,
        project: String,
        instance: String,
        database_prefix: String,
    },
    Legacy {
        emulator_host: String,
        database: String,
    },
}

impl SpannerTestTarget {
    fn from_env() -> anyhow::Result<Option<Self>> {
        let emulator_host = read_env(TEST_SPANNER_EMULATOR_HOST_ENV);
        let database = read_env(TEST_SPANNER_DATABASE_ENV);
        let project = read_env(TEST_SPANNER_PROJECT_ENV);
        let instance = read_env(TEST_SPANNER_INSTANCE_ENV);
        let database_prefix = read_env(TEST_SPANNER_DATABASE_PREFIX_ENV);

        let any_vars_present = emulator_host.is_some()
            || database.is_some()
            || project.is_some()
            || instance.is_some()
            || database_prefix.is_some();

        if let Some(database) = database {
            let emulator_host = emulator_host.ok_or_else(|| {
                anyhow::anyhow!(
                    "{TEST_SPANNER_EMULATOR_HOST_ENV} is required when {TEST_SPANNER_DATABASE_ENV} is set"
                )
            })?;
            return Ok(Some(Self::Legacy {
                emulator_host,
                database,
            }));
        }

        match (emulator_host, project, instance, database_prefix) {
            (Some(emulator_host), Some(project), Some(instance), Some(database_prefix)) => {
                Ok(Some(Self::Managed {
                    emulator_host,
                    project,
                    instance,
                    database_prefix,
                }))
            }
            (None, None, None, None) if !any_vars_present => {
                if env::var_os("CI").is_some() {
                    anyhow::bail!(
                        "Spanner emulator tests are enabled in CI but the emulator env is missing; set {TEST_SPANNER_EMULATOR_HOST_ENV}, {TEST_SPANNER_PROJECT_ENV}, {TEST_SPANNER_INSTANCE_ENV}, and {TEST_SPANNER_DATABASE_PREFIX_ENV}"
                    );
                }
                Ok(None)
            }
            _ => anyhow::bail!(
                "incomplete Spanner emulator test configuration; set either {TEST_SPANNER_DATABASE_ENV} plus {TEST_SPANNER_EMULATOR_HOST_ENV}, or the full managed env set: {TEST_SPANNER_EMULATOR_HOST_ENV}, {TEST_SPANNER_PROJECT_ENV}, {TEST_SPANNER_INSTANCE_ENV}, and {TEST_SPANNER_DATABASE_PREFIX_ENV}"
            ),
        }
    }

    async fn provision_stateful_config(
        &self,
        suite: &str,
    ) -> anyhow::Result<StatefulStorageConfig> {
        let (emulator_host, database) = match self {
            Self::Managed {
                emulator_host,
                project,
                instance,
                database_prefix,
            } => {
                let database_id = unique_database_id(database_prefix, suite);
                (
                    emulator_host.clone(),
                    format!("projects/{project}/instances/{instance}/databases/{database_id}"),
                )
            }
            Self::Legacy {
                emulator_host,
                database,
            } => (emulator_host.clone(), database.clone()),
        };

        ensure_emulator_database(&emulator_host, &database).await?;

        Ok(StatefulStorageConfig {
            backend: "spanner".into(),
            database,
            emulator_host,
            ..Default::default()
        })
    }
}

pub async fn spanner_stateful_config_from_env(
    suite: &str,
) -> anyhow::Result<Option<StatefulStorageConfig>> {
    let Some(target) = SpannerTestTarget::from_env()? else {
        eprintln!(
            "skipping Spanner suite {suite}: no {} or managed Spanner emulator env configured",
            TEST_SPANNER_DATABASE_ENV
        );
        return Ok(None);
    };

    target.provision_stateful_config(suite).await.map(Some)
}

pub async fn spanner_db_from_env(suite: &str) -> anyhow::Result<Option<Db>> {
    let Some(config) = spanner_stateful_config_from_env(suite).await? else {
        return Ok(None);
    };

    Db::open_with_config("", &config)
        .await
        .with_context(|| format!("open Spanner test database {}", config.database))
        .map(Some)
}

async fn ensure_emulator_database(emulator_host: &str, database: &str) -> anyhow::Result<()> {
    let parsed = ParsedDatabaseName::parse(database)?;
    let (project_parent, instance_id) = parsed
        .parent
        .rsplit_once("/instances/")
        .ok_or_else(|| anyhow::anyhow!("invalid Spanner instance parent {}", parsed.parent))?;
    let admin = AdminClient::new(AdminClientConfig {
        environment: Environment::Emulator(emulator_host.to_string()),
    })
    .await
    .with_context(|| format!("open Spanner admin client against {emulator_host}"))?;

    ensure_emulator_instance(&admin, project_parent, instance_id).await?;
    ensure_database_exists(&admin, &parsed).await?;
    Ok(())
}

async fn ensure_emulator_instance(
    admin: &AdminClient,
    project_parent: &str,
    instance_id: &str,
) -> anyhow::Result<()> {
    let instance_name = format!("{project_parent}/instances/{instance_id}");
    match admin
        .instance()
        .get_instance(
            GetInstanceRequest {
                name: instance_name.clone(),
                field_mask: None,
            },
            None,
        )
        .await
    {
        Ok(_) => return Ok(()),
        Err(status) if status.code() == Code::NotFound => {}
        Err(status) => return Err(status).context("lookup emulator Spanner instance"),
    }

    let request = CreateInstanceRequest {
        parent: project_parent.to_string(),
        instance_id: instance_id.to_string(),
        instance: Some(Instance {
            name: instance_name.clone(),
            config: format!("{project_parent}/instanceConfigs/emulator-config"),
            display_name: format!("Zitadel {instance_id}"),
            node_count: 1,
            processing_units: 0,
            autoscaling_config: None,
            state: 0,
            labels: Default::default(),
            endpoint_uris: vec![],
            create_time: None,
            update_time: None,
            edition: 0,
        }),
    };

    match admin.instance().create_instance(request, None).await {
        Ok(mut operation) => {
            operation
                .wait(None)
                .await
                .context("wait for emulator Spanner instance creation")?
                .context("emulator Spanner instance creation returned no instance")?;
            Ok(())
        }
        Err(status) if status.code() == Code::AlreadyExists => Ok(()),
        Err(status) => Err(status).context("create emulator Spanner instance"),
    }
}

async fn ensure_database_exists(
    admin: &AdminClient,
    parsed: &ParsedDatabaseName,
) -> anyhow::Result<()> {
    match admin
        .database()
        .get_database(
            GetDatabaseRequest {
                name: parsed.full_name.clone(),
            },
            None,
        )
        .await
    {
        Ok(_) => return Ok(()),
        Err(status) if status.code() == Code::NotFound => {}
        Err(status) => return Err(status).context("lookup emulator Spanner database"),
    }

    let request = CreateDatabaseRequest {
        parent: parsed.parent.clone(),
        create_statement: format!("CREATE DATABASE `{}`", parsed.database_id),
        extra_statements: vec![],
        encryption_config: None,
        database_dialect: DatabaseDialect::GoogleStandardSql.into(),
        proto_descriptors: vec![],
    };

    match admin.database().create_database(request, None).await {
        Ok(mut operation) => {
            operation
                .wait(None)
                .await
                .context("wait for emulator Spanner database creation")?
                .context("emulator Spanner database creation returned no database")?;
            Ok(())
        }
        Err(status) if status.code() == Code::AlreadyExists => Ok(()),
        Err(status) => Err(status).context("create emulator Spanner database"),
    }
}

fn read_env(key: &str) -> Option<String> {
    env::var(key)
        .ok()
        .map(|value| value.trim().to_string())
        .and_then(|value| if value.is_empty() { None } else { Some(value) })
}

fn unique_database_id(prefix: &str, suite: &str) -> String {
    let prefix = sanitize_component(prefix, 12);
    let suite = sanitize_component(suite, 10);
    let suffix = Uuid::new_v4().simple().to_string()[..6].to_string();
    let mut candidate = format!("{prefix}-{suite}-{suffix}");

    if candidate.len() > 30 {
        candidate.truncate(30);
    }
    while candidate.ends_with('-') || candidate.ends_with('_') {
        candidate.pop();
    }
    if !candidate
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
    {
        candidate.insert(0, 't');
    }
    if candidate.len() < 2 {
        candidate.push('0');
    }
    candidate
}

fn sanitize_component(raw: &str, max_len: usize) -> String {
    let mut out = String::with_capacity(raw.len().min(max_len));
    let mut previous_was_separator = false;
    for ch in raw.chars().flat_map(|ch| ch.to_lowercase()) {
        let normalized = if ch.is_ascii_lowercase() || ch.is_ascii_digit() {
            previous_was_separator = false;
            Some(ch)
        } else if (ch == '-' || ch == '_') && !previous_was_separator {
            previous_was_separator = true;
            Some('-')
        } else if previous_was_separator {
            None
        } else {
            previous_was_separator = true;
            Some('-')
        };

        if let Some(ch) = normalized {
            out.push(ch);
            if out.len() >= max_len {
                break;
            }
        }
    }

    let trimmed = out.trim_matches(|ch| ch == '-' || ch == '_');
    if trimmed.is_empty() {
        "zitadel".chars().take(max_len).collect()
    } else if trimmed
        .chars()
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase())
    {
        trimmed.to_string()
    } else {
        format!(
            "t{}",
            trimmed
                .chars()
                .take(max_len.saturating_sub(1))
                .collect::<String>()
        )
    }
}
