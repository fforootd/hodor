use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ServerConfig;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct EncryptionConfig {
    pub active_key_id: String,
    pub keys: Vec<EncryptionKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EncryptionKey {
    pub id: String,
    pub secret: String,
}
