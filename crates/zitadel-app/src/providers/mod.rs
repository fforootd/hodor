use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{ListParams, ListResult, ProviderRecord, Repositories};
use std::sync::Arc;

pub struct CreateProvider {
    repos: Arc<Repositories>,
}

pub struct CreateProviderCommand {
    pub name: String,
    pub protocol: String,
    pub config: serde_json::Value,
}

impl CreateProvider {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.create_provider",
        skip_all,
        fields(event_type = "provider.configured", category = "provider")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateProviderCommand,
    ) -> Result<ProviderRecord, AppError> {
        if cmd.name.is_empty() {
            return Err(AppError::validation("name is required"));
        }

        // Authz: caller must be admin on the current instance
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("instance:{}", ctx.instance_id())).await?;

        let id = uuid::Uuid::now_v7().to_string();
        let now = crate::users::chrono_now();

        let record = ProviderRecord {
            id: id.clone(),
            name: cmd.name,
            protocol: cmd.protocol.clone(),
            state: "active".to_string(),
            config: cmd.config,
            created_at: now.clone(),
            updated_at: now,
        };

        let created = self
            .repos
            .providers
            .create(ctx.instance_id(), &record)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::ProviderConfigured {
                    provider_id: id,
                    protocol: cmd.protocol,
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(created)
    }
}

pub struct GetProvider {
    repos: Arc<Repositories>,
}

impl GetProvider {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_provider", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        provider_id: &str,
    ) -> Result<ProviderRecord, AppError> {
        self.repos
            .providers
            .get(ctx.instance_id(), provider_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("provider", provider_id))
    }
}

pub struct ListProviders {
    repos: Arc<Repositories>,
}

impl ListProviders {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_providers", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        params: &ListParams,
    ) -> Result<ListResult<ProviderRecord>, AppError> {
        self.repos
            .providers
            .list(ctx.instance_id(), params)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct UpdateProvider {
    repos: Arc<Repositories>,
}

pub struct UpdateProviderCommand {
    pub provider_id: String,
    pub name: Option<String>,
    pub config: Option<serde_json::Value>,
}

impl UpdateProvider {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.update_provider",
        skip_all,
        fields(event_type = "provider.configured", category = "provider")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpdateProviderCommand,
    ) -> Result<ProviderRecord, AppError> {
        // Authz: caller must be admin on the current instance
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("instance:{}", ctx.instance_id())).await?;

        let mut provider = self
            .repos
            .providers
            .get(ctx.instance_id(), &cmd.provider_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("provider", &cmd.provider_id))?;

        if let Some(name) = cmd.name {
            provider.name = name;
        }
        if let Some(config) = cmd.config {
            provider.config = config;
        }

        provider.updated_at = crate::users::chrono_now();

        let updated = self
            .repos
            .providers
            .update(ctx.instance_id(), &provider)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::ProviderConfigured {
                    provider_id: cmd.provider_id,
                    protocol: updated.protocol.clone(),
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                None,
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(updated)
    }
}

pub struct DeleteProvider {
    repos: Arc<Repositories>,
}

impl DeleteProvider {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.delete_provider",
        skip_all,
        fields(event_type = "provider.removed", category = "provider")
    )]
    pub async fn execute(&self, ctx: &ActorContext, provider_id: &str) -> Result<(), AppError> {
        // Authz: caller must be admin on the current instance
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("instance:{}", ctx.instance_id())).await?;

        self.repos
            .providers
            .delete(ctx.instance_id(), provider_id)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::ProviderRemoved {
                    provider_id: provider_id.to_string(),
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
