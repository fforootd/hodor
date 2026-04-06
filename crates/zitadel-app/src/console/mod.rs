use crate::context::ActorContext;
use crate::error::AppError;
use crate::repo::{ConsoleBootstrapData, Repositories};
use std::sync::Arc;

pub struct LoadConsoleBootstrap {
    repos: Arc<Repositories>,
}

impl LoadConsoleBootstrap {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.load_console_bootstrap", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext) -> Result<ConsoleBootstrapData, AppError> {
        self.repos
            .console_queries
            .load_console_bootstrap(ctx.instance_id())
            .await
            .map_err(AppError::Internal)
    }
}

pub struct LoadEntityCounts {
    repos: Arc<Repositories>,
}

impl LoadEntityCounts {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.load_entity_counts", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext) -> Result<Vec<(String, i64)>, AppError> {
        self.repos
            .console_queries
            .load_entity_counts(ctx.instance_id())
            .await
            .map_err(AppError::Internal)
    }
}
