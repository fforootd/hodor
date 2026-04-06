use crate::context::ActorContext;
use crate::error::AppError;
use crate::repo::{FingerprintRecord, Repositories};
use std::sync::Arc;

pub struct ListFingerprints {
    repos: Arc<Repositories>,
}

impl ListFingerprints {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_fingerprints", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cursor: &str,
        limit: i64,
    ) -> Result<Vec<FingerprintRecord>, AppError> {
        self.repos
            .raw
            .list_fingerprints(ctx.instance_id(), cursor, limit)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct UpsertFingerprint {
    repos: Arc<Repositories>,
}

pub struct UpsertFingerprintCommand {
    pub id: String,
    pub type_: String,
    pub raw_data: String,
}

impl UpsertFingerprint {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.upsert_fingerprint", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpsertFingerprintCommand,
    ) -> Result<(), AppError> {
        self.repos
            .raw
            .upsert_fingerprint(ctx.instance_id(), &cmd.id, &cmd.type_, &cmd.raw_data)
            .await
            .map_err(AppError::Internal)
    }
}
