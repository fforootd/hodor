use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{GroupRecord, ListParams, ListResult, Repositories};
use std::sync::Arc;

pub struct CreateGroup {
    repos: Arc<Repositories>,
}

pub struct CreateGroupCommand {
    pub name: String,
    pub org_id: String,
    pub metadata: serde_json::Value,
}

impl CreateGroup {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.create_group",
        skip_all,
        fields(event_type = "group.created", category = "group")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateGroupCommand,
    ) -> Result<GroupRecord, AppError> {
        if cmd.name.is_empty() {
            return Err(AppError::validation("name is required"));
        }

        // Authz: caller must be admin on the target org
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("org:{}", cmd.org_id))
            .await?;

        let id = uuid::Uuid::now_v7().to_string();
        let now = crate::users::chrono_now();

        let record = GroupRecord {
            id: id.clone(),
            org_id: cmd.org_id.clone(),
            name: cmd.name.clone(),
            state: "active".to_string(),
            metadata: cmd.metadata,
            created_at: now.clone(),
            updated_at: now,
        };

        let created = self
            .repos
            .groups
            .create(ctx.instance_id(), &record)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::GroupCreated {
                    group_id: id,
                    org_id: cmd.org_id,
                    name: cmd.name,
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

pub struct GetGroup {
    repos: Arc<Repositories>,
}

impl GetGroup {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_group", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        group_id: &str,
    ) -> Result<GroupRecord, AppError> {
        let group = self
            .repos
            .groups
            .get(ctx.instance_id(), group_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("group", group_id))?;

        // Authz: caller must be viewer on the group's org
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", group.org_id),
        )
        .await?;

        Ok(group)
    }
}

pub struct ListGroups {
    repos: Arc<Repositories>,
}

impl ListGroups {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_groups", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        org_id: Option<&str>,
        params: &ListParams,
    ) -> Result<ListResult<GroupRecord>, AppError> {
        // Authz: caller must be viewer on the target org (or own org if unfiltered)
        let effective_org = org_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| ctx.org_id().to_string());
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", effective_org),
        )
        .await?;

        self.repos
            .groups
            .list(ctx.instance_id(), org_id, params)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct DeleteGroup {
    repos: Arc<Repositories>,
}

impl DeleteGroup {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.delete_group",
        skip_all,
        fields(event_type = "group.deleted", category = "group")
    )]
    pub async fn execute(&self, ctx: &ActorContext, group_id: &str) -> Result<(), AppError> {
        // Verify exists (instance-scoped: returns 404 for cross-instance)
        let group = self
            .repos
            .groups
            .get(ctx.instance_id(), group_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("group", group_id))?;

        crate::authz::require_scoped_permission(
            &self.repos,
            ctx,
            "group.write",
            &[
                format!("org:{}", group.org_id),
                format!("instance:{}", ctx.instance_id()),
            ],
        )
        .await?;

        self.repos
            .groups
            .delete(ctx.instance_id(), group_id)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::GroupDeleted {
                    group_id: group_id.to_string(),
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(())
    }
}

pub struct UpdateGroup {
    repos: Arc<Repositories>,
}

pub struct UpdateGroupCommand {
    pub group_id: String,
    pub name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl UpdateGroup {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.update_group",
        skip_all,
        fields(event_type = "group.updated", category = "group")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpdateGroupCommand,
    ) -> Result<GroupRecord, AppError> {
        let mut group = self
            .repos
            .groups
            .get(ctx.instance_id(), &cmd.group_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("group", &cmd.group_id))?;

        crate::authz::require_scoped_permission(
            &self.repos,
            ctx,
            "group.write",
            &[
                format!("org:{}", group.org_id),
                format!("instance:{}", ctx.instance_id()),
            ],
        )
        .await?;

        let mut fields_changed = Vec::new();
        if let Some(name) = cmd.name {
            group.name = name;
            fields_changed.push("name".to_string());
        }
        if let Some(meta) = cmd.metadata {
            group.metadata = meta;
            fields_changed.push("metadata".to_string());
        }

        if fields_changed.is_empty() {
            return Ok(group);
        }

        group.updated_at = crate::users::chrono_now();

        let updated = self
            .repos
            .groups
            .update(ctx.instance_id(), &group)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::GroupUpdated {
                    group_id: cmd.group_id,
                    fields_changed,
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(updated)
    }
}
