use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{InstanceRecord, ListParams, ListResult, Repositories};
use std::sync::Arc;

pub struct CreateInstance {
    repos: Arc<Repositories>,
}

pub struct CreateInstanceCommand {
    pub kind: String,
    pub placement_mode: String,
    pub region_key: Option<String>,
    pub owner_org_id: String,
    pub feature_overrides: serde_json::Value,
    pub primary_domain: Option<String>,
}

impl CreateInstance {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.create_instance",
        skip_all,
        fields(event_type = "instance.created", category = "instance")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateInstanceCommand,
    ) -> Result<InstanceRecord, AppError> {
        // Verify caller has permission (operator_admin or parent instance owner)
        if !ctx.is_operator_admin() {
            // Check if the owner_org_id belongs to the caller in the current instance
            let org = self
                .repos
                .orgs
                .get(ctx.instance_id(), &cmd.owner_org_id)
                .await
                .map_err(AppError::Internal)?;
            if org.is_none() {
                return Err(AppError::PermissionDenied {
                    reason: "owner_org_id not found in current instance".to_string(),
                });
            }
        }

        let instance_id = uuid::Uuid::now_v7().to_string();
        let now = crate::users::chrono_now();

        let record = InstanceRecord {
            instance_id: instance_id.clone(),
            state: "created".to_string(),
            kind: cmd.kind,
            placement_mode: cmd.placement_mode,
            region_key: cmd.region_key,
            owner_org_id: cmd.owner_org_id,
            feature_overrides: cmd.feature_overrides,
            primary_domain: cmd.primary_domain,
            created_at: now.clone(),
            updated_at: now,
        };

        let created = self
            .repos
            .instances
            .create(ctx.instance_id(), &record)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::InstanceCreated {
                    instance_id: instance_id.clone(),
                    parent_instance_id: Some(ctx.instance_id().to_string()),
                    owner_org_id: created.owner_org_id.clone(),
                    kind: created.kind.clone(),
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

pub struct GetInstance {
    repos: Arc<Repositories>,
}

impl GetInstance {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_instance", skip_all)]
    pub async fn execute(
        &self,
        _ctx: &ActorContext,
        instance_id: &str,
    ) -> Result<InstanceRecord, AppError> {
        self.repos
            .instances
            .get(instance_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("instance", instance_id))
    }
}

pub struct ListInstances {
    repos: Arc<Repositories>,
}

impl ListInstances {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_instances", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        params: &ListParams,
    ) -> Result<ListResult<InstanceRecord>, AppError> {
        self.repos
            .instances
            .list(ctx.instance_id(), params)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct UpdateInstance {
    repos: Arc<Repositories>,
}

pub struct UpdateInstanceCommand {
    pub instance_id: String,
    pub placement_mode: Option<String>,
    pub region_key: Option<String>,
    pub feature_overrides: Option<serde_json::Value>,
}

impl UpdateInstance {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.update_instance",
        skip_all,
        fields(event_type = "instance.updated", category = "instance")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpdateInstanceCommand,
    ) -> Result<InstanceRecord, AppError> {
        let mut instance = self
            .repos
            .instances
            .get(&cmd.instance_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("instance", &cmd.instance_id))?;

        let mut fields_changed = Vec::new();
        if let Some(pm) = cmd.placement_mode {
            instance.placement_mode = pm;
            fields_changed.push("placement_mode".to_string());
        }
        if let Some(rk) = cmd.region_key {
            instance.region_key = Some(rk);
            fields_changed.push("region_key".to_string());
        }
        if let Some(fo) = cmd.feature_overrides {
            instance.feature_overrides = fo;
            fields_changed.push("feature_overrides".to_string());
        }

        if fields_changed.is_empty() {
            return Ok(instance);
        }

        instance.updated_at = crate::users::chrono_now();

        let updated = self
            .repos
            .instances
            .update(&instance)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::InstanceUpdated {
                    instance_id: cmd.instance_id,
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

pub struct DeprovisionInstance {
    repos: Arc<Repositories>,
}

impl DeprovisionInstance {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.deprovision_instance",
        skip_all,
        fields(event_type = "instance.deprovisioned", category = "instance")
    )]
    pub async fn execute(&self, ctx: &ActorContext, instance_id: &str) -> Result<(), AppError> {
        let instance = self
            .repos
            .instances
            .get(instance_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("instance", instance_id))?;

        if instance.state == "deprovisioned" {
            return Err(AppError::InvalidState {
                entity: "instance".to_string(),
                id: instance_id.to_string(),
                current_state: "deprovisioned".to_string(),
                expected_state: "active or created".to_string(),
            });
        }

        self.repos
            .instances
            .deprovision(instance_id)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::InstanceDeprovisioned {
                    instance_id: instance_id.to_string(),
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
