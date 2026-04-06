use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{LoginFlowRecord, Repositories};
use std::sync::Arc;

pub struct CreateLoginFlow {
    repos: Arc<Repositories>,
}

pub struct CreateLoginFlowCommand {
    pub flow_id: Option<String>,
    pub name: String,
    pub steps: serde_json::Value,
    pub branding: serde_json::Value,
}

impl CreateLoginFlow {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.create_login_flow",
        skip_all,
        fields(event_type = "login_flow.configured", category = "login_flow")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateLoginFlowCommand,
    ) -> Result<LoginFlowRecord, AppError> {
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        let flow_id = cmd
            .flow_id
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
        let now = crate::users::chrono_now();

        let record = LoginFlowRecord {
            id: flow_id.clone(),
            name: cmd.name,
            strategy: "default".to_string(),
            state: "draft".to_string(),
            is_default: false,
            enabled: false,
            priority: 0,
            config: cmd.steps,
            audience: cmd.branding,
            auth_methods: serde_json::Value::Array(vec![]),
            created_at: now.clone(),
            updated_at: now,
        };

        self.repos
            .login_flows
            .upsert_flow(ctx.instance_id(), &record)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::LoginFlowConfigured {
                    flow_id,
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

pub struct GetLoginFlow {
    repos: Arc<Repositories>,
}

impl GetLoginFlow {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_login_flow", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        flow_id: &str,
    ) -> Result<LoginFlowRecord, AppError> {
        // Authz: caller must be viewer on their own org
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        self.repos
            .login_flows
            .get_flow(ctx.instance_id(), flow_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("login_flow", flow_id))
    }
}

pub struct ListLoginFlows {
    repos: Arc<Repositories>,
}

impl ListLoginFlows {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_login_flows", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext) -> Result<Vec<LoginFlowRecord>, AppError> {
        // Authz: caller must be viewer on their own org
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        self.repos
            .login_flows
            .list_flows(ctx.instance_id())
            .await
            .map_err(AppError::Internal)
    }
}

pub struct UpdateLoginFlow {
    repos: Arc<Repositories>,
}

pub struct UpdateLoginFlowCommand {
    pub flow_id: Option<String>,
    pub name: String,
    pub steps: serde_json::Value,
    pub branding: serde_json::Value,
}

impl UpdateLoginFlow {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.update_login_flow",
        skip_all,
        fields(event_type = "login_flow.configured", category = "login_flow")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpdateLoginFlowCommand,
    ) -> Result<(), AppError> {
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        let flow_id = cmd
            .flow_id
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());
        let now = crate::users::chrono_now();

        let record = LoginFlowRecord {
            id: flow_id.clone(),
            name: cmd.name,
            strategy: "default".to_string(),
            state: "draft".to_string(),
            is_default: false,
            enabled: false,
            priority: 0,
            config: cmd.steps,
            audience: cmd.branding,
            auth_methods: serde_json::Value::Array(vec![]),
            created_at: now.clone(),
            updated_at: now,
        };

        self.repos
            .login_flows
            .upsert_flow(ctx.instance_id(), &record)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::LoginFlowConfigured {
                    flow_id,
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

pub struct DeleteLoginFlow {
    repos: Arc<Repositories>,
}

impl DeleteLoginFlow {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.delete_login_flow",
        skip_all,
        fields(event_type = "login_flow.configured", category = "login_flow")
    )]
    pub async fn execute(&self, ctx: &ActorContext, flow_id: &str) -> Result<(), AppError> {
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        self.repos
            .login_flows
            .delete_flow(ctx.instance_id(), flow_id)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct PromoteLoginFlow {
    repos: Arc<Repositories>,
}

impl PromoteLoginFlow {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.promote_login_flow",
        skip_all,
        fields(event_type = "login_flow.configured", category = "login_flow")
    )]
    pub async fn execute(&self, ctx: &ActorContext, flow_id: &str) -> Result<bool, AppError> {
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        self.repos
            .login_flows
            .set_state(ctx.instance_id(), flow_id, "active", true)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct ArchiveLoginFlow {
    repos: Arc<Repositories>,
}

impl ArchiveLoginFlow {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.archive_login_flow",
        skip_all,
        fields(event_type = "login_flow.configured", category = "login_flow")
    )]
    pub async fn execute(&self, ctx: &ActorContext, flow_id: &str) -> Result<bool, AppError> {
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("org:{}", ctx.org_id()),
        )
        .await?;

        self.repos
            .login_flows
            .set_state(ctx.instance_id(), flow_id, "archived", false)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct ResolveLoginFlow {
    repos: Arc<Repositories>,
}

impl ResolveLoginFlow {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.resolve_login_flow", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext) -> Result<Option<LoginFlowRecord>, AppError> {
        let flows = self
            .repos
            .login_flows
            .list_flows(ctx.instance_id())
            .await
            .map_err(AppError::Internal)?;

        Ok(flows
            .into_iter()
            .find(|f| f.is_default && f.state == "active"))
    }
}
