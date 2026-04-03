use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct StreamRoutingConfig {
    pub runtime: StreamConfig,
    pub request: StreamConfig,
    pub jobs: StreamConfig,
    pub queue: Option<StreamConfig>,
    pub event_handler: StreamConfig,
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
                mode: "sampled".into(),
                sample_rate: 0.01,
                ..Default::default()
            },
            jobs: StreamConfig {
                sinks: vec!["stdout".into(), "analytics".into()],
                mode: "buffered".into(),
                ..Default::default()
            },
            queue: None,
            event_handler: StreamConfig {
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

impl StreamRoutingConfig {
    pub fn by_name(&self, name: &str) -> Option<&StreamConfig> {
        match name {
            "runtime" => Some(&self.runtime),
            "request" => Some(&self.request),
            "jobs" => Some(self.queue.as_ref().unwrap_or(&self.jobs)),
            "queue" => Some(self.queue.as_ref().unwrap_or(&self.jobs)),
            "event_handler" => Some(&self.event_handler),
            "event_pusher" => Some(&self.event_pusher),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SinksConfig {
    pub otel: OtelSinkConfig,
    pub analytics: AnalyticsSinkConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct OtelSinkConfig {
    pub endpoint: String,
    pub protocol: String,
}

impl Default for OtelSinkConfig {
    fn default() -> Self {
        Self {
            endpoint: String::new(),
            protocol: "http".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
