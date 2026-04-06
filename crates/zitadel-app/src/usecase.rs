use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::hook::{EffectHook, HookContext, HookPhase, PolicyInterceptor};
use std::sync::Arc;

/// Core trait for all business operations.
///
/// Each use case receives a typed command and produces a typed result.
/// Transport adapters (HTTP handlers, login routes, OIDC endpoints, CLI)
/// build the command, call execute(), and map the result to their response format.
pub trait UseCase: Send + Sync {
    type Command: Send;
    type Result: Send;
    type Error: Send + Into<AppError>;

    fn execute(
        &self,
        ctx: &ActorContext,
        cmd: Self::Command,
    ) -> impl Future<Output = Result<Self::Result, Self::Error>> + Send;
}

use std::future::Future;

/// Orchestrates use case execution with hooks.
///
/// Wraps a use case and runs interceptors and effect hooks at the appropriate phases.
/// This is the primary entry point for transport adapters.
pub struct UseCaseRunner {
    pre_validate: Vec<Arc<dyn PolicyInterceptor>>,
    pre_commit: Vec<Arc<dyn PolicyInterceptor>>,
    post_commit: Vec<Arc<dyn EffectHook>>,
}

impl UseCaseRunner {
    pub fn new(
        pre_validate: Vec<Arc<dyn PolicyInterceptor>>,
        pre_commit: Vec<Arc<dyn PolicyInterceptor>>,
        post_commit: Vec<Arc<dyn EffectHook>>,
    ) -> Self {
        Self {
            pre_validate,
            pre_commit,
            post_commit,
        }
    }

    /// Run a use case with the full hook pipeline:
    /// 1. Pre-validate interceptors
    /// 2. Use case execution (includes domain validation + repository calls + event append)
    /// 3. Post-commit effect hooks
    pub async fn run<U: UseCase>(
        &self,
        usecase: &U,
        ctx: &ActorContext,
        cmd: U::Command,
        operation: &str,
    ) -> Result<U::Result, AppError> {
        // Phase: PreValidate interceptors
        let hook_ctx = HookContext {
            instance_id: ctx.instance_id().to_string(),
            actor_id: ctx.user_id().to_string(),
            org_id: ctx.org_id().to_string(),
            operation: operation.to_string(),
            metadata: serde_json::Value::Null,
        };

        run_interceptors(&self.pre_validate, HookPhase::PreValidate, &hook_ctx).await?;

        // Phase: PreCommit interceptors
        run_interceptors(&self.pre_commit, HookPhase::PreCommit, &hook_ctx).await?;

        // Execute the use case (validation + persist + event in same TX)
        let result = usecase.execute(ctx, cmd).await.map_err(Into::into)?;

        // Phase: PostCommit effects (fire-and-forget)
        run_effects(&self.post_commit, HookPhase::PostCommit, &hook_ctx, None).await;

        Ok(result)
    }

    /// Run a closure-based use case with the full hook pipeline.
    ///
    /// This avoids requiring use cases to implement the formal `UseCase` trait,
    /// while still running interceptors and effects at the correct phases.
    ///
    /// # Example
    ///
    /// ```ignore
    /// state.app.runner.run_fn(&ctx, "user.create", || {
    ///     state.app.create_user.execute(&ctx, cmd)
    /// }).await
    /// ```
    pub async fn run_fn<F, Fut, R>(
        &self,
        ctx: &ActorContext,
        operation: &str,
        f: F,
    ) -> Result<R, AppError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<R, AppError>> + Send,
    {
        let hook_ctx = HookContext {
            instance_id: ctx.instance_id().to_string(),
            actor_id: ctx.user_id().to_string(),
            org_id: ctx.org_id().to_string(),
            operation: operation.to_string(),
            metadata: serde_json::Value::Null,
        };

        // Phase: PreValidate interceptors
        run_interceptors(&self.pre_validate, HookPhase::PreValidate, &hook_ctx).await?;

        // Phase: PreCommit interceptors
        run_interceptors(&self.pre_commit, HookPhase::PreCommit, &hook_ctx).await?;

        // Execute the use case
        let result = f().await?;

        // Phase: PostCommit effects (fire-and-forget)
        run_effects(&self.post_commit, HookPhase::PostCommit, &hook_ctx, None).await;

        Ok(result)
    }
}

/// Run policy interceptors in priority order. First Deny short-circuits.
async fn run_interceptors(
    interceptors: &[Arc<dyn PolicyInterceptor>],
    phase: HookPhase,
    ctx: &HookContext,
) -> Result<(), AppError> {
    for interceptor in interceptors {
        match interceptor.intercept(phase, ctx).await {
            crate::hook::InterceptResult::Continue => continue,
            crate::hook::InterceptResult::Deny(reason) => {
                return Err(AppError::PolicyDenied {
                    phase,
                    reason: reason.message,
                });
            }
            crate::hook::InterceptResult::RequireStepUp(kind) => {
                return Err(AppError::StepUpRequired { kind });
            }
            crate::hook::InterceptResult::MutateContext(_patch) => {
                // Context mutation applied — continue pipeline
                continue;
            }
        }
    }
    Ok(())
}

/// Run effect hooks. Failures are logged but do not roll back.
pub async fn run_effects(
    hooks: &[Arc<dyn EffectHook>],
    phase: HookPhase,
    ctx: &HookContext,
    event: Option<&DomainEvent>,
) {
    for hook in hooks {
        if let Err(e) = hook.on_event(phase, ctx, event).await {
            tracing::warn!(
                hook = %std::any::type_name_of_val(&**hook),
                phase = ?phase,
                error = %e,
                "effect hook failed"
            );
        }
    }
}
