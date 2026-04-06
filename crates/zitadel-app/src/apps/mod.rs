use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{AppRecord, ListParams, ListResult, Repositories};
use std::sync::Arc;

// Note: Apps are currently backed by the generic named resource table via
// RawQueryRepository. App-specific fields (protocol, redirect_uris, etc.)
// will migrate to a dedicated AppRepository when OIDC client validation
// is added.

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
            &format!("instance:{}", ctx.instance_id()),
        )
        .await?;

        let id = uuid::Uuid::now_v7().to_string();

        // Persist via RawQueryRepository (apps table)
        let resource = self
            .repos
            .raw
            .create_named_resource(ctx.instance_id(), "apps", &id, &cmd.name, &cmd.group_id)
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

        Ok(AppRecord {
            id: resource.id,
            group_id: String::new(),
            name: resource.name,
            protocol: String::new(),
            state: resource.state,
            metadata: cmd.metadata,
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        })
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
        let resource = self
            .repos
            .raw
            .get_named_resource(ctx.instance_id(), "apps", app_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("app", app_id))?;

        Ok(AppRecord {
            id: resource.id,
            group_id: String::new(),
            name: resource.name,
            protocol: String::new(),
            state: resource.state,
            metadata: serde_json::Value::Object(Default::default()),
            created_at: resource.created_at,
            updated_at: resource.updated_at,
        })
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
        let limit = params.limit.unwrap_or(50) as i64;
        let cursor = params.cursor.clone().unwrap_or_default();
        let resources = self
            .repos
            .raw
            .list_named_resources(ctx.instance_id(), "apps", &cursor, limit + 1)
            .await
            .map_err(AppError::Internal)?;

        let has_more = resources.len() as i64 > limit;
        let items: Vec<AppRecord> = resources
            .into_iter()
            .take(limit as usize)
            .map(|r| AppRecord {
                id: r.id,
                group_id: String::new(),
                name: r.name,
                protocol: String::new(),
                state: r.state,
                metadata: serde_json::Value::Object(Default::default()),
                created_at: r.created_at.clone(),
                updated_at: r.updated_at,
            })
            .collect();

        let next_cursor = if has_more {
            items.last().map(|a| a.id.clone())
        } else {
            None
        };

        Ok(ListResult {
            items,
            next_cursor,
            total_count: None,
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
        ctx: &ActorContext,
        app_id: &str,
        name: &str,
    ) -> Result<bool, AppError> {
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("instance:{}", ctx.instance_id()),
        )
        .await?;

        let updated = self
            .repos
            .raw
            .update_named_resource_name(ctx.instance_id(), "apps", app_id, name)
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
