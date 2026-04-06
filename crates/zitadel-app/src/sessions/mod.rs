use crate::context::ActorContext;
use crate::error::AppError;
use crate::repo::{Repositories, SessionDetail};
use std::sync::Arc;

// Note: IssueSession and RevokeSession already exist in auth/mod.rs.
// This module adds ListSessions and GetSession for the sessions API endpoint.

pub struct ListSessions {
    repos: Arc<Repositories>,
}

impl ListSessions {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_sessions", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext) -> Result<Vec<SessionDetail>, AppError> {
        // Authz: caller must be viewer on their own org
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        Ok(self
            .repos
            .sessions
            .list_by_instance(ctx.instance_id())
            .await?)
    }

    #[tracing::instrument(name = "use_case.list_own_sessions", skip_all)]
    pub async fn execute_self(&self, ctx: &ActorContext) -> Result<Vec<SessionDetail>, AppError> {
        Ok(self
            .repos
            .sessions
            .list_by_instance(ctx.instance_id())
            .await?
            .into_iter()
            .filter(|session| session.user_id == ctx.user_id())
            .collect())
    }
}

pub struct GetSession {
    repos: Arc<Repositories>,
}

impl GetSession {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_session", skip_all, fields(session_id))]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        session_id: &str,
    ) -> Result<SessionDetail, AppError> {
        // Authz: caller must be viewer on their own org
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        self.repos
            .sessions
            .get(ctx.instance_id(), session_id)
            .await?
            .ok_or_else(|| AppError::not_found("session", session_id))
    }

    #[tracing::instrument(name = "use_case.get_own_session", skip_all, fields(session_id))]
    pub async fn execute_self(
        &self,
        ctx: &ActorContext,
        session_id: &str,
    ) -> Result<SessionDetail, AppError> {
        let session = self
            .repos
            .sessions
            .get(ctx.instance_id(), session_id)
            .await?
            .ok_or_else(|| AppError::not_found("session", session_id))?;

        if session.user_id != ctx.user_id() {
            return Err(AppError::PermissionDenied {
                reason: format!(
                    "principal {} cannot access session {}",
                    ctx.principal_ref(),
                    session_id
                ),
            });
        }

        Ok(session)
    }
}
