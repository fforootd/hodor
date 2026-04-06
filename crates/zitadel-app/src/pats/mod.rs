use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{PatRecord, Repositories};
use std::sync::Arc;

pub struct CreatePat {
    repos: Arc<Repositories>,
}

pub struct CreatePatCommand {
    pub user_id: String,
    pub name: String,
    pub scopes: Vec<String>,
}

pub struct CreatePatResult {
    pub pat_id: String,
    pub token: String,
}

impl CreatePat {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.create_pat",
        skip_all,
        fields(event_type = "pat.created", category = "pat")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreatePatCommand,
    ) -> Result<CreatePatResult, AppError> {
        if cmd.name.is_empty() {
            return Err(AppError::validation("name is required"));
        }

        // Authz: caller must be admin or the target user themselves
        crate::authz::require_permission_or_self(
            &self.repos,
            ctx,
            "admin",
            &format!("user:{}", cmd.user_id),
            &cmd.user_id,
        )
        .await?;

        let pat_id = uuid::Uuid::now_v7().to_string();
        let now = crate::users::chrono_now();
        let record = PatRecord {
            id: pat_id.clone(),
            user_id: cmd.user_id.clone(),
            name: cmd.name,
            created_at: now,
        };

        let raw_token = uuid::Uuid::now_v7().to_string();
        let token = self
            .repos
            .pats
            .create(ctx.instance_id(), &record, &raw_token)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::PatCreated {
                    pat_id: pat_id.clone(),
                    user_id: cmd.user_id,
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(CreatePatResult { pat_id, token })
    }
}

pub struct ListPats {
    repos: Arc<Repositories>,
}

impl ListPats {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_pats", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        user_id: &str,
    ) -> Result<Vec<PatRecord>, AppError> {
        // Authz: caller must be admin or the target user themselves
        crate::authz::require_permission_or_self(
            &self.repos,
            ctx,
            "admin",
            &format!("user:{}", user_id),
            user_id,
        )
        .await?;

        self.repos
            .pats
            .list(ctx.instance_id(), user_id)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct RevokePat {
    repos: Arc<Repositories>,
}

impl RevokePat {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.revoke_pat",
        skip_all,
        fields(event_type = "pat.revoked", category = "pat")
    )]
    pub async fn execute(&self, ctx: &ActorContext, pat_id: &str) -> Result<(), AppError> {
        // Authz: caller must be admin on the instance
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        self.repos
            .pats
            .revoke(ctx.instance_id(), pat_id)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::PatRevoked {
                    pat_id: pat_id.to_string(),
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(())
    }
}
