use std::future::Future;
use std::pin::Pin;

/// Phases at which hooks can be attached.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HookPhase {
    /// Before routing — HTTP rate limiting, IP blocking, geo-fencing
    Request,
    /// After authN, before authZ — OTP throttling, provider-login gating
    Auth,
    /// Before use case validation — feature gates, billing checks
    PreValidate,
    /// After validation, before persist — command-specific policy, step-up auth
    PreCommit,
    /// After successful persist — notifications, webhook delivery, FGA sync
    PostCommit,
    /// After event consumption by worker — downstream provisioning, analytics enrichment
    PostEvent,
}

/// Context available to hooks at all phases.
#[derive(Clone, Debug)]
pub struct HookContext {
    pub instance_id: String,
    pub actor_id: String,
    pub org_id: String,
    pub operation: String,
    pub metadata: serde_json::Value,
}

/// Result of a policy interceptor evaluation.
#[derive(Clone, Debug)]
pub enum InterceptResult {
    /// Allow execution to continue.
    Continue,
    /// Deny execution with a reason.
    Deny(DenyReason),
    /// Require additional authentication (step-up).
    RequireStepUp(StepUpKind),
    /// Mutate the execution context (e.g., inject metadata).
    MutateContext(ContextPatch),
}

#[derive(Clone, Debug)]
pub struct DenyReason {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug)]
pub enum StepUpKind {
    Otp,
    Captcha,
    Passkey,
    Custom(String),
}

#[derive(Clone, Debug)]
pub struct ContextPatch {
    pub metadata_merge: serde_json::Value,
}

/// Synchronous policy interceptor — may block execution.
///
/// Interceptors are called in priority order. The first `Deny` or `RequireStepUp`
/// short-circuits the pipeline. If the interceptor itself errors and `fail_open`
/// is true, treat as `Continue`.
pub trait PolicyInterceptor: Send + Sync {
    fn intercept<'a>(
        &'a self,
        phase: HookPhase,
        ctx: &'a HookContext,
    ) -> Pin<Box<dyn Future<Output = InterceptResult> + Send + 'a>>;
}

/// Asynchronous effect hook — fire-after-commit, cannot block the operation.
///
/// Failures are logged but do not roll back the committed operation.
pub trait EffectHook: Send + Sync {
    fn on_event<'a>(
        &'a self,
        phase: HookPhase,
        ctx: &'a HookContext,
        event: Option<&'a crate::event::DomainEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + 'a>>;
}

/// Registry that holds all active hooks, organized by phase.
pub struct HookPipeline {
    pub request_interceptors: Vec<std::sync::Arc<dyn PolicyInterceptor>>,
    pub auth_interceptors: Vec<std::sync::Arc<dyn PolicyInterceptor>>,
    pub pre_validate_interceptors: Vec<std::sync::Arc<dyn PolicyInterceptor>>,
    pub pre_commit_interceptors: Vec<std::sync::Arc<dyn PolicyInterceptor>>,
    pub post_commit_effects: Vec<std::sync::Arc<dyn EffectHook>>,
    pub post_event_effects: Vec<std::sync::Arc<dyn EffectHook>>,
}

impl HookPipeline {
    pub fn empty() -> Self {
        Self {
            request_interceptors: Vec::new(),
            auth_interceptors: Vec::new(),
            pre_validate_interceptors: Vec::new(),
            pre_commit_interceptors: Vec::new(),
            post_commit_effects: Vec::new(),
            post_event_effects: Vec::new(),
        }
    }
}
