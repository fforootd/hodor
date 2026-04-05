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
        self.repos
            .groups
            .get(ctx.instance_id(), group_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("group", group_id))
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
        self.repos
            .groups
            .list(ctx.instance_id(), org_id, params)
            .await
            .map_err(AppError::Internal)
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
