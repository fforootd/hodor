use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{NamedResourceRecord, Repositories};
use std::sync::Arc;

pub struct CreateNamedResource {
    repos: Arc<Repositories>,
}

pub struct CreateNamedResourceCommand {
    pub kind: String,
    pub name: String,
    pub org_id: String,
}

impl CreateNamedResource {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.create_named_resource", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateNamedResourceCommand,
    ) -> Result<NamedResourceRecord, AppError> {
        let id = uuid::Uuid::now_v7().to_string();

        let result = self
            .repos
            .raw
            .create_named_resource(
                ctx.instance_id(),
                &cmd.kind,
                &id,
                &cmd.name,
                &cmd.org_id,
            )
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::ResourceCreated {
                    resource_id: result.id.clone(),
                    kind: cmd.kind,
                    name: cmd.name,
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(result)
    }
}

pub struct GetNamedResource {
    repos: Arc<Repositories>,
}

impl GetNamedResource {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_named_resource", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        kind: &str,
        id: &str,
    ) -> Result<NamedResourceRecord, AppError> {
        self.repos
            .raw
            .get_named_resource(ctx.instance_id(), kind, id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found(kind, id))
    }
}

pub struct ListNamedResources {
    repos: Arc<Repositories>,
}

impl ListNamedResources {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_named_resources", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        kind: &str,
        cursor: &str,
        limit: i64,
    ) -> Result<Vec<NamedResourceRecord>, AppError> {
        self.repos
            .raw
            .list_named_resources(ctx.instance_id(), kind, cursor, limit)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct UpdateNamedResource {
    repos: Arc<Repositories>,
}

pub struct UpdateNamedResourceCommand {
    pub kind: String,
    pub id: String,
    pub name: String,
}

impl UpdateNamedResource {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.update_named_resource", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpdateNamedResourceCommand,
    ) -> Result<bool, AppError> {
        let updated = self
            .repos
            .raw
            .update_named_resource_name(
                ctx.instance_id(),
                &cmd.kind,
                &cmd.id,
                &cmd.name,
            )
            .await
            .map_err(AppError::Internal)?;

        if updated {
            self.repos
                .events
                .append(
                    ctx.instance_id(),
                    &DomainEvent::ResourceUpdated {
                        resource_id: cmd.id,
                        kind: cmd.kind,
                        fields_changed: vec!["name".to_string()],
                        actor_id: ctx.user_id().to_string(),
                    },
                    None,
                    None,
                    None,
                )
                .await
                .map_err(AppError::Internal)?;
        }

        Ok(updated)
    }
}

pub struct DeleteNamedResource {
    repos: Arc<Repositories>,
}

impl DeleteNamedResource {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.delete_named_resource", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        kind: &str,
        id: &str,
    ) -> Result<bool, AppError> {
        let deleted = self
            .repos
            .raw
            .delete_named_resource(ctx.instance_id(), kind, id)
            .await
            .map_err(AppError::Internal)?;

        if deleted {
            self.repos
                .events
                .append(
                    ctx.instance_id(),
                    &DomainEvent::ResourceDeleted {
                        resource_id: id.to_string(),
                        kind: kind.to_string(),
                        actor_id: ctx.user_id().to_string(),
                    },
                    None,
                    None,
                    None,
                )
                .await
                .map_err(AppError::Internal)?;
        }

        Ok(deleted)
    }
}
