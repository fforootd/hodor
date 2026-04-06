use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{LinkedIdentityRecord, Repositories};
use std::sync::Arc;

// ─── SetPassword ───

pub struct SetPassword {
    repos: Arc<Repositories>,
}

pub struct SetPasswordCommand {
    pub user_id: String,
    pub password_hash: String,
}

impl SetPassword {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.set_password",
        skip_all,
        fields(event_type = "credential.password_set", category = "credential")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: SetPasswordCommand,
    ) -> Result<(), AppError> {
        // Authz: caller must be admin on the target user, or be the user themselves
        crate::authz::require_permission_or_self(
            &self.repos,
            ctx,
            "admin",
            &format!("user:{}", cmd.user_id),
            &cmd.user_id,
        )
        .await?;

        // Verify user exists
        let user = self
            .repos
            .users
            .get(ctx.instance_id(), &cmd.user_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("user", &cmd.user_id))?;

        if user.state != "active" {
            return Err(AppError::InvalidState {
                entity: "user".to_string(),
                id: cmd.user_id.clone(),
                current_state: user.state,
                expected_state: "active".to_string(),
            });
        }

        self.repos
            .credentials
            .set_password(ctx.instance_id(), &cmd.user_id, &cmd.password_hash)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::PasswordSet {
                    user_id: cmd.user_id,
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

// ─── VerifyPassword ───

pub struct VerifyPassword {
    repos: Arc<Repositories>,
}

pub struct VerifyPasswordCommand {
    pub user_id: String,
    pub password: String,
}

pub struct VerifyPasswordResult {
    pub verified: bool,
    pub needs_rehash: bool,
    pub new_hash: Option<String>,
}

impl VerifyPassword {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.verify_password", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: VerifyPasswordCommand,
    ) -> Result<VerifyPasswordResult, AppError> {
        let hash = self
            .repos
            .credentials
            .get_password_hash(ctx.instance_id(), &cmd.user_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("credential", &cmd.user_id))?;

        // Password verification is delegated to the authn crate's Swapper.
        // The transport adapter should call Swapper::verify() with the hash
        // and update via SetPassword if needs_rehash is true.
        // For now, return the hash for the transport layer to verify.
        Ok(VerifyPasswordResult {
            verified: false, // Transport adapter does actual verification via Swapper
            needs_rehash: false,
            new_hash: Some(hash),
        })
    }
}

// ─── LinkIdentity ───

pub struct LinkIdentity {
    repos: Arc<Repositories>,
}

pub struct LinkIdentityCommand {
    pub user_id: String,
    pub provider_id: String,
    pub external_sub: String,
    pub external_email: Option<String>,
    pub raw_claims: serde_json::Value,
}

impl LinkIdentity {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.link_identity",
        skip_all,
        fields(event_type = "credential.identity_linked", category = "credential")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: LinkIdentityCommand,
    ) -> Result<(), AppError> {
        // Authz: caller must be admin on the target user
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "admin",
            &format!("user:{}", cmd.user_id),
        )
        .await?;

        // Verify user exists
        self.repos
            .users
            .get(ctx.instance_id(), &cmd.user_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("user", &cmd.user_id))?;

        // Check if already linked
        let existing = self
            .repos
            .credentials
            .find_by_external_sub(ctx.instance_id(), &cmd.provider_id, &cmd.external_sub)
            .await
            .map_err(AppError::Internal)?;
        if existing.is_some() {
            return Err(AppError::already_exists(
                "linked_identity",
                &cmd.external_sub,
            ));
        }

        let link = LinkedIdentityRecord {
            id: uuid::Uuid::now_v7().to_string(),
            user_id: cmd.user_id.clone(),
            provider_id: cmd.provider_id.clone(),
            external_sub: cmd.external_sub.clone(),
            external_email: cmd.external_email,
            raw_claims: cmd.raw_claims,
        };

        self.repos
            .credentials
            .link_identity(ctx.instance_id(), &link)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::IdentityLinked {
                    user_id: cmd.user_id,
                    provider_id: cmd.provider_id,
                    external_sub: cmd.external_sub,
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
