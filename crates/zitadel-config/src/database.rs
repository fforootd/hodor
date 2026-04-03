use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DatabaseConfig {
    pub url: String,
    pub migrate: String,
    pub bootstrap: String,
    pub max_open_conns: u32,
    pub max_idle_conns: u32,
    pub conn_max_lifetime: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://./data/zitadel.db".into(),
            migrate: String::new(),
            bootstrap: String::new(),
            max_open_conns: 25,
            max_idle_conns: 5,
            conn_max_lifetime: "1h".into(),
        }
    }
}

impl DatabaseConfig {
    pub fn resolve_migrate_mode(&self) -> &str {
        match self.migrate.as_str() {
            "auto" | "check" | "skip" => self.migrate.as_str(),
            _ => "auto",
        }
    }

    pub fn resolve_bootstrap_mode(&self) -> &str {
        match self.bootstrap.as_str() {
            "auto" | "skip" => self.bootstrap.as_str(),
            _ => "auto",
        }
    }
}
