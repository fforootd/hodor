use crate::context::ActorContext;
use crate::error::AppError;
use crate::repo::{Repositories, SavedQueryRecord};
use std::sync::Arc;

pub struct ListSavedQueries {
    repos: Arc<Repositories>,
}

impl ListSavedQueries {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_saved_queries", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext) -> Result<Vec<SavedQueryRecord>, AppError> {
        self.repos
            .saved_queries
            .list_saved_queries(ctx.instance_id())
            .await
            .map_err(AppError::Internal)
    }
}

pub struct CreateSavedQuery {
    repos: Arc<Repositories>,
}

pub struct CreateSavedQueryCommand {
    pub name: String,
    pub description: String,
    pub sql: String,
}

impl CreateSavedQuery {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.create_saved_query", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateSavedQueryCommand,
    ) -> Result<SavedQueryRecord, AppError> {
        if cmd.name.is_empty() || cmd.sql.is_empty() {
            return Err(AppError::validation("name and sql are required"));
        }

        let id = format!("sq_{}", uuid::Uuid::new_v4());
        self.repos
            .saved_queries
            .create_saved_query(ctx.instance_id(), &id, &cmd.name, &cmd.description, &cmd.sql)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct DeleteSavedQuery {
    repos: Arc<Repositories>,
}

impl DeleteSavedQuery {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.delete_saved_query", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        id: &str,
    ) -> Result<bool, AppError> {
        self.repos
            .saved_queries
            .delete_saved_query(ctx.instance_id(), id)
            .await
            .map_err(AppError::Internal)
    }
}
