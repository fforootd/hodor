use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{DomainRecord, DomainRemoveResult, InstanceRecord, ListParams, ListResult, Repositories};
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
        let owner_org_id = cmd.owner_org_id.clone();

        // Authz: operator admins can always create instances.
        // Non-operators must be admin on the owner org.
        crate::authz::require_permission(
            &self.repos, ctx, "admin", &format!("org:{}", owner_org_id),
        )
        .await?;

        let instance_id = uuid::Uuid::now_v7().to_string();
        let now = crate::users::chrono_now();

        let record = InstanceRecord {
            instance_id: instance_id.clone(),
            state: "created".to_string(),
            kind: cmd.kind,
            placement_mode: cmd.placement_mode,
            region_key: cmd.region_key,
            owner_org_id: Some(cmd.owner_org_id),
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
                    owner_org_id,
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
        // Authz: caller must be admin on the target instance
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("instance:{}", cmd.instance_id)).await?;

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
        // Authz: only operator admins can deprovision instances
        crate::authz::require_operator_admin(ctx)?;

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

// ─── Domain management ─────────────────────────────────────

pub struct ListDomains {
    repos: Arc<Repositories>,
}

impl ListDomains {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_domains", skip_all)]
    pub async fn execute(
        &self,
        _ctx: &ActorContext,
        instance_id: &str,
    ) -> Result<Vec<DomainRecord>, AppError> {
        self.repos
            .instances
            .list_domains(instance_id)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct AddDomain {
    repos: Arc<Repositories>,
}

pub struct AddDomainCommand {
    pub instance_id: String,
    pub domain: String,
}

impl AddDomain {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.add_domain",
        skip_all,
        fields(event_type = "instance.updated", category = "instance")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: AddDomainCommand,
    ) -> Result<DomainRecord, AppError> {
        if cmd.domain.is_empty() {
            return Err(AppError::validation("domain is required"));
        }

        // Authz: caller must be admin on the target instance
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("instance:{}", cmd.instance_id)).await?;

        let now = crate::users::chrono_now();
        let record = DomainRecord {
            domain: cmd.domain.clone(),
            is_primary: false,
            state: "active".to_string(),
            verified: false,
            created_at: now.clone(),
            updated_at: now,
        };

        self.repos
            .instances
            .set_domain(&cmd.instance_id, &record)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::InstanceUpdated {
                    instance_id: cmd.instance_id,
                    fields_changed: vec!["domain_added".to_string()],
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

pub struct RemoveDomain {
    repos: Arc<Repositories>,
}

impl RemoveDomain {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.remove_domain",
        skip_all,
        fields(event_type = "instance.updated", category = "instance")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        instance_id: &str,
        domain: &str,
    ) -> Result<DomainRemoveResult, AppError> {
        // Authz: caller must be admin on the target instance
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("instance:{}", instance_id)).await?;

        let result = self
            .repos
            .instances
            .remove_domain(instance_id, domain)
            .await
            .map_err(AppError::Internal)?;

        if result == DomainRemoveResult::Deleted {
            self.repos
                .events
                .append(
                    ctx.instance_id(),
                    &DomainEvent::InstanceUpdated {
                        instance_id: instance_id.to_string(),
                        fields_changed: vec!["domain_removed".to_string()],
                        actor_id: ctx.user_id().to_string(),
                    },
                    None,
                    None,
                    None,
                )
                .await
                .map_err(AppError::Internal)?;
        }

        Ok(result)
    }
}
