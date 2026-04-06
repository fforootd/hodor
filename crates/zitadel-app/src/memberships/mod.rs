use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{MembershipRecord, Repositories};
use std::sync::Arc;

pub struct ListMemberships {
    repos: Arc<Repositories>,
}

impl ListMemberships {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_memberships", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        entity_type: &str,
        entity_id: &str,
    ) -> Result<Vec<MembershipRecord>, AppError> {
        self.repos
            .memberships
            .list(ctx.instance_id(), entity_type, entity_id)
            .await
            .map_err(AppError::Internal)
    }
}

pub struct AddMembership {
    repos: Arc<Repositories>,
}

pub struct AddMembershipCommand {
    pub entity_type: String,
    pub entity_id: String,
    pub user_id: String,
    pub role: String,
}

impl AddMembership {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.add_membership", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: AddMembershipCommand,
    ) -> Result<MembershipRecord, AppError> {
        self.repos
            .memberships
            .add(
                ctx.instance_id(),
                &cmd.entity_type,
                &cmd.entity_id,
                &cmd.user_id,
                &cmd.role,
            )
            .await
            .map_err(AppError::Internal)?;

        let mut uow = self
            .repos
            .uow
            .begin(ctx.instance_id())
            .await
            .map_err(AppError::Internal)?;
        uow.buffer_event(
            DomainEvent::MembershipChanged {
                entity_type: cmd.entity_type,
                entity_id: cmd.entity_id,
                user_id: cmd.user_id.clone(),
                action: "added".to_string(),
                role: cmd.role.clone(),
                actor_id: ctx.user_id().to_string(),
            },
            None,
            None,
            None,
        );
        uow.commit().await.map_err(AppError::Internal)?;

        Ok(MembershipRecord {
            user_id: cmd.user_id,
            display_name: None,
            role: cmd.role,
            added_at: String::new(),
        })
    }
}

pub struct RemoveMembership {
    repos: Arc<Repositories>,
}

pub struct RemoveMembershipCommand {
    pub entity_type: String,
    pub entity_id: String,
    pub user_id: String,
}

impl RemoveMembership {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.remove_membership", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: RemoveMembershipCommand,
    ) -> Result<(), AppError> {
        self.repos
            .memberships
            .remove(
                ctx.instance_id(),
                &cmd.entity_type,
                &cmd.entity_id,
                &cmd.user_id,
            )
            .await
            .map_err(AppError::Internal)?;

        let mut uow = self
            .repos
            .uow
            .begin(ctx.instance_id())
            .await
            .map_err(AppError::Internal)?;
        uow.buffer_event(
            DomainEvent::MembershipChanged {
                entity_type: cmd.entity_type,
                entity_id: cmd.entity_id,
                user_id: cmd.user_id,
                action: "removed".to_string(),
                role: String::new(),
                actor_id: ctx.user_id().to_string(),
            },
            None,
            None,
            None,
        );
        uow.commit().await.map_err(AppError::Internal)
    }
}
