use crate::hook::{HookPhase, StepUpKind};

/// Unified application-layer error type.
///
/// All use case errors are mapped into this type so transport adapters
/// can translate them to the appropriate response format (HTTP status, JSON body, etc.).
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    // ── Authorization ──
    #[error("not authenticated")]
    Unauthenticated,

    #[error("permission denied: {reason}")]
    PermissionDenied { reason: String },

    #[error("operator admin required")]
    OperatorAdminRequired,

    // ── Hook pipeline ──
    #[error("policy denied at {phase:?}: {reason}")]
    PolicyDenied { phase: HookPhase, reason: String },

    #[error("step-up authentication required: {kind:?}")]
    StepUpRequired { kind: StepUpKind },

    // ── Validation ──
    #[error("validation error: {message}")]
    Validation { message: String },

    #[error("not found: {entity} {id}")]
    NotFound { entity: String, id: String },

    #[error("already exists: {entity} {identifier}")]
    AlreadyExists { entity: String, identifier: String },

    #[error("invalid state: {entity} {id} is {current_state}, expected {expected_state}")]
    InvalidState {
        entity: String,
        id: String,
        current_state: String,
        expected_state: String,
    },

    // ── Feature gating ──
    #[error("feature not enabled: {feature}")]
    FeatureNotEnabled { feature: String },

    // ── Rate limiting ──
    #[error("rate limited: {message}")]
    RateLimited { message: String },

    // ── Internal ──
    #[error("internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn not_found(entity: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound {
            entity: entity.into(),
            id: id.into(),
        }
    }

    pub fn already_exists(entity: impl Into<String>, identifier: impl Into<String>) -> Self {
        Self::AlreadyExists {
            entity: entity.into(),
            identifier: identifier.into(),
        }
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::Validation {
            message: message.into(),
        }
    }

    /// HTTP status code hint for transport adapters.
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Unauthenticated => 401,
            Self::PermissionDenied { .. } | Self::OperatorAdminRequired => 403,
            Self::PolicyDenied { .. } => 403,
            Self::StepUpRequired { .. } => 403,
            Self::Validation { .. } => 400,
            Self::NotFound { .. } => 404,
            Self::AlreadyExists { .. } => 409,
            Self::InvalidState { .. } => 409,
            Self::FeatureNotEnabled { .. } => 403,
            Self::RateLimited { .. } => 429,
            Self::Internal(_) => 500,
        }
    }
}
