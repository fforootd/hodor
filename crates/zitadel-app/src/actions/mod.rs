use crate::context::ActorContext;
use crate::error::AppError;
use crate::repo::{ActionRecord, ListParams, ListResult, Repositories};
use std::sync::Arc;

pub struct ListActions {
    repos: Arc<Repositories>,
}

impl ListActions {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_actions", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        params: &ListParams,
    ) -> Result<ListResult<ActionRecord>, AppError> {
        self.repos
            .actions
            .list(ctx.instance_id(), params)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct GetAction {
    repos: Arc<Repositories>,
}

impl GetAction {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_action", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        action_id: &str,
    ) -> Result<ActionRecord, AppError> {
        self.repos
            .actions
            .get(ctx.instance_id(), action_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("action", action_id))
    }
}

pub struct CreateAction {
    repos: Arc<Repositories>,
}

impl CreateAction {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.create_action", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        action: &ActionRecord,
    ) -> Result<ActionRecord, AppError> {
        self.repos
            .actions
            .create(ctx.instance_id(), action)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct UpdateAction {
    repos: Arc<Repositories>,
}

impl UpdateAction {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.update_action", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        action: &ActionRecord,
    ) -> Result<ActionRecord, AppError> {
        self.repos
            .actions
            .update(ctx.instance_id(), action)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct DeleteAction {
    repos: Arc<Repositories>,
}

impl DeleteAction {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.delete_action", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext, action_id: &str) -> Result<(), AppError> {
        // Authz: caller must be admin on the instance
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("instance:{}", ctx.instance_id()),
        )
        .await?;

        self.repos
            .actions
            .delete(ctx.instance_id(), action_id)
            .await
            .map_err(AppError::Internal)
    }
}
