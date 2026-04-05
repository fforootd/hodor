use std::cmp;

use anyhow::Context;
use google_cloud_auth::credentials::CredentialsFile;
use google_cloud_gax::{conn::Environment, grpc::Code};
use google_cloud_googleapis::spanner::admin::database::v1::{
    CreateDatabaseRequest, DatabaseDialect, GetDatabaseDdlRequest, GetDatabaseRequest,
    UpdateDatabaseDdlRequest,
};
use google_cloud_spanner::{
    admin::{AdminClientConfig, client::Client as AdminClient},
    client::{Client, ClientConfig},
};
use zitadel_config::StatefulStorageConfig;

#[derive(Clone)]
pub struct SpannerDb {
    database: ParsedDatabaseName,
    client: Client,
    admin: AdminClient,
}

impl SpannerDb {
    pub async fn open(config: &StatefulStorageConfig) -> anyhow::Result<Self> {
        let database = ParsedDatabaseName::parse(&config.database)?;
        let client = Client::new(
            database.full_name.clone(),
            build_client_config(config).await?,
        )
        .await
        .context("open spanner data client")?;
        let admin = AdminClient::new(build_admin_config(config).await?)
            .await
            .context("open spanner admin client")?;

        Ok(Self {
            database,
            client,
            admin,
        })
    }

    pub fn database_name(&self) -> &str {
        &self.database.full_name
    }

    pub fn database_id(&self) -> &str {
        &self.database.database_id
    }

    pub fn instance_parent(&self) -> &str {
        &self.database.parent
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn admin(&self) -> &AdminClient {
        &self.admin
    }

    pub async fn close(self) {
        self.client.close().await;
    }

    pub async fn ensure_database(&self, ddl: &[String]) -> anyhow::Result<()> {
        let exists = match self
            .admin
            .database()
            .get_database(
                GetDatabaseRequest {
                    name: self.database.full_name.clone(),
                },
                None,
            )
            .await
        {
            Ok(_) => true,
            Err(status) if status.code() == Code::NotFound => false,
            Err(status) => return Err(status).context("lookup spanner database"),
        };

        if !exists {
            let request = CreateDatabaseRequest {
                parent: self.database.parent.clone(),
                create_statement: format!("CREATE DATABASE `{}`", self.database.database_id),
                extra_statements: ddl.to_vec(),
                encryption_config: None,
                database_dialect: DatabaseDialect::GoogleStandardSql.into(),
                proto_descriptors: vec![],
            };
            let mut operation = self
                .admin
                .database()
                .create_database(request, None)
                .await
                .context("create spanner database")?;
            operation
                .wait(None)
                .await
                .context("wait for spanner database creation")?
                .context("spanner database creation returned no database")?;
            return Ok(());
        }

        let existing = self.current_ddl().await?;
        let normalized_existing = normalize_ddl(&existing);
        let normalized_target = normalize_ddl(ddl);
        if !normalized_existing.is_empty() && normalized_existing != normalized_target {
            anyhow::bail!(
                "existing Spanner database schema does not match the prototype baseline; delete the database and retry"
            );
        }
        if normalized_existing.is_empty() && !ddl.is_empty() {
            let request = UpdateDatabaseDdlRequest {
                database: self.database.full_name.clone(),
                statements: ddl.to_vec(),
                operation_id: String::new(),
                proto_descriptors: vec![],
            };
            let mut operation = self
                .admin
                .database()
                .update_database_ddl(request, None)
                .await
                .context("apply spanner baseline DDL")?;
            operation
                .wait(None)
                .await
                .context("wait for spanner DDL operation")?;
        }

        Ok(())
    }

    pub async fn current_ddl(&self) -> anyhow::Result<Vec<String>> {
        let ddl = self
            .admin
            .database()
            .get_database_ddl(
                GetDatabaseDdlRequest {
                    database: self.database.full_name.clone(),
                },
                None,
            )
            .await
            .context("read spanner database DDL")?;
        Ok(ddl.into_inner().statements)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedDatabaseName {
    pub full_name: String,
    pub parent: String,
    pub database_id: String,
}

impl ParsedDatabaseName {
    pub fn parse(raw: &str) -> anyhow::Result<Self> {
        let parts: Vec<&str> = raw.split('/').collect();
        if parts.len() != 6
            || parts[0] != "projects"
            || parts[2] != "instances"
            || parts[4] != "databases"
            || parts[1].is_empty()
            || parts[3].is_empty()
            || parts[5].is_empty()
        {
            anyhow::bail!(
                "storage.stateful.database must be projects/<project>/instances/<instance>/databases/<database>"
            );
        }

        Ok(Self {
            full_name: raw.to_string(),
            parent: format!("projects/{}/instances/{}", parts[1], parts[3]),
            database_id: parts[5].to_string(),
        })
    }
}

async fn build_client_config(config: &StatefulStorageConfig) -> anyhow::Result<ClientConfig> {
    let mut client_config = ClientConfig::default();
    client_config.channel_config.num_channels =
        cmp::max(1usize, cmp::min(config.max_open_conns as usize, 8usize));
    client_config.session_config.min_opened = client_config.channel_config.num_channels;
    client_config.session_config.max_opened = cmp::max(
        client_config.channel_config.num_channels,
        config.max_open_conns as usize,
    );

    if !config.emulator_host.is_empty() {
        client_config.environment = Environment::Emulator(config.emulator_host.clone());
        return Ok(client_config);
    }

    if !config.credentials_json.is_empty() {
        let credentials = CredentialsFile::new_from_str(&config.credentials_json)
            .await
            .context("parse storage.stateful.credentials_json")?;
        return client_config
            .with_credentials(credentials)
            .await
            .context("configure spanner credentials_json");
    }

    if !config.credentials_file.is_empty() {
        let credentials = CredentialsFile::new_from_file(config.credentials_file.clone())
            .await
            .with_context(|| {
                format!(
                    "read storage.stateful.credentials_file {}",
                    config.credentials_file
                )
            })?;
        return client_config
            .with_credentials(credentials)
            .await
            .context("configure spanner credentials_file");
    }

    client_config
        .with_auth()
        .await
        .context("configure spanner ADC auth")
}

async fn build_admin_config(config: &StatefulStorageConfig) -> anyhow::Result<AdminClientConfig> {
    let client_config = AdminClientConfig::default();
    if !config.emulator_host.is_empty() {
        return Ok(AdminClientConfig {
            environment: Environment::Emulator(config.emulator_host.clone()),
        });
    }

    if !config.credentials_json.is_empty() {
        let credentials = CredentialsFile::new_from_str(&config.credentials_json)
            .await
            .context("parse storage.stateful.credentials_json")?;
        return client_config
            .with_credentials(credentials)
            .await
            .context("configure spanner admin credentials_json");
    }

    if !config.credentials_file.is_empty() {
        let credentials = CredentialsFile::new_from_file(config.credentials_file.clone())
            .await
            .with_context(|| {
                format!(
                    "read storage.stateful.credentials_file {}",
                    config.credentials_file
                )
            })?;
        return client_config
            .with_credentials(credentials)
            .await
            .context("configure spanner admin credentials_file");
    }

    client_config
        .with_auth()
        .await
        .context("configure spanner admin ADC auth")
}

fn normalize_ddl(statements: &[String]) -> Vec<String> {
    statements
        .iter()
        .map(|statement| statement.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_database_name() {
        let parsed =
            ParsedDatabaseName::parse("projects/test/instances/dev/databases/zitadel").unwrap();
        assert_eq!(parsed.parent, "projects/test/instances/dev");
        assert_eq!(parsed.database_id, "zitadel");
    }

    #[test]
    fn rejects_invalid_database_name() {
        assert!(ParsedDatabaseName::parse("postgres://localhost/zitadel").is_err());
    }
}
