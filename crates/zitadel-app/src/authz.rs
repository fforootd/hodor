//! Authorization helpers for use cases (ADR-032 §7).
//!
//! Provides a standard pattern for FGA permission checks with
//! `operator_admin` bypass. Use cases call these helpers before
//! performing mutations.
//!
//! **Current mode: audit (log-only).** FGA denials are logged as warnings
//! but do not block operations. Switch to `enforce` mode by changing
//! `ENFORCE_FGA` to `true` once FGA tuple seeding is complete for all
//! instance lifecycle paths.

use crate::context::ActorContext;
use crate::error::AppError;
use crate::repo::Repositories;

/// Set to `true` to enforce FGA denials (returns 403).
/// Set to `false` for audit mode (logs denials but allows operations).
const ENFORCE_FGA: bool = false;

/// Check that the actor has `relation` on `object` in the current instance's
/// FGA store. Returns `Ok(())` if allowed, `Err(PermissionDenied)` if denied.
///
/// Operator admins bypass FGA entirely (per ADR-032 §1).
///
/// In audit mode (`ENFORCE_FGA = false`), FGA denials are logged but the
/// operation proceeds. This allows incremental rollout of FGA enforcement.
///
/// # Example
///
/// ```ignore
/// authz::require_permission(&self.repos, ctx, "admin", &format!("org:{}", org_id)).await?;
/// ```
pub async fn require_permission(
    repos: &Repositories,
    ctx: &ActorContext,
    relation: &str,
    object: &str,
) -> Result<(), AppError> {
    if ctx.is_operator_admin() {
        return Ok(());
    }

    let check_result = repos
        .fga
        .check(
            ctx.instance_id(),
            &format!("user:{}", ctx.user_id()),
            relation,
            object,
        )
        .await;

    match check_result {
        Ok(true) => Ok(()),
        Ok(false) => {
            if ENFORCE_FGA {
                Err(AppError::PermissionDenied {
                    reason: format!(
                        "user {} lacks '{}' on '{}'",
                        ctx.user_id(),
                        relation,
                        object
                    ),
                })
            } else {
                tracing::debug!(
                    instance_id = ctx.instance_id(),
                    user_id = ctx.user_id(),
                    relation,
                    object,
                    "FGA check denied (audit mode; allowing)"
                );
                Ok(())
            }
        }
        Err(e) => {
            // FGA infrastructure unavailable — fail-open with warning.
            tracing::warn!(
                instance_id = ctx.instance_id(),
                user_id = ctx.user_id(),
                relation,
                object,
                error = %e,
                "FGA check error (fail-open; allowing)"
            );
            Ok(())
        }
    }
}

/// Check that the actor has `relation` on `object`, OR that the actor is
/// the resource owner (identified by `owner_user_id`). Useful for
/// self-service operations like password changes.
pub async fn require_permission_or_self(
    repos: &Repositories,
    ctx: &ActorContext,
    relation: &str,
    object: &str,
    owner_user_id: &str,
) -> Result<(), AppError> {
    if ctx.user_id() == owner_user_id {
        return Ok(());
    }
    require_permission(repos, ctx, relation, object).await
}

/// Require that the actor has the `operator_admin` capability.
/// Returns `Err(OperatorAdminRequired)` if not.
pub fn require_operator_admin(ctx: &ActorContext) -> Result<(), AppError> {
    if ctx.is_operator_admin() {
        Ok(())
    } else {
        Err(AppError::OperatorAdminRequired)
    }
}
