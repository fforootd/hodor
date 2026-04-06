//! Hook pipeline engine — loads action definitions from the database
//! and produces [`PolicyInterceptor`] and [`EffectHook`] implementations
//! that evaluate trigger expressions via CEL (zitadel-expr).
//!
//! # Architecture
//!
//! Actions stored in the `actions` table map to hook phases via the `hook` field:
//!
//! | `hook` value       | Phase            | Contract            |
//! |--------------------|------------------|---------------------|
//! | `"request"`        | Request          | PolicyInterceptor   |
//! | `"auth"`           | Auth             | PolicyInterceptor   |
//! | `"pre_validate"`   | PreValidate      | PolicyInterceptor   |
//! | `"pre_commit"`     | PreCommit        | PolicyInterceptor   |
//! | `"post_commit"`    | PostCommit       | EffectHook          |
//! | `"post_event"`     | PostEvent        | EffectHook          |
//!
//! Each action's `trigger_expr` is a CEL expression evaluated against a
//! [`HookContext`] environment. If it evaluates to `true`, the action fires.
//! The `action_type` field determines what happens:
//!
//! - `"deny"` — returns `InterceptResult::Deny` (interceptor only)
//! - `"require_step_up"` — returns `InterceptResult::RequireStepUp` (interceptor only)
//! - `"log"` — logs the event (effect hook)
//! - `"webhook"` — future: calls an external URL (effect hook, currently logs a TODO)
//! - `"expr"` — evaluates config as a CEL expression (interceptor: deny/continue based on result)

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::event::DomainEvent;
use crate::hook::{
    ContextPatch, DenyReason, EffectHook, HookContext, HookPhase, HookPipeline, InterceptResult,
    PolicyInterceptor, StepUpKind,
};
use crate::repo::{ActionRecord, ActionRepository, ListParams};

/// A single loaded action definition, ready for evaluation.
#[derive(Clone, Debug)]
struct LoadedAction {
    id: String,
    name: String,
    trigger_expr: String,
    action_type: String,
    config: serde_json::Value,
    priority: i32,
    fail_open: bool,
}

impl From<&ActionRecord> for LoadedAction {
    fn from(r: &ActionRecord) -> Self {
        Self {
            id: r.id.clone(),
            name: r.name.clone(),
            trigger_expr: r.trigger_expr.clone(),
            action_type: r.action_type.clone(),
            config: r.config.clone(),
            priority: r.priority,
            fail_open: r.fail_open,
        }
    }
}

/// Evaluate a trigger expression against the hook context.
/// Returns `true` if the action should fire.
fn trigger_matches(trigger_expr: &str, ctx: &HookContext) -> bool {
    if trigger_expr.is_empty() || trigger_expr == "true" {
        return true;
    }
    if trigger_expr == "false" {
        return false;
    }

    let env = serde_json::json!({
        "instance_id": ctx.instance_id,
        "actor_id": ctx.actor_id,
        "org_id": ctx.org_id,
        "operation": ctx.operation,
        "metadata": ctx.metadata,
    });

    match zitadel_expr::eval(trigger_expr, &env) {
        Ok(val) => val.as_bool().unwrap_or(false),
        Err(e) => {
            tracing::warn!(
                trigger_expr,
                error = %e,
                "trigger expression evaluation failed, treating as non-match"
            );
            false
        }
    }
}

// ─── PolicyInterceptor implementation ────────────────────────

/// A policy interceptor backed by a set of action definitions.
///
/// Actions are evaluated in priority order. The first `Deny` or `RequireStepUp`
/// short-circuits the pipeline.
pub struct ActionPolicyInterceptor {
    actions: Vec<LoadedAction>,
    phase: HookPhase,
}

impl ActionPolicyInterceptor {
    fn new(actions: Vec<LoadedAction>, phase: HookPhase) -> Self {
        Self { actions, phase }
    }
}

impl PolicyInterceptor for ActionPolicyInterceptor {
    fn intercept<'a>(
        &'a self,
        phase: HookPhase,
        ctx: &'a HookContext,
    ) -> Pin<Box<dyn Future<Output = InterceptResult> + Send + 'a>> {
        // Clone ctx into owned data so the async block only borrows &self.
        let ctx = ctx.clone();
        Box::pin(async move {
            if phase != self.phase {
                return InterceptResult::Continue;
            }

            for action in &self.actions {
                let _span = tracing::info_span!(
                    "hook.intercept",
                    action.id = %action.id,
                    action.name = %action.name,
                    phase = ?phase,
                )
                .entered();

                // Evaluate trigger expression
                let ctx_ref = &ctx;
                let matches = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    trigger_matches(&action.trigger_expr, ctx_ref)
                }));

                let matches = match matches {
                    Ok(m) => m,
                    Err(_) => {
                        tracing::error!(
                            action.id = %action.id,
                            "trigger expression panicked"
                        );
                        if action.fail_open {
                            continue;
                        }
                        return InterceptResult::Deny(DenyReason {
                            code: "hook_error".into(),
                            message: format!("action '{}' trigger expression failed", action.name),
                        });
                    }
                };

                if !matches {
                    continue;
                }

                // Action triggered — determine result based on action_type
                match action.action_type.as_str() {
                    "deny" => {
                        let message = action
                            .config
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("denied by policy");
                        let code = action
                            .config
                            .get("code")
                            .and_then(|v| v.as_str())
                            .unwrap_or("policy_denied");
                        tracing::info!(
                            action.id = %action.id,
                            action.name = %action.name,
                            "action denied request"
                        );
                        return InterceptResult::Deny(DenyReason {
                            code: code.into(),
                            message: message.into(),
                        });
                    }
                    "require_step_up" => {
                        let kind = action
                            .config
                            .get("step_up_kind")
                            .and_then(|v| v.as_str())
                            .unwrap_or("otp");
                        let step_up = match kind {
                            "otp" => StepUpKind::Otp,
                            "captcha" => StepUpKind::Captcha,
                            "passkey" => StepUpKind::Passkey,
                            other => StepUpKind::Custom(other.into()),
                        };
                        tracing::info!(
                            action.id = %action.id,
                            action.name = %action.name,
                            step_up_kind = kind,
                            "action requires step-up auth"
                        );
                        return InterceptResult::RequireStepUp(step_up);
                    }
                    "mutate_context" => {
                        if let Some(merge) = action.config.get("metadata_merge") {
                            tracing::debug!(
                                action.id = %action.id,
                                action.name = %action.name,
                                "action mutating context"
                            );
                            return InterceptResult::MutateContext(ContextPatch {
                                metadata_merge: merge.clone(),
                            });
                        }
                        // No metadata to merge — continue
                    }
                    "expr" => {
                        // Evaluate config.expr as a CEL expression.
                        // If it returns true, deny; if false, continue.
                        if let Some(expr_str) = action.config.get("expr").and_then(|v| v.as_str()) {
                            let env = serde_json::json!({
                                "instance_id": ctx.instance_id,
                                "actor_id": ctx.actor_id,
                                "org_id": ctx.org_id,
                                "operation": ctx.operation,
                                "metadata": ctx.metadata,
                            });
                            match zitadel_expr::eval(expr_str, &env) {
                                Ok(val) if val.as_bool().unwrap_or(false) => {
                                    let message = action
                                        .config
                                        .get("deny_message")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("denied by expression");
                                    return InterceptResult::Deny(DenyReason {
                                        code: "expr_denied".into(),
                                        message: message.into(),
                                    });
                                }
                                Ok(_) => {} // Expression returned false — continue
                                Err(e) => {
                                    tracing::warn!(
                                        action.id = %action.id,
                                        error = %e,
                                        "action expression evaluation failed"
                                    );
                                    if !action.fail_open {
                                        return InterceptResult::Deny(DenyReason {
                                            code: "hook_error".into(),
                                            message: format!(
                                                "action '{}' expression failed: {e}",
                                                action.name
                                            ),
                                        });
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        // Unknown action_type for interceptor — log and continue
                        tracing::debug!(
                            action.id = %action.id,
                            action_type = %action.action_type,
                            "unknown action type for interceptor, skipping"
                        );
                    }
                }
            }

            InterceptResult::Continue
        })
    }
}

// ─── EffectHook implementation ───────────────────────────────

/// An effect hook backed by a set of action definitions.
///
/// Effect hooks fire after commit or after event consumption.
/// They cannot block the operation.
pub struct ActionEffectHook {
    actions: Vec<LoadedAction>,
    phase: HookPhase,
}

impl ActionEffectHook {
    fn new(actions: Vec<LoadedAction>, phase: HookPhase) -> Self {
        Self { actions, phase }
    }
}

impl EffectHook for ActionEffectHook {
    fn on_event<'a>(
        &'a self,
        phase: HookPhase,
        ctx: &'a HookContext,
        event: Option<&'a DomainEvent>,
    ) -> Pin<Box<dyn Future<Output = Result<(), anyhow::Error>> + Send + 'a>> {
        Box::pin(async move {
            if phase != self.phase {
                return Ok(());
            }

            for action in &self.actions {
                let _span = tracing::info_span!(
                    "hook.effect",
                    action.id = %action.id,
                    action.name = %action.name,
                    phase = ?phase,
                )
                .entered();

                // Build environment with event data if available
                let trigger_matches =
                    if action.trigger_expr.is_empty() || action.trigger_expr == "true" {
                        true
                    } else {
                        let mut env = serde_json::json!({
                            "instance_id": ctx.instance_id,
                            "actor_id": ctx.actor_id,
                            "org_id": ctx.org_id,
                            "operation": ctx.operation,
                            "metadata": ctx.metadata,
                        });

                        // Enrich environment with event data for PostCommit/PostEvent phases
                        if let Some(event) = event
                            && let serde_json::Value::Object(map) = &mut env
                        {
                            map.insert(
                                "event_type".into(),
                                serde_json::Value::String(event.event_type().into()),
                            );
                            map.insert(
                                "category".into(),
                                serde_json::Value::String(event.category().into()),
                            );
                            map.insert(
                                "aggregate_id".into(),
                                serde_json::Value::String(event.aggregate_id().into()),
                            );
                        }

                        match zitadel_expr::eval(&action.trigger_expr, &env) {
                            Ok(val) => val.as_bool().unwrap_or(false),
                            Err(e) => {
                                tracing::warn!(
                                    action.id = %action.id,
                                    error = %e,
                                    "effect trigger expression failed"
                                );
                                if action.fail_open {
                                    continue;
                                }
                                return Err(anyhow::anyhow!(
                                    "action '{}' trigger expression failed: {e}",
                                    action.name
                                ));
                            }
                        }
                    };

                if !trigger_matches {
                    continue;
                }

                // Execute the effect based on action_type
                match action.action_type.as_str() {
                    "log" => {
                        let event_info = event
                            .map(|e| e.event_type().to_string())
                            .unwrap_or_else(|| "none".into());
                        tracing::info!(
                            action.id = %action.id,
                            action.name = %action.name,
                            event_type = %event_info,
                            "effect hook fired (log)"
                        );
                    }
                    "webhook" => {
                        // Webhook delivery uses the durable effects system.
                        // The event consumer worker will create Effect records
                        // for matching webhook actions once it gains access to
                        // EffectRepository. Until then, log the intent.
                        let url = action
                            .config
                            .get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("<unconfigured>");
                        let event_info = event
                            .map(|e| e.event_type().to_string())
                            .unwrap_or_else(|| "none".into());
                        tracing::info!(
                            action.id = %action.id,
                            action.name = %action.name,
                            webhook.url = %url,
                            event_type = %event_info,
                            "webhook action matched — durable effect delivery pending"
                        );
                    }
                    "expr" => {
                        // Evaluate config.expr as a side-effect expression (result discarded)
                        if let Some(expr_str) = action.config.get("expr").and_then(|v| v.as_str()) {
                            let env = serde_json::json!({
                                "instance_id": ctx.instance_id,
                                "actor_id": ctx.actor_id,
                                "org_id": ctx.org_id,
                                "operation": ctx.operation,
                                "metadata": ctx.metadata,
                            });
                            match zitadel_expr::eval(expr_str, &env) {
                                Ok(_) => {
                                    tracing::debug!(
                                        action.id = %action.id,
                                        "effect expression evaluated"
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        action.id = %action.id,
                                        error = %e,
                                        "effect expression failed"
                                    );
                                    if !action.fail_open {
                                        return Err(anyhow::anyhow!(
                                            "action '{}' effect expression failed: {e}",
                                            action.name
                                        ));
                                    }
                                }
                            }
                        }
                    }
                    other => {
                        tracing::debug!(
                            action.id = %action.id,
                            action_type = %other,
                            "unknown action type for effect hook, skipping"
                        );
                    }
                }
            }

            Ok(())
        })
    }
}

// ─── HookPipeline builder ────────────────────────────────────

/// Build a [`HookPipeline`] from action records loaded from the database.
///
/// Loads all enabled actions for the given instance, groups them by hook phase,
/// sorts by priority, and wraps them in the appropriate trait implementation.
pub struct HookPipelineBuilder;

impl HookPipelineBuilder {
    /// Load actions from the repository and build a populated [`HookPipeline`].
    ///
    /// This is called at startup and whenever actions are modified.
    pub async fn build(
        action_repo: &dyn ActionRepository,
        instance_id: &str,
    ) -> anyhow::Result<HookPipeline> {
        // Load all actions (use a large limit for startup load)
        let list_result = action_repo
            .list(
                instance_id,
                &ListParams {
                    limit: Some(1000),
                    cursor: None,
                    search: None,
                },
            )
            .await?;

        let enabled_actions: Vec<ActionRecord> = list_result
            .items
            .into_iter()
            .filter(|a| a.enabled)
            .collect();

        Self::build_from_records(&enabled_actions)
    }

    /// Build a pipeline from a pre-loaded set of action records.
    /// Useful for testing or when actions are already in memory.
    pub fn build_from_records(actions: &[ActionRecord]) -> anyhow::Result<HookPipeline> {
        // Group actions by hook phase
        let mut request_actions = Vec::new();
        let mut auth_actions = Vec::new();
        let mut pre_validate_actions = Vec::new();
        let mut pre_commit_actions = Vec::new();
        let mut post_commit_actions = Vec::new();
        let mut post_event_actions = Vec::new();

        for action in actions {
            if !action.enabled {
                continue;
            }

            let loaded = LoadedAction::from(action);
            match action.hook.as_str() {
                "request" => request_actions.push(loaded),
                "auth" => auth_actions.push(loaded),
                "pre_validate" => pre_validate_actions.push(loaded),
                "pre_commit" => pre_commit_actions.push(loaded),
                "post_commit" => post_commit_actions.push(loaded),
                "post_event" | "on_event" => post_event_actions.push(loaded),
                unknown => {
                    tracing::warn!(
                        action.id = %action.id,
                        hook = %unknown,
                        "unknown hook phase, skipping action"
                    );
                }
            }
        }

        // Sort each group by priority (lower = earlier)
        for group in [
            &mut request_actions,
            &mut auth_actions,
            &mut pre_validate_actions,
            &mut pre_commit_actions,
            &mut post_commit_actions,
            &mut post_event_actions,
        ] {
            group.sort_by_key(|a| a.priority);
        }

        Ok(HookPipeline {
            request_interceptors: build_interceptor_vec(request_actions, HookPhase::Request),
            auth_interceptors: build_interceptor_vec(auth_actions, HookPhase::Auth),
            pre_validate_interceptors: build_interceptor_vec(
                pre_validate_actions,
                HookPhase::PreValidate,
            ),
            pre_commit_interceptors: build_interceptor_vec(
                pre_commit_actions,
                HookPhase::PreCommit,
            ),
            post_commit_effects: build_effect_vec(post_commit_actions, HookPhase::PostCommit),
            post_event_effects: build_effect_vec(post_event_actions, HookPhase::PostEvent),
        })
    }
}

fn build_interceptor_vec(
    actions: Vec<LoadedAction>,
    phase: HookPhase,
) -> Vec<Arc<dyn PolicyInterceptor>> {
    if actions.is_empty() {
        return Vec::new();
    }
    vec![Arc::new(ActionPolicyInterceptor::new(actions, phase))]
}

fn build_effect_vec(actions: Vec<LoadedAction>, phase: HookPhase) -> Vec<Arc<dyn EffectHook>> {
    if actions.is_empty() {
        return Vec::new();
    }
    vec![Arc::new(ActionEffectHook::new(actions, phase))]
}

// ─── Tests ───────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook::HookPhase;

    fn make_action(
        name: &str,
        hook: &str,
        action_type: &str,
        trigger_expr: &str,
        priority: i32,
    ) -> ActionRecord {
        ActionRecord {
            id: format!("action-{name}"),
            org_id: "org-1".into(),
            name: name.into(),
            hook: hook.into(),
            action_type: action_type.into(),
            trigger_expr: trigger_expr.into(),
            config: serde_json::json!({}),
            priority,
            enabled: true,
            fail_open: false,
            metadata: serde_json::json!({}),
            created_at: "2026-01-01T00:00:00Z".into(),
        }
    }

    fn make_ctx() -> HookContext {
        HookContext {
            instance_id: "inst-1".into(),
            actor_id: "user-1".into(),
            org_id: "org-1".into(),
            operation: "user.create".into(),
            metadata: serde_json::Value::Null,
        }
    }

    #[test]
    fn trigger_matches_empty_is_true() {
        let ctx = make_ctx();
        assert!(trigger_matches("", &ctx));
        assert!(trigger_matches("true", &ctx));
    }

    #[test]
    fn trigger_matches_false_literal() {
        let ctx = make_ctx();
        assert!(!trigger_matches("false", &ctx));
    }

    #[test]
    fn trigger_matches_operation_filter() {
        let ctx = make_ctx();
        assert!(trigger_matches(r#"operation == "user.create""#, &ctx));
        assert!(!trigger_matches(r#"operation == "user.delete""#, &ctx));
    }

    #[test]
    fn trigger_matches_complex_expression() {
        let ctx = make_ctx();
        assert!(trigger_matches(
            r#"operation.startsWith("user.") && org_id == "org-1""#,
            &ctx
        ));
    }

    #[test]
    fn trigger_matches_invalid_expr_returns_false() {
        let ctx = make_ctx();
        assert!(!trigger_matches("this is not valid CEL !!!", &ctx));
    }

    #[tokio::test]
    async fn interceptor_deny_action() {
        let mut action = make_action("block-users", "pre_validate", "deny", "true", 0);
        action.config = serde_json::json!({
            "message": "user creation disabled",
            "code": "user_creation_disabled",
        });

        let interceptor =
            ActionPolicyInterceptor::new(vec![LoadedAction::from(&action)], HookPhase::PreValidate);

        let ctx = make_ctx();
        let result = interceptor.intercept(HookPhase::PreValidate, &ctx).await;
        match result {
            InterceptResult::Deny(reason) => {
                assert_eq!(reason.code, "user_creation_disabled");
                assert_eq!(reason.message, "user creation disabled");
            }
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn interceptor_continues_when_trigger_doesnt_match() {
        let action = make_action(
            "block-deletes",
            "pre_validate",
            "deny",
            r#"operation == "user.delete""#,
            0,
        );

        let interceptor =
            ActionPolicyInterceptor::new(vec![LoadedAction::from(&action)], HookPhase::PreValidate);

        let ctx = make_ctx(); // operation is "user.create"
        let result = interceptor.intercept(HookPhase::PreValidate, &ctx).await;
        assert!(matches!(result, InterceptResult::Continue));
    }

    #[tokio::test]
    async fn interceptor_wrong_phase_continues() {
        let action = make_action("block-all", "pre_validate", "deny", "true", 0);
        let interceptor =
            ActionPolicyInterceptor::new(vec![LoadedAction::from(&action)], HookPhase::PreValidate);

        let ctx = make_ctx();
        // Call with a different phase
        let result = interceptor.intercept(HookPhase::PreCommit, &ctx).await;
        assert!(matches!(result, InterceptResult::Continue));
    }

    #[tokio::test]
    async fn interceptor_require_step_up() {
        let mut action = make_action("require-otp", "pre_commit", "require_step_up", "true", 0);
        action.config = serde_json::json!({ "step_up_kind": "captcha" });

        let interceptor =
            ActionPolicyInterceptor::new(vec![LoadedAction::from(&action)], HookPhase::PreCommit);

        let ctx = make_ctx();
        let result = interceptor.intercept(HookPhase::PreCommit, &ctx).await;
        assert!(matches!(
            result,
            InterceptResult::RequireStepUp(StepUpKind::Captcha)
        ));
    }

    #[tokio::test]
    async fn effect_hook_fires_on_match() {
        let action = make_action(
            "log-creates",
            "post_commit",
            "log",
            r#"operation == "user.create""#,
            0,
        );
        let hook = ActionEffectHook::new(vec![LoadedAction::from(&action)], HookPhase::PostCommit);

        let ctx = make_ctx();
        let result = hook.on_event(HookPhase::PostCommit, &ctx, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn effect_hook_skips_when_trigger_doesnt_match() {
        let action = make_action(
            "log-deletes",
            "post_commit",
            "log",
            r#"operation == "user.delete""#,
            0,
        );
        let hook = ActionEffectHook::new(vec![LoadedAction::from(&action)], HookPhase::PostCommit);

        let ctx = make_ctx(); // operation is "user.create"
        let result = hook.on_event(HookPhase::PostCommit, &ctx, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn effect_hook_with_event_data() {
        let action = make_action(
            "log-user-events",
            "post_event",
            "log",
            r#"category == "user""#,
            0,
        );
        let hook = ActionEffectHook::new(vec![LoadedAction::from(&action)], HookPhase::PostEvent);

        let ctx = make_ctx();
        let event = DomainEvent::UserCreated {
            user_id: "u-1".into(),
            org_id: "org-1".into(),
            identifier: "alice".into(),
            schema_type: "default".into(),
            actor_id: "admin".into(),
        };
        let result = hook
            .on_event(HookPhase::PostEvent, &ctx, Some(&event))
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn build_pipeline_from_records() {
        let actions = vec![
            make_action("rate-limit", "request", "deny", r#"false"#, 10),
            make_action("block-users", "pre_validate", "deny", "true", 0),
            make_action("log-all", "post_commit", "log", "true", 0),
            make_action("webhook", "post_event", "webhook", "true", 5),
        ];

        let pipeline = HookPipelineBuilder::build_from_records(&actions).unwrap();
        assert_eq!(pipeline.request_interceptors.len(), 1);
        assert_eq!(pipeline.auth_interceptors.len(), 0);
        assert_eq!(pipeline.pre_validate_interceptors.len(), 1);
        assert_eq!(pipeline.pre_commit_interceptors.len(), 0);
        assert_eq!(pipeline.post_commit_effects.len(), 1);
        assert_eq!(pipeline.post_event_effects.len(), 1);
    }

    #[test]
    fn build_pipeline_skips_disabled() {
        let mut action = make_action("disabled-hook", "pre_validate", "deny", "true", 0);
        action.enabled = false;

        let pipeline = HookPipelineBuilder::build_from_records(&[action]).unwrap();
        assert_eq!(pipeline.pre_validate_interceptors.len(), 0);
    }

    #[test]
    fn build_pipeline_handles_legacy_on_event_hook() {
        let action = make_action("legacy", "on_event", "log", "true", 0);
        let pipeline = HookPipelineBuilder::build_from_records(&[action]).unwrap();
        assert_eq!(pipeline.post_event_effects.len(), 1);
    }
}
