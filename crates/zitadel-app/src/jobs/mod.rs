use crate::context::ActorContext;
use crate::error::AppError;
use crate::repo::{JobRecord, Repositories};
use std::sync::Arc;

pub struct ListJobs {
    repos: Arc<Repositories>,
}

impl ListJobs {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_jobs", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext) -> Result<Vec<JobRecord>, AppError> {
        self.repos
            .jobs
            .list_jobs(ctx.instance_id())
            .await
            .map_err(AppError::Internal)
    }
}
