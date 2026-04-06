//! Durable side-effect types for async delivery (webhooks, email, SMS).
//!
//! Effects are written transactionally alongside domain events via the
//! UnitOfWork. A background worker polls pending effects and dispatches
//! them with retry semantics. This is distinct from fire-and-forget
//! PostEvent hooks, which run without delivery guarantees.

use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

/// The kind of side-effect to deliver.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectType {
    Webhook,
    Email,
    Sms,
    Log,
}

impl EffectType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Webhook => "webhook",
            Self::Email => "email",
            Self::Sms => "sms",
            Self::Log => "log",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "webhook" => Some(Self::Webhook),
            "email" => Some(Self::Email),
            "sms" => Some(Self::Sms),
            "log" => Some(Self::Log),
            _ => None,
        }
    }
}

/// Processing status of an effect.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectStatus {
    /// Awaiting first dispatch attempt.
    Pending,
    /// Currently being dispatched (transient, prevents double-pickup).
    Processing,
    /// Successfully delivered.
    Completed,
    /// Failed but retryable (attempt < max_attempts).
    Failed,
    /// Max attempts exhausted — will not be retried.
    Dead,
}

impl EffectStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Dead => "dead",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "processing" => Some(Self::Processing),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "dead" => Some(Self::Dead),
            _ => None,
        }
    }
}

/// A durable side-effect queued for async delivery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Effect {
    pub id: String,
    pub event_id: String,
    pub source_key: String,
    pub effect_type: EffectType,
    pub status: EffectStatus,
    /// Delivery configuration (e.g. `{"url": "...", "headers": {...}}` for webhooks,
    /// `{"template": "welcome", "to": "user@example.com"}` for email).
    pub config: serde_json::Value,
    /// Payload to deliver (event data, rendered template content, etc.).
    pub payload: serde_json::Value,
    pub attempt: i32,
    pub max_attempts: i32,
    pub next_retry_at: String,
    pub last_error: String,
    pub lease_owner: String,
    pub lease_expires_at: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

impl Effect {
    /// Create a new pending effect with sensible defaults.
    pub fn new(
        event_id: String,
        source_key: String,
        effect_type: EffectType,
        config: serde_json::Value,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            event_id,
            source_key,
            effect_type,
            status: EffectStatus::Pending,
            config,
            payload,
            attempt: 0,
            max_attempts: 5,
            next_retry_at: String::new(), // filled by DB default
            last_error: String::new(),
            lease_owner: String::new(),
            lease_expires_at: None,
            created_at: String::new(), // filled by DB default
            completed_at: None,
        }
    }

    /// Create a new pending effect with a custom max_attempts.
    pub fn with_max_attempts(mut self, max: i32) -> Self {
        self.max_attempts = max;
        self
    }
}

/// Trait for dispatching a specific effect type.
///
/// Implementations handle the actual delivery (HTTP call, SMTP send, etc.).
/// The effects worker calls `dispatch()` and handles retry/failure tracking.
pub trait EffectDispatcher: Send + Sync {
    fn effect_type(&self) -> EffectType;

    fn dispatch<'a>(
        &'a self,
        effect: &'a Effect,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + 'a>>;
}
