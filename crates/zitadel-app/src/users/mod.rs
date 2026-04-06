use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{ListParams, ListResult, Repositories, UserRecord};
use std::sync::Arc;

// ─── CreateUser ───

pub struct CreateUser {
    repos: Arc<Repositories>,
}

pub struct CreateUserCommand {
    pub identifier: String,
    pub display_name: String,
    pub user_type: String,
    pub schema_id: String,
    pub org_id: Option<String>,
    pub metadata: serde_json::Value,
}

impl CreateUser {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.create_user",
        skip_all,
        fields(
            event_type = "user.created",
            category = "user",
            aggregate_type = "user",
        )
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: CreateUserCommand,
    ) -> Result<UserRecord, AppError> {
        // Validate
        if cmd.identifier.is_empty() {
            return Err(AppError::validation("identifier is required"));
        }

        // Check for duplicates
        let existing = self
            .repos
            .users
            .find_by_identifier(ctx.instance_id(), &cmd.identifier)
            .await
            .map_err(AppError::Internal)?;
        if existing.is_some() {
            return Err(AppError::already_exists("user", &cmd.identifier));
        }

        // Resolve org — use provided org_id or fall back to first org
        let org_id = match cmd.org_id {
            Some(ref id) => id.clone(),
            None => self
                .repos
                .orgs
                .first_org_id(ctx.instance_id())
                .await
                .map_err(AppError::Internal)?
                .unwrap_or_default(),
        };

        // Authz: caller must be admin on the target org
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("org:{}", org_id)).await?;

        let id = uuid::Uuid::now_v7().to_string();
        let now = chrono_now();

        let record = UserRecord {
            id: id.clone(),
            org_id: org_id.clone(),
            identifier: cmd.identifier.clone(),
            display_name: cmd.display_name,
            user_type: cmd.user_type,
            state: "active".to_string(),
            schema_id: cmd.schema_id.clone(),
            metadata: cmd.metadata,
            created_at: now.clone(),
            updated_at: now,
        };

        let created = self
            .repos
            .users
            .create(ctx.instance_id(), &record)
            .await
            .map_err(AppError::Internal)?;

        // Emit domain event (in same TX in the repo implementation)
        let event = DomainEvent::UserCreated {
            user_id: id,
            org_id,
            identifier: cmd.identifier,
            schema_type: cmd.schema_id,
            actor_id: ctx.user_id().to_string(),
        };
        self.repos
            .events
            .append(ctx.instance_id(), &event, None, None, None)
            .await
            .map_err(AppError::Internal)?;

        Ok(created)
    }
}

// ─── GetUser ───

pub struct GetUser {
    repos: Arc<Repositories>,
}

impl GetUser {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.get_user", skip_all)]
    pub async fn execute(&self, ctx: &ActorContext, user_id: &str) -> Result<UserRecord, AppError> {
        let user = self
            .repos
            .users
            .get(ctx.instance_id(), user_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("user", user_id))?;

        // Authz: caller must be viewer on the user's org
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", user.org_id),
        )
        .await?;

        Ok(user)
    }
}

// ─── ListUsers ───

pub struct ListUsers {
    repos: Arc<Repositories>,
}

impl ListUsers {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(name = "use_case.list_users", skip_all)]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        org_id: Option<&str>,
        params: &ListParams,
    ) -> Result<ListResult<UserRecord>, AppError> {
        // Authz: caller must be viewer on the target org (or own org if unfiltered)
        let effective_org = org_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| ctx.org_id().to_string());
        crate::authz::require_permission(
            &self.repos,
            ctx,
            "viewer",
            &format!("org:{}", effective_org),
        )
        .await?;

        self.repos
            .users
            .list(ctx.instance_id(), org_id, params)
            .await
            .map_err(AppError::Internal)
    }
}

// ─── UpdateUser ───

pub struct UpdateUser {
    repos: Arc<Repositories>,
}

pub struct UpdateUserCommand {
    pub user_id: String,
    pub display_name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

impl UpdateUser {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.update_user",
        skip_all,
        fields(event_type = "user.updated", category = "user")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: UpdateUserCommand,
    ) -> Result<UserRecord, AppError> {
        let mut user = self
            .repos
            .users
            .get(ctx.instance_id(), &cmd.user_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("user", &cmd.user_id))?;

        // Authz: caller must be admin on the target user
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("user:{}", cmd.user_id)).await?;

        let mut fields_changed = Vec::new();

        if let Some(name) = cmd.display_name {
            user.display_name = name;
            fields_changed.push("display_name".to_string());
        }
        if let Some(meta) = cmd.metadata {
            user.metadata = meta;
            fields_changed.push("metadata".to_string());
        }

        if fields_changed.is_empty() {
            return Ok(user);
        }

        user.updated_at = chrono_now();

        let updated = self
            .repos
            .users
            .update(ctx.instance_id(), &user)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::UserUpdated {
                    user_id: cmd.user_id,
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

// ─── DeleteUser ───

pub struct DeleteUser {
    repos: Arc<Repositories>,
}

impl DeleteUser {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.delete_user",
        skip_all,
        fields(event_type = "user.deleted", category = "user")
    )]
    pub async fn execute(&self, ctx: &ActorContext, user_id: &str) -> Result<(), AppError> {
        let _user = self
            .repos
            .users
            .get(ctx.instance_id(), user_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("user", user_id))?;

        // Authz: caller must be admin on the target user
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("user:{}", user_id)).await?;

        self.repos
            .users
            .delete(ctx.instance_id(), user_id)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::UserDeleted {
                    user_id: user_id.to_string(),
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

// ─── DeactivateUser ───

pub struct DeactivateUser {
    repos: Arc<Repositories>,
}

impl DeactivateUser {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.deactivate_user",
        skip_all,
        fields(event_type = "user.deactivated", category = "user")
    )]
    pub async fn execute(&self, ctx: &ActorContext, user_id: &str) -> Result<(), AppError> {
        // Verify user exists and is active (instance-scoped: returns 404 for cross-instance)
        let user = self
            .repos
            .users
            .get(ctx.instance_id(), user_id)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("user", user_id))?;

        // Authz: caller must be admin on the target user
        crate::authz::require_permission(&self.repos, ctx, "admin", &format!("user:{}", user_id)).await?;

        if user.state != "active" {
            return Err(AppError::InvalidState {
                entity: "user".to_string(),
                id: user_id.to_string(),
                current_state: user.state,
                expected_state: "active".to_string(),
            });
        }

        self.repos
            .users
            .deactivate(ctx.instance_id(), user_id)
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::UserDeactivated {
                    user_id: user_id.to_string(),
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

pub fn chrono_now() -> String {
    // Use the same format as the existing codebase
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let nanos = now.subsec_nanos();
    format!(
        "{}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        1970 + secs / 31_536_000,
        (secs % 31_536_000) / 2_592_000 + 1,
        (secs % 2_592_000) / 86_400 + 1,
        (secs % 86_400) / 3600,
        (secs % 3600) / 60,
        secs % 60,
        nanos / 1_000_000,
    )
}
