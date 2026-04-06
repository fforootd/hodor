use std::sync::Arc;

use zitadel_authz::{builtin_role_definition, role_key_for_relation};

use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{Repositories, RoleAssignmentFilter, RoleAssignmentRecord};

pub struct CreateSupportGrant {
    repos: Arc<Repositories>,
}

pub struct CreateSupportGrantCommand {
    pub target_instance_id: String,
    pub principal_ref: Option<String>,
    pub role: String,
    pub reason: Option<String>,
    pub expires_at: Option<String>,
}

impl CreateSupportGrant {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.create_support_grant",
        skip_all,
        fields(event_type = "support.grant_created", category = "support")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateSupportGrantCommand,
    ) -> Result<RoleAssignmentRecord, AppError> {
        crate::authz::require_scoped_permission(
            &self.repos,
            ctx,
            "support.grant.write",
            &[format!("instance:{}", ctx.instance_id())],
        )
        .await?;

        let target = self
            .repos
            .instances
            .get(&cmd.target_instance_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("instance", &cmd.target_instance_id))?;

        if target.kind == "root" {
            return Err(AppError::validation(
                "support grants cannot target a root instance",
            ));
        }

        let role_key = normalize_support_role_key(&cmd.role)?;
        let source_kind = match target.kind.as_str() {
            "federated" => "support_grant_federated",
            _ => "support_grant_managed",
        };
        let principal_ref = cmd
            .principal_ref
            .unwrap_or_else(|| default_support_principal(ctx, target.kind.as_str()));
        let now = crate::users::chrono_now();
        let assignment = RoleAssignmentRecord {
            assignment_id: uuid::Uuid::now_v7().to_string(),
            enforcement_instance_id: cmd.target_instance_id.clone(),
            scope_kind: "instance".to_string(),
            scope_id: cmd.target_instance_id.clone(),
            principal_ref: principal_ref.clone(),
            role_key: role_key.to_string(),
            source_kind: source_kind.to_string(),
            origin_instance_id: Some(ctx.instance_id().to_string()),
            approved_by: Some(ctx.principal_ref().to_string()),
            reason: cmd.reason,
            expires_at: cmd.expires_at,
            revoked_at: None,
            created_at: now.clone(),
            updated_at: now,
        };

        let created = self
            .repos
            .authorization
            .create_role_assignment(&assignment)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::SupportGrantCreated {
                    grant_id: created.assignment_id.clone(),
                    target_instance_id: created.scope_id.clone(),
                    principal_ref,
                    role_key: created.role_key.clone(),
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(created)
    }
}

pub struct ListSupportGrants {
    repos: Arc<Repositories>,
}

#[derive(Clone, Debug, Default)]
pub struct ListSupportGrantFilter {
    pub target_instance_id: Option<String>,
    pub principal_ref: Option<String>,
    pub include_revoked: bool,
}

impl ListSupportGrants {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_support_grants", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        filter: &ListSupportGrantFilter,
    ) -> Result<Vec<RoleAssignmentRecord>, AppError> {
        crate::authz::require_scoped_permission(
            &self.repos,
            ctx,
            "support.grant.read",
            &[format!("instance:{}", ctx.instance_id())],
        )
        .await?;

        let mut assignments = self
            .repos
            .authorization
            .list_role_assignments(&RoleAssignmentFilter {
                enforcement_instance_id: filter.target_instance_id.clone(),
                scope_kind: Some("instance".to_string()),
                scope_id: filter.target_instance_id.clone(),
                principal_ref: filter.principal_ref.clone(),
                include_revoked: filter.include_revoked,
                ..Default::default()
            })
            .await
            .map_err(AppError::Internal)?;

        assignments.retain(is_support_grant_assignment);
        if filter.target_instance_id.is_none() && filter.principal_ref.is_none() {
            assignments.retain(|assignment| assignment.principal_ref == ctx.principal_ref());
        }
        assignments.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(assignments)
    }
}

pub struct RevokeSupportGrant {
    repos: Arc<Repositories>,
}

impl RevokeSupportGrant {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.revoke_support_grant",
        skip_all,
        fields(event_type = "support.grant_revoked", category = "support")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        assignment_id: &str,
    ) -> Result<RoleAssignmentRecord, AppError> {
        crate::authz::require_scoped_permission(
            &self.repos,
            ctx,
            "support.grant.write",
            &[format!("instance:{}", ctx.instance_id())],
        )
        .await?;

        let assignment = self
            .repos
            .authorization
            .get_role_assignment(assignment_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("support_grant", assignment_id))?;
        if !is_support_grant_assignment(&assignment) {
            return Err(AppError::validation("assignment is not a support grant"));
        }

        let revoked_at = crate::users::chrono_now();
        let changed = self
            .repos
            .authorization
            .revoke_role_assignment(assignment_id, &revoked_at)
            .await
            .map_err(AppError::Internal)?;
        if !changed {
            return Err(AppError::not_found("support_grant", assignment_id));
        }

        let revoked = self
            .repos
            .authorization
            .get_role_assignment(assignment_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("support_grant", assignment_id))?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::SupportGrantRevoked {
                    grant_id: revoked.assignment_id.clone(),
                    target_instance_id: revoked.scope_id.clone(),
                    principal_ref: revoked.principal_ref.clone(),
                    role_key: revoked.role_key.clone(),
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(revoked)
    }
}

fn normalize_support_role_key(role: &str) -> Result<String, AppError> {
    let trimmed = role.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("role is required"));
    }
    let role_key = if let Some(role_key) = role_key_for_relation(trimmed) {
        role_key.to_string()
    } else {
        trimmed.to_ascii_uppercase()
    };
    let definition = builtin_role_definition(&role_key)
        .ok_or_else(|| AppError::validation("unknown support role"))?;
    if definition.scope_kind != "instance" || !role_key.starts_with("SUPPORT_") {
        return Err(AppError::validation(
            "role must be an instance-scoped support role",
        ));
    }
    Ok(role_key)
}

fn default_support_principal(ctx: &ActorContext, target_kind: &str) -> String {
    match target_kind {
        "federated" => format!("principal:{}:{}", ctx.instance_id(), ctx.user_id()),
        _ => ctx.principal_ref().to_string(),
    }
}

pub fn is_support_grant_assignment(assignment: &RoleAssignmentRecord) -> bool {
    assignment.scope_kind == "instance"
        && assignment.role_key.starts_with("SUPPORT_")
        && assignment.source_kind.starts_with("support_grant_")
}
