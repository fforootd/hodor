use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{AppRecord, ListParams, ListResult, Repositories};
use std::sync::Arc;

pub struct CreateApp {
    repos: Arc<Repositories>,
}

pub struct CreateAppCommand {
    pub name: String,
    pub group_id: String,
    pub protocol: String,
    pub metadata: serde_json::Value,
}

impl CreateApp {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.create_app",
        skip_all,
        fields(event_type = "app.created", category = "app")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateAppCommand,
    ) -> Result<AppRecord, AppError> {
        if cmd.name.is_empty() {
            return Err(AppError::validation("name is required"));
        }

        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        let id = uuid::Uuid::now_v7().to_string();

        let record = AppRecord {
            id: id.clone(),
            group_id: cmd.group_id.clone(),
            name: cmd.name,
            protocol: cmd.protocol.clone(),
            state: "active".to_string(),
            metadata: cmd.metadata,
            created_at: String::new(),
            updated_at: String::new(),
        };

        let created = self
            .repos
            .apps
            .create(ctx.instance_id(), &record)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::AppCreated {
                    app_id: id.clone(),
                    group_id: cmd.group_id,
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

pub struct GetApp {
    repos: Arc<Repositories>,
}

impl GetApp {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_app", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext, app_id: &str) -> Result<AppRecord, AppError> {
        self.repos
            .apps
            .get(ctx.instance_id(), app_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("app", app_id))
    }
}

pub struct ListApps {
    repos: Arc<Repositories>,
}

impl ListApps {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_apps", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        _group_id: &str,
        params: &ListParams,
    ) -> Result<ListResult<AppRecord>, AppError> {
        self.repos
            .apps
            .list(ctx.instance_id(), Some(_group_id), params)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct UpdateApp {
    repos: Arc<Repositories>,
}

impl UpdateApp {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.update_app", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        app_id: &str,
        name: &str,
    ) -> Result<bool, AppError> {
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        let updated = self
            .repos
            .apps
            .update_name(ctx.instance_id(), app_id, name)
            .await
            .map_err(AppError::Internal)?;

        if updated {
            self.repos
                .events
                .append(
                    ctx.instance_id(),
                    &DomainEvent::AppUpdated {
                        app_id: app_id.to_string(),
                        fields_changed: vec!["name".to_string()],
                        actor_id: ctx.user_id().to_string(),
                    },
                    None,
                    None,
                    None,
                )
                .await
                .map_err(AppError::Internal)?;
        }

        Ok(updated)
    }
}
