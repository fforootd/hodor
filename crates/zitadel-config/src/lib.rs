use figment::{
    Figment,
    providers::{Env, Format, Serialized, Toml},
};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Runtime configuration for the Zitadel server.
/// Covers infrastructure-level settings only.
/// Domain-specific configuration (policies, whitelabeling) lives in seed YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub tls: TlsConfig,
    pub encryption: EncryptionConfig,
    pub database: DatabaseConfig,
    pub observability: ObservabilityConfig,
    pub workers: WorkersConfig,
    pub rate_limit: RateLimitConfig,
    pub catalog: CatalogConfig,
    pub dev: DevConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub port: u16,
    pub external_domain: String,
    pub force_insecure_cookies: bool,
    pub management_secret: String,
    pub tls_cert: String,
    pub tls_key: String,
    pub cookie_secrets: Vec<String>,
    pub base_path: String,
    pub path_overrides: PathOverrideConfig,
    pub app_access: AppAccessConfig,
    pub trusted_proxies: Vec<String>,
    pub proxy_header_mode: String,
    pub real_ip_header: String,
    pub multi_tenant: bool,
    pub security_headers: SecurityHeadersConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            external_domain: "localhost".into(),
            force_insecure_cookies: false,
            management_secret: String::new(),
            tls_cert: String::new(),
            tls_key: String::new(),
            cookie_secrets: Vec::new(),
            base_path: String::new(),
            path_overrides: PathOverrideConfig::default(),
            app_access: AppAccessConfig::default(),
            trusted_proxies: Vec::new(),
            proxy_header_mode: String::new(),
            real_ip_header: String::new(),
            multi_tenant: false,
            security_headers: SecurityHeadersConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PathOverrideConfig {
    pub oidc: String,
    pub saml: String,
    pub api: String,
    pub login: String,
    pub console: String,
    pub assets: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppAccessEntry {
    pub enabled: bool,
    pub ip_allow: Vec<String>,
}

impl Default for AppAccessEntry {
    fn default() -> Self {
        Self {
            enabled: true,
            ip_allow: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppAccessConfig {
    pub console: AppAccessEntry,
    pub admin: AppAccessEntry,
    pub api: AppAccessEntry,
    pub login: AppAccessEntry,
}

impl Default for AppAccessConfig {
    fn default() -> Self {
        Self {
            console: AppAccessEntry::default(),
            admin: AppAccessEntry::default(),
            api: AppAccessEntry::default(),
            login: AppAccessEntry::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityHeadersConfig {
    pub hsts_enabled: bool,
    pub hsts_max_age: u64,
    pub hsts_subdomains: bool,
    pub hsts_preload: bool,
    pub csp_enabled: bool,
    pub csp_policy: String,
    pub csp_report_uri: String,
    pub x_frame_options: String,
    pub x_content_type_options: bool,
    pub referrer_policy: String,
    pub permissions_policy: String,
    pub cross_origin_opener: String,
}

impl Default for SecurityHeadersConfig {
    fn default() -> Self {
        Self {
            hsts_enabled: true,
            hsts_max_age: 63072000,
            hsts_subdomains: true,
            hsts_preload: false,
            csp_enabled: true,
            csp_policy: String::new(),
            csp_report_uri: String::new(),
            x_frame_options: "DENY".into(),
            x_content_type_options: true,
            referrer_policy: "strict-origin-when-cross-origin".into(),
            permissions_policy: "camera=(), microphone=(), geolocation=(), payment=()".into(),
            cross_origin_opener: "same-origin".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TlsConfig {
    pub mode: String,
    pub email: String,
    pub ca_dir: String,
    pub http_port: u16,
    pub https_port: u16,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            mode: String::new(),
            email: String::new(),
            ca_dir: String::new(),
            http_port: 80,
            https_port: 443,
        }
    }
}

impl TlsConfig {
    /// Determine effective TLS mode from config + server state.
    pub fn resolve_mode(&self, server_cfg: &ServerConfig, is_dev: bool) -> &str {
        match self.mode.as_str() {
            "auto" | "manual" | "external" | "off" => return self.mode.as_str(),
            _ => {}
        }
        if !server_cfg.tls_cert.is_empty() && !server_cfg.tls_key.is_empty() {
            return "manual";
        }
        if is_dev {
            return "off";
        }
        let d = &server_cfg.external_domain;
        if !d.is_empty() && d != "localhost" && d != "127.0.0.1" && d != "0.0.0.0" {
            return "auto";
        }
        "off"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct EncryptionConfig {
    pub active_key_id: String,
    pub keys: Vec<EncryptionKey>,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            active_key_id: String::new(),
            keys: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionKey {
    pub id: String,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObservabilityConfig {
    pub log_level: String,
    pub log_format: String,
    pub cache_path: String,
    pub cache_max: u64,
    pub streams: StreamRoutingConfig,
    pub sinks: SinksConfig,
    pub redaction: RedactionConfig,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            log_level: "info".into(),
            log_format: "text".into(),
            cache_path: "./data/zitadel-cache.db".into(),
            cache_max: 50000,
            streams: StreamRoutingConfig::default(),
            sinks: SinksConfig::default(),
            redaction: RedactionConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamConfig {
    pub sinks: Vec<String>,
    pub mode: String,
    pub sample_rate: f64,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            sinks: vec!["stdout".into()],
            mode: "buffered".into(),
            sample_rate: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StreamRoutingConfig {
    pub runtime: StreamConfig,
    pub request: StreamConfig,
    pub jobs: StreamConfig,
    pub event_pusher: StreamConfig,
}

impl Default for StreamRoutingConfig {
    fn default() -> Self {
        Self {
            runtime: StreamConfig {
                sinks: vec!["stdout".into(), "analytics".into()],
                mode: "buffered".into(),
                ..Default::default()
            },
            request: StreamConfig {
                sinks: vec!["stdout".into(), "otel".into(), "analytics".into()],
                mode: "buffered".into(),
                ..Default::default()
            },
            jobs: StreamConfig {
                sinks: vec!["stdout".into(), "analytics".into()],
                mode: "buffered".into(),
                ..Default::default()
            },
            event_pusher: StreamConfig {
                mode: "off".into(),
                ..Default::default()
            },
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SinksConfig {
    pub otel: OtelSinkConfig,
    pub analytics: AnalyticsSinkConfig,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OtelSinkConfig {
    pub endpoint: String,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalyticsSinkConfig {
    pub enabled: bool,
    pub drain_interval: String,
    pub drain_batch: u32,
}

impl Default for AnalyticsSinkConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            drain_interval: "5s".into(),
            drain_batch: 500,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RedactionConfig {
    pub keys: Vec<String>,
    pub mask: String,
    pub ip_mode: String,
}

impl Default for RedactionConfig {
    fn default() -> Self {
        Self {
            keys: vec![
                "password".into(),
                "secret".into(),
                "token".into(),
                "client_secret".into(),
                "private_key".into(),
            ],
            mask: "***REDACTED***".into(),
            ip_mode: "keep".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkersConfig {
    pub notification_workers: u32,
    pub event_workers: u32,
    pub lake_batch_size: u32,
    pub lake_batch_window_secs: u32,
}

impl Default for WorkersConfig {
    fn default() -> Self {
        Self {
            notification_workers: 1,
            event_workers: 1,
            lake_batch_size: 1000,
            lake_batch_window_secs: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    pub backend: String,
    pub redis_url: String,
    pub gc_interval: u32,
    pub batch_write: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            backend: "memory".into(),
            redis_url: String::new(),
            gc_interval: 60,
            batch_write: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CatalogConfig {
    pub url: String,
    pub local_path: String,
    pub refresh_interval: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DevConfig {
    pub mock_oidc: bool,
    pub mock_oidc_port: u16,
    pub seed_file: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            tls: TlsConfig::default(),
            encryption: EncryptionConfig::default(),
            database: DatabaseConfig::default(),
            observability: ObservabilityConfig::default(),
            workers: WorkersConfig::default(),
            rate_limit: RateLimitConfig::default(),
            catalog: CatalogConfig::default(),
            dev: DevConfig::default(),
        }
    }
}

impl Config {
    /// Load config from an optional TOML file path, with env var overlays.
    /// If path is None, only defaults + env vars are used.
    ///
    /// Environment variables use ZITADEL_ prefix with __ for nesting:
    ///   ZITADEL_SERVER__PORT=9090
    ///   ZITADEL_DATABASE__URL=postgres://...
    ///
    /// Flat env vars from the Go version are also supported:
    ///   ZITADEL_PORT, ZITADEL_DATABASE_URL, etc.
    pub fn load(path: Option<&Path>) -> Result<Self, figment::Error> {
        let mut figment = Figment::from(Serialized::defaults(Config::default()));

        if let Some(p) = path {
            figment = figment.merge(Toml::file(p));
        }

        // Nested env vars: ZITADEL_SERVER__PORT etc.
        figment = figment.merge(Env::prefixed("ZITADEL_").split("__"));

        // Flat env var overrides matching Go behavior.
        figment = figment.merge(flat_env_overrides());

        figment.extract()
    }

    /// Whether this looks like a dev environment.
    pub fn is_dev(&self) -> bool {
        self.dev.mock_oidc
            || !self.dev.seed_file.is_empty()
            || self.server.external_domain == "localhost"
    }
}

/// Map flat Go-style env vars (ZITADEL_PORT) to nested config paths.
fn flat_env_overrides() -> Serialized<serde_json::Value> {
    use serde_json::{Map, Value, json};
    use std::env;

    let mut overrides = Map::new();

    // Server
    if let Ok(v) = env::var("ZITADEL_PORT") {
        if let Ok(port) = v.parse::<u16>() {
            overrides.insert("server".into(), json!({"port": port}));
        }
    }
    if let Ok(v) = env::var("ZITADEL_EXTERNAL_DOMAIN") {
        merge_into(
            &mut overrides,
            "server",
            "external_domain",
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_MANAGEMENT_SECRET") {
        merge_into(
            &mut overrides,
            "server",
            "management_secret",
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_COOKIE_SECRETS") {
        let secrets: Vec<Value> = v
            .split(',')
            .map(|s| Value::String(s.trim().to_string()))
            .collect();
        merge_into(
            &mut overrides,
            "server",
            "cookie_secrets",
            Value::Array(secrets),
        );
    }

    // Database
    if let Ok(v) = env::var("ZITADEL_DATABASE_URL") {
        merge_into(&mut overrides, "database", "url", Value::String(v));
    }
    if let Ok(v) = env::var("ZITADEL_DATABASE_MIGRATE") {
        merge_into(&mut overrides, "database", "migrate", Value::String(v));
    }

    // Observability
    if let Ok(v) = env::var("ZITADEL_LOG_LEVEL") {
        merge_into(
            &mut overrides,
            "observability",
            "log_level",
            Value::String(v),
        );
    }
    if let Ok(v) = env::var("ZITADEL_LOG_FORMAT") {
        merge_into(
            &mut overrides,
            "observability",
            "log_format",
            Value::String(v),
        );
    }

    // Dev
    if let Ok(v) = env::var("ZITADEL_MOCK_OIDC") {
        if v == "true" || v == "1" {
            merge_into(&mut overrides, "dev", "mock_oidc", Value::Bool(true));
        }
    }
    if let Ok(v) = env::var("ZITADEL_SEED_FILE") {
        merge_into(&mut overrides, "dev", "seed_file", Value::String(v));
    }

    // TLS
    if let Ok(v) = env::var("ZITADEL_TLS_MODE") {
        merge_into(&mut overrides, "tls", "mode", Value::String(v));
    }

    Serialized::defaults(Value::Object(overrides))
}

fn merge_into(
    overrides: &mut serde_json::Map<String, serde_json::Value>,
    section: &str,
    key: &str,
    value: serde_json::Value,
) {
    let section_obj = overrides
        .entry(section.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if let serde_json::Value::Object(m) = section_obj {
        m.insert(key.to_string(), value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_sensible_values() {
        let cfg = Config::default();
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(cfg.server.external_domain, "localhost");
        assert_eq!(cfg.database.url, "sqlite://./data/zitadel.db");
        assert_eq!(cfg.observability.log_level, "info");
        assert_eq!(cfg.database.resolve_migrate_mode(), "auto");
        assert_eq!(cfg.database.resolve_bootstrap_mode(), "auto");
    }

    #[test]
    fn tls_resolve_mode_dev() {
        let tls = TlsConfig::default();
        let server = ServerConfig::default();
        assert_eq!(tls.resolve_mode(&server, true), "off");
    }

    #[test]
    fn tls_resolve_mode_explicit() {
        let tls = TlsConfig {
            mode: "auto".into(),
            ..Default::default()
        };
        let server = ServerConfig::default();
        assert_eq!(tls.resolve_mode(&server, false), "auto");
    }
}
