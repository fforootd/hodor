//! Authorization helpers for use cases (ADR-032 §7).
//!
//! The app layer resolves product permissions from the vendored
//! role-permission catalog and uses embedded FGA as the relationship engine.
//! `operator_admin` remains the break-glass bypass.

use std::collections::BTreeSet;

use zitadel_authz::{grants_for_permission, role_grants_permission};

use crate::context::ActorContext;
use crate::error::AppError;
use crate::repo::Repositories;

/// Set to `true` to enforce FGA denials (returns 403).
/// Set to `false` for audit mode (logs denials but allows operations).
const ENFORCE_FGA: bool = true;

#[derive(Clone, Copy)]
struct PermissionAlias {
    permissions: &'static [&'static str],
}

pub async fn require_scoped_permission(
    repos: &Repositories,
    ctx: &ActorContext,
    permission_key: &str,
    scope_refs: &[String],
) -> Result<(), AppError> {
    if ctx.is_operator_admin() {
        return Ok(());
    }

    let Some(alias) = alias(permission_key) else {
        return Err(AppError::PermissionDenied {
            reason: format!("unknown permission alias '{permission_key}'"),
        });
    };

    if let Some(grant) = ctx.support_grant_for_instance(ctx.instance_id())
        && scope_refs
            .iter()
            .any(|scope| scope == &format!("instance:{}", grant.target_instance_id))
        && alias
            .permissions
            .iter()
            .any(|permission| role_grants_permission(&grant.role_key, permission))
    {
        return Ok(());
    }

    for scope in scope_refs {
        let Some((scope_kind, _)) = scope.split_once(':') else {
            continue;
        };
        let mut candidates = Vec::new();
        let mut seen = BTreeSet::new();
        for permission in alias.permissions {
            for grant in grants_for_permission(scope_kind, permission) {
                if seen.insert(grant.relation_name.clone()) {
                    candidates.push(grant);
                }
            }
        }

        for candidate in candidates {
            match repos
                .fga
                .check(
                    ctx.instance_id(),
                    ctx.principal_ref(),
                    &candidate.relation_name,
                    scope.as_str(),
                )
                .await
            {
                Ok(true) => return Ok(()),
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(
                            instance_id = ctx.instance_id(),
                            principal = ctx.principal_ref(),
                            permission_key,
                    scope,
                            role = candidate.role_key,
                            error = %error,
                            "permission check error (fail-open; allowing)"
                        );
                    return Ok(());
                }
            }
        }
    }

    if ENFORCE_FGA {
        Err(AppError::PermissionDenied {
            reason: format!(
                "principal {} lacks '{}' on {:?}",
                ctx.principal_ref(),
                permission_key,
                scope_refs
            ),
        })
    } else {
        tracing::debug!(
            instance_id = ctx.instance_id(),
            principal = ctx.principal_ref(),
            permission_key,
            ?scope_refs,
            "permission denied (audit mode; allowing)"
        );
        Ok(())
    }
}

/// Compatibility wrapper for legacy relation checks.
pub async fn require_permission(
    repos: &Repositories,
    ctx: &ActorContext,
    relation: &str,
    object: &str,
) -> Result<(), AppError> {
    if let Some((permission_key, scopes)) = relation_alias(ctx, relation, object) {
        return require_scoped_permission(repos, ctx, permission_key, &scopes).await;
    }

    if ctx.is_operator_admin() {
        return Ok(());
    }

    let check_result = repos
        .fga
        .check(ctx.instance_id(), ctx.principal_ref(), relation, object)
        .await;

    match check_result {
        Ok(true) => Ok(()),
        Ok(false) => {
            if ENFORCE_FGA {
                Err(AppError::PermissionDenied {
                    reason: format!(
                        "principal {} lacks '{}' on '{}'",
                        ctx.principal_ref(),
                        relation,
                        object
                    ),
                })
            } else {
                tracing::debug!(
                    instance_id = ctx.instance_id(),
                    principal = ctx.principal_ref(),
                    relation,
                    object,
                    "FGA check denied (audit mode; allowing)"
                );
                Ok(())
            }
        }
        Err(error) => {
            tracing::warn!(
                instance_id = ctx.instance_id(),
                principal = ctx.principal_ref(),
                relation,
                object,
                error = %error,
                "FGA check error (fail-open; allowing)"
            );
            Ok(())
        }
    }
}

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
pub fn require_operator_admin(ctx: &ActorContext) -> Result<(), AppError> {
    if ctx.is_operator_admin() {
        Ok(())
    } else {
        Err(AppError::OperatorAdminRequired)
    }
}

fn relation_alias<'a>(
    ctx: &'a ActorContext,
    relation: &str,
    object: &'a str,
) -> Option<(&'static str, Vec<String>)> {
    match (relation, object.split_once(':')?) {
        ("viewer", ("instance", _)) => Some(("instance.read", vec![object.to_string()])),
        ("admin", ("instance", _)) => Some(("instance.write", vec![object.to_string()])),
        ("viewer", ("org", _)) => Some((
            "org.read",
            vec![
                object.to_string(),
                format!("instance:{}", ctx.instance_id()),
            ],
        )),
        ("admin", ("org", _)) => Some((
            "org.write",
            vec![
                object.to_string(),
                format!("instance:{}", ctx.instance_id()),
            ],
        )),
        ("viewer", ("user", _)) => Some((
            "org.user.read",
            vec![format!("instance:{}", ctx.instance_id())],
        )),
        ("admin", ("user", _)) => Some((
            "org.user.write",
            vec![format!("instance:{}", ctx.instance_id())],
        )),
        _ => None,
    }
}

fn alias(permission_key: &str) -> Option<PermissionAlias> {
    let alias = match permission_key {
        "instance.read" => &["iam.read", "system.instance.read", "support.read"][..],
        "instance.write" => &["iam.write", "system.instance.write", "support.write"][..],
        "org.read" => &["org.read", "support.read"][..],
        "org.write" => &["org.write", "support.config"][..],
        "org.user.read" => &["user.read", "user.global.read", "support.read"][..],
        "org.user.write" => &["user.write", "support.write"][..],
        "group.read" => &["group.read", "support.read"][..],
        "group.write" => &["group.write", "group.delete", "support.write"][..],
        "session.read" => &["session.read", "support.read"][..],
        "session.write" => &["session.write", "session.delete", "support.write"][..],
        "provider.read" => &["iam.idp.read", "org.idp.read", "support.read"][..],
        "provider.write" => &[
            "iam.idp.write",
            "iam.idp.delete",
            "org.idp.write",
            "org.idp.delete",
            "support.config",
        ][..],
        "login_flow.read" => &["iam.flow.read", "org.flow.read", "support.read"][..],
        "login_flow.write" => &[
            "iam.flow.write",
            "iam.flow.delete",
            "org.flow.write",
            "org.flow.delete",
            "support.config",
        ][..],
        "settings.read" => &[
            "policy.read",
            "iam.feature.read",
            "org.feature.read",
            "iam.restrictions.read",
            "support.read",
        ][..],
        "settings.write" => &[
            "policy.write",
            "policy.delete",
            "iam.feature.write",
            "iam.feature.delete",
            "org.feature.write",
            "org.feature.delete",
            "iam.restrictions.write",
            "support.config",
        ][..],
        "schema.read" => &["userschema.read", "support.read"][..],
        "schema.write" => &["userschema.write", "userschema.delete", "support.config"][..],
        "support.grant.read" => &["support.grant.read", "iam.read", "system.instance.read"][..],
        "support.grant.write" => &[
            "support.grant.write",
            "support.grant.delete",
            "support.admin",
            "iam.write",
            "system.instance.write",
        ][..],
        _ => return None,
    };
    Some(PermissionAlias { permissions: alias })
}
