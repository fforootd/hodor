use crate::context::ActorContext;
use crate::error::AppError;
use crate::repo::{Repositories, SearchResult};
use std::sync::Arc;

pub struct SearchEntities {
    repos: Arc<Repositories>,
}

pub struct SearchEntitiesCommand {
    pub query: String,
    pub limit: Option<u32>,
}

impl SearchEntities {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.search_entities", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: SearchEntitiesCommand,
    ) -> Result<Vec<SearchResult>, AppError> {
        if cmd.query.is_empty() {
            return Err(AppError::validation("query is required"));
        }
        self.repos
            .search
            .search(ctx.instance_id(), &cmd.query, None, cmd.limit)
            .await
            .map_err(AppError::Internal)
    }
}
