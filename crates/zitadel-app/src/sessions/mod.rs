use crate::context::ActorContext;
use crate::error::AppError;
use crate::repo::Repositories;
use std::sync::Arc;

// Note: IssueSession and RevokeSession already exist in auth/mod.rs.
// This module adds ListSessions for the sessions API endpoint.

pub struct ListSessions {
    repos: Arc<Repositories>,
}

impl ListSessions {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_sessions", skip_all)]
    pub async fn execute(
        &self,
        _ctx: &ActorContext,
    ) -> Result<Vec<crate::repo::SessionInfo>, AppError> {
        // SessionRepository doesn't expose a list method (sessions are in KvStore).
        // The list endpoint queries transient storage directly.
        // This is a no-op placeholder — the handler should continue using transient storage.
        Ok(vec![])
    }
}
