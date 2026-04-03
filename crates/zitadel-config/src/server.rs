use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ServerConfig {
    pub port: u16,
    pub external_domain: String,
    pub public_origin: String,
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
            public_origin: String::new(),
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PathOverrideConfig {
    pub oidc: String,
    pub saml: String,
    pub api: String,
    pub login: String,
    pub console: String,
    pub assets: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
