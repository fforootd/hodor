use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{ListParams, ListResult, OrgRecord, Repositories};
use std::sync::Arc;

pub struct CreateOrg {
    repos: Arc<Repositories>,
}

pub struct CreateOrgCommand {
    pub name: String,
    pub metadata: serde_json::Value,
}

impl CreateOrg {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.create_org",
        skip_all,
        fields(event_type = "org.created", category = "org")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateOrgCommand,
    ) -> Result<OrgRecord, AppError> {
        if cmd.name.is_empty() {
            return Err(AppError::validation("name is required"));
        }

        let id = uuid::Uuid::now_v7().to_string();
        let now = crate::users::chrono_now();

        let record = OrgRecord {
            id: id.clone(),
            name: cmd.name.clone(),
            state: "active".to_string(),
            metadata: cmd.metadata,
            created_at: now.clone(),
            updated_at: now,
        };

        let created = self
            .repos
            .orgs
            .create(ctx.instance_id(), &record)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::OrgCreated {
                    org_id: id,
                    name: cmd.name,
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

pub struct GetOrg {
    repos: Arc<Repositories>,
}

impl GetOrg {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_org", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext, org_id: &str) -> Result<OrgRecord, AppError> {
        self.repos
            .orgs
            .get(ctx.instance_id(), org_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("org", org_id))
    }
}

pub struct ListOrgs {
    repos: Arc<Repositories>,
}

impl ListOrgs {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_orgs", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        params: &ListParams,
    ) -> Result<ListResult<OrgRecord>, AppError> {
        self.repos
            .orgs
            .list(ctx.instance_id(), params)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct UpdateOrg {
    repos: Arc<Repositories>,
}

pub struct UpdateOrgCommand {
    pub org_id: String,
    pub name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl UpdateOrg {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.update_org",
        skip_all,
        fields(event_type = "org.updated", category = "org")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpdateOrgCommand,
    ) -> Result<OrgRecord, AppError> {
        let mut org = self
            .repos
            .orgs
            .get(ctx.instance_id(), &cmd.org_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("org", &cmd.org_id))?;

        let mut fields_changed = Vec::new();
        if let Some(name) = cmd.name {
            org.name = name;
            fields_changed.push("name".to_string());
        }
        if let Some(meta) = cmd.metadata {
            org.metadata = meta;
            fields_changed.push("metadata".to_string());
        }

        if fields_changed.is_empty() {
            return Ok(org);
        }

        org.updated_at = crate::users::chrono_now();

        let updated = self
            .repos
            .orgs
            .update(ctx.instance_id(), &org)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::OrgUpdated {
                    org_id: cmd.org_id,
                    fields_changed,
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
