use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{Repositories, SettingsRecord};
use std::sync::Arc;

pub struct GetSettings {
    repos: Arc<Repositories>,
}

impl GetSettings {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_settings", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        settings_type: &str,
        org_id: Option<&str>,
        app_id: Option<&str>,
    ) -> Result<SettingsRecord, AppError> {
        self.repos
            .settings
            .resolve(ctx.instance_id(), settings_type, org_id, app_id)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct UpdateSettings {
    repos: Arc<Repositories>,
}

pub struct UpdateSettingsCommand {
    pub settings_type: String,
    pub scope: String,
    pub data: serde_json::Value,
}

pub struct DeleteSettings {
    repos: Arc<Repositories>,
}

impl DeleteSettings {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.delete_settings",
        skip_all,
        fields(event_type = "settings.deleted", category = "settings")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        settings_type: &str,
    ) -> Result<(), AppError> {
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("instance:{}", ctx.instance_id()),
        )
        .await?;

        self.repos
            .settings
            .delete(ctx.instance_id(), settings_type)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::SettingsDeleted {
                    settings_type: settings_type.to_string(),
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

impl UpdateSettings {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.update_settings",
        skip_all,
        fields(event_type = "settings.updated", category = "settings")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpdateSettingsCommand,
    ) -> Result<(), AppError> {
        // Authz: caller must be admin on the current instance
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("instance:{}", ctx.instance_id())).await?;

        let record = SettingsRecord {
            settings_type: cmd.settings_type.clone(),
            scope: cmd.scope.clone(),
            data: cmd.data,
        };

        self.repos
            .settings
            .set(ctx.instance_id(), &record)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::SettingsUpdated {
                    scope: cmd.scope,
                    settings_type: cmd.settings_type,
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
