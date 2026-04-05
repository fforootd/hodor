use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Identity established by the AuthN layer (token/session/cookie verification).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    pub user_id: String,
    pub session_id: String,
    pub token_type: String,
    pub org_id: String,
}

/// Capability granted outside normal FGA relations (e.g., operator_admin).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    OperatorAdmin,
    Custom(String),
}

/// Authentication context resolved by the AuthN layer.
#[derive(Clone, Debug)]
pub struct AuthContext {
    pub identity: Identity,
    pub capabilities: Vec<Capability>,
}

impl AuthContext {
    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.capabilities.contains(cap)
    }

    pub fn is_operator_admin(&self) -> bool {
        self.has_capability(&Capability::OperatorAdmin)
    }
}

/// Instance context resolved by the instance resolution middleware.
#[derive(Clone, Debug)]
pub struct InstanceContext {
    pub instance_id: String,
    pub placement_mode: String,
    pub region_key: Option<String>,
    pub feature_overrides: HashMap<String, serde_json::Value>,
    pub host: String,
}

impl InstanceContext {
    /// Check if a feature flag is enabled for this instance.
    pub fn feature_enabled(&self, feature: &str) -> bool {
        self.feature_overrides
            .get(feature)
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    }
}

/// Combined actor context passed to use cases.
/// Built by the transport adapter from middleware-provided identity and instance info.
#[derive(Clone, Debug)]
pub struct ActorContext {
    pub auth: AuthContext,
    pub instance: InstanceContext,
}

impl ActorContext {
    pub fn instance_id(&self) -> &str {
        &self.instance.instance_id
    }

    pub fn user_id(&self) -> &str {
        &self.auth.identity.user_id
    }

    pub fn org_id(&self) -> &str {
        &self.auth.identity.org_id
    }

    pub fn is_operator_admin(&self) -> bool {
        self.auth.is_operator_admin()
    }
}

/// Request-level context for transport-phase hooks (rate limiting, IP filtering).
/// Available before authentication.
#[derive(Clone, Debug)]
pub struct RequestContext {
    pub method: String,
    pub path: String,
    pub remote_ip: String,
    pub headers: HashMap<String, String>,
    pub instance_id: String,
}
