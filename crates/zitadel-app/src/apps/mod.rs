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

        // Verify group exists
        self.repos
            .groups
            .get(ctx.instance_id(), &cmd.group_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("group", &cmd.group_id))?;

        let id = uuid::Uuid::now_v7().to_string();
        let now = crate::users::chrono_now();

        let record = AppRecord {
            id: id.clone(),
            group_id: cmd.group_id.clone(),
            name: cmd.name,
            protocol: cmd.protocol.clone(),
            state: "active".to_string(),
            metadata: cmd.metadata,
            created_at: now.clone(),
            updated_at: now,
        };

        // Apps use the schema registry (ADR-005). For now, persist directly.
        // The group repo handles app storage since apps belong to groups.
        // TODO: This needs a dedicated app table or apps-as-entities in the schema model.
        let _ = &record;

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

        Ok(record)
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
    pub async fn execute(&self, _ctx: &ActorContext, _app_id: &str) -> Result<AppRecord, AppError> {
        // TODO: Implement once app storage is defined
        Err(AppError::not_found("app", _app_id))
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
        _ctx: &ActorContext,
        _group_id: &str,
        _params: &ListParams,
    ) -> Result<ListResult<AppRecord>, AppError> {
        // TODO: Implement once app storage is defined
        Ok(ListResult {
            items: Vec::new(),
            next_cursor: None,
            total_count: Some(0),
        })
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
        _ctx: &ActorContext,
        _app_id: &str,
        _updates: serde_json::Value,
    ) -> Result<AppRecord, AppError> {
        // TODO: Implement once app storage is defined
        Err(AppError::not_found("app", _app_id))
    }
}
