use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{ListParams, ListResult, Repositories, SchemaRecord};
use std::sync::Arc;

pub struct RegisterSchema {
    repos: Arc<Repositories>,
}

pub struct RegisterSchemaCommand {
    pub schema_type: String,
    pub schema_json: serde_json::Value,
    pub visibility: String,
}

impl RegisterSchema {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.register_schema",
        skip_all,
        fields(event_type = "schema.registered", category = "schema")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: RegisterSchemaCommand,
    ) -> Result<SchemaRecord, AppError> {
        if cmd.schema_type.is_empty() {
            return Err(AppError::validation("schema_type is required"));
        }

        // Authz: caller must be admin on the current instance
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        // Check for existing schema with same type
        let existing = self
            .repos
            .schemas
            .get_by_type(ctx.instance_id(), &cmd.schema_type)
            .await
            .map_err(AppError::Internal)?;

        if existing.is_some() {
            return Err(AppError::already_exists("schema", &cmd.schema_type));
        }

        let id = uuid::Uuid::now_v7().to_string();
        let now = crate::users::chrono_now();

        let record = SchemaRecord {
            id: id.clone(),
            schema_type: cmd.schema_type.clone(),
            schema_json: cmd.schema_json,
            version: 1,
            is_default: false,
            visibility: cmd.visibility,
            created_at: now,
        };

        let created = self
            .repos
            .schemas
            .register(ctx.instance_id(), &record)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::SchemaRegistered {
                    schema_id: id,
                    schema_type: cmd.schema_type,
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

pub struct GetSchema {
    repos: Arc<Repositories>,
}

impl GetSchema {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_schema", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        schema_id: &str,
    ) -> Result<SchemaRecord, AppError> {
        // Authz: caller must be viewer on their own org
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        self.repos
            .schemas
            .get(ctx.instance_id(), schema_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("schema", schema_id))
    }
}

pub struct UpdateSchema {
    repos: Arc<Repositories>,
}

pub struct UpdateSchemaCommand {
    pub schema_id: String,
    pub schema_json: serde_json::Value,
}

impl UpdateSchema {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.update_schema",
        skip_all,
        fields(event_type = "schema.updated", category = "schema")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpdateSchemaCommand,
    ) -> Result<SchemaRecord, AppError> {
        // Authz: caller must be admin on the current instance
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        let mut schema = self
            .repos
            .schemas
            .get(ctx.instance_id(), &cmd.schema_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("schema", &cmd.schema_id))?;

        schema.schema_json = cmd.schema_json;
        schema.version += 1;

        let updated = self
            .repos
            .schemas
            .update(ctx.instance_id(), &schema)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::SchemaUpdated {
                    schema_id: cmd.schema_id,
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

pub struct PromoteSchema {
    repos: Arc<Repositories>,
}

impl PromoteSchema {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.promote_schema",
        skip_all,
        fields(event_type = "schema.updated", category = "schema")
    )]
    pub async fn execute(&self, ctx: &ActorContext, schema_id: &str) -> Result<bool, AppError> {
        // Authz: caller must be admin on the current instance
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        let promoted = self
            .repos
            .schemas
            .promote(ctx.instance_id(), schema_id)
            .await
            .map_err(AppError::Internal)?;

        if promoted {
            self.repos
                .events
                .append(
                    ctx.instance_id(),
                    &DomainEvent::SchemaUpdated {
                        schema_id: schema_id.to_string(),
                        actor_id: ctx.user_id().to_string(),
                    },
                    None,
                    None,
                    None,
                )
                .await
                .map_err(AppError::Internal)?;
        }

        Ok(promoted)
    }
}

pub struct CountSchemaUsers {
    repos: Arc<Repositories>,
}

impl CountSchemaUsers {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.count_schema_users", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext, schema_id: &str) -> Result<i64, AppError> {
        // Authz: caller must be viewer on their own org
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        self.repos
            .schemas
            .count_by_schema(ctx.instance_id(), schema_id)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct ListSchemas {
    repos: Arc<Repositories>,
}

impl ListSchemas {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_schemas", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        params: &ListParams,
    ) -> Result<ListResult<SchemaRecord>, AppError> {
        // Authz: caller must be viewer on their own org
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        self.repos
            .schemas
            .list(ctx.instance_id(), params)
            .await
            .map_err(AppError::Internal)
    }
}
