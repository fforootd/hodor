use crate::context::ActorContext;
use crate::error::AppError;
use crate::event::DomainEvent;
use crate::repo::{CreatedSession, Repositories};
use std::sync::Arc;

// ─── StartLogin ───

pub struct StartLogin {
    repos: Arc<Repositories>,
}

pub struct StartLoginCommand {
    pub flow_id: Option<String>,
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
    pub scope: Option<String>,
}

pub struct StartLoginResult {
    pub flow_id: String,
    pub current_step: String,
    pub ui_nodes: serde_json::Value,
}

impl StartLogin {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.start_login",
        skip_all,
        fields(event_type = "auth.login_started", category = "auth")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: StartLoginCommand,
    ) -> Result<StartLoginResult, AppError> {
        let flow_id = cmd
            .flow_id
            .unwrap_or_else(|| uuid::Uuid::now_v7().to_string());

        // Load login flow configuration to determine steps
        let flows = self
            .repos
            .login_flows
            .list_flows(ctx.instance_id())
            .await
            .map_err(AppError::Internal)?;

        let default_flow = flows
            .iter()
            .find(|f| f.is_default && f.enabled)
            .or(flows.first());

        let step = "identifier";

        // Build UI nodes for the identifier step (ADR-019 server-driven login)
        let ui_nodes = serde_json::json!({
            "nodes": [
                {
                    "type": "input",
                    "group": "identifier",
                    "attributes": {
                        "name": "identifier",
                        "type": "text",
                        "required": true,
                        "label": "Username or Email",
                    }
                },
                {
                    "type": "button",
                    "group": "submit",
                    "attributes": {
                        "name": "method",
                        "type": "submit",
                        "value": "identifier",
                        "label": "Continue",
                    }
                }
            ],
            "flow_config": default_flow.map(|f| &f.config),
        });

        Ok(StartLoginResult {
            flow_id,
            current_step: step.to_string(),
            ui_nodes,
        })
    }
}

// ─── SubmitLoginStep ───

pub struct SubmitLoginStep {
    repos: Arc<Repositories>,
}

pub struct SubmitLoginStepCommand {
    pub flow_id: String,
    pub step: String,
    pub data: serde_json::Value,
}

pub struct SubmitLoginStepResult {
    pub flow_id: String,
    pub current_step: String,
    pub completed: bool,
    pub user_id: Option<String>,
    pub ui_nodes: serde_json::Value,
}

impl SubmitLoginStep {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.submit_login_step",
        skip_all,
        fields(event_type = "auth.login_step", category = "auth")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: SubmitLoginStepCommand,
    ) -> Result<SubmitLoginStepResult, AppError> {
        match cmd.step.as_str() {
            "identifier" => self.handle_identifier(ctx, &cmd).await,
            "password" => self.handle_password(ctx, &cmd).await,
            _ => Err(AppError::validation(format!(
                "unknown login step: {}",
                cmd.step
            ))),
        }
    }

    async fn handle_identifier(
        &self,
        ctx: &ActorContext,
        cmd: &SubmitLoginStepCommand,
    ) -> Result<SubmitLoginStepResult, AppError> {
        let identifier = cmd.data["identifier"]
            .as_str()
            .ok_or_else(|| AppError::validation("identifier is required"))?;

        // Look up user
        let user = self
            .repos
            .users
            .find_by_identifier(ctx.instance_id(), identifier)
            .await
            .map_err(AppError::Internal)?
            .ok_or_else(|| AppError::not_found("user", identifier))?;

        // Advance to password step
        let ui_nodes = serde_json::json!({
            "nodes": [
                {
                    "type": "input",
                    "group": "password",
                    "attributes": {
                        "name": "password",
                        "type": "password",
                        "required": true,
                        "label": "Password",
                    }
                },
                {
                    "type": "button",
                    "group": "submit",
                    "attributes": {
                        "name": "method",
                        "type": "submit",
                        "value": "password",
                        "label": "Sign In",
                    }
                }
            ],
            "user_id": user.id,
        });

        Ok(SubmitLoginStepResult {
            flow_id: cmd.flow_id.clone(),
            current_step: "password".to_string(),
            completed: false,
            user_id: Some(user.id),
            ui_nodes,
        })
    }

    async fn handle_password(
        &self,
        _ctx: &ActorContext,
        cmd: &SubmitLoginStepCommand,
    ) -> Result<SubmitLoginStepResult, AppError> {
        let user_id = cmd.data["user_id"]
            .as_str()
            .ok_or_else(|| AppError::validation("user_id is required for password step"))?;

        // Password verification is handled by the transport adapter using Swapper.
        // The use case trusts that the caller has verified the password.
        // In a full implementation, the password hash would be checked here.

        Ok(SubmitLoginStepResult {
            flow_id: cmd.flow_id.clone(),
            current_step: "complete".to_string(),
            completed: true,
            user_id: Some(user_id.to_string()),
            ui_nodes: serde_json::json!({ "nodes": [] }),
        })
    }
}

// ─── IssueSession ───

pub struct IssueSession {
    repos: Arc<Repositories>,
}

pub struct IssueSessionCommand {
    pub user_id: String,
    pub auth_method: String,
    pub user_agent: String,
    pub ip_address: String,
    pub fingerprint: String,
}

impl IssueSession {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.issue_session",
        skip_all,
        fields(event_type = "session.started", category = "session")
    )]
    pub async fn execute(
        &self,
        ctx: &ActorContext,
        cmd: IssueSessionCommand,
    ) -> Result<CreatedSession, AppError> {
        // Verify user exists and is active
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

        let session = self
            .repos
            .sessions
            .create(
                ctx.instance_id(),
                &cmd.user_id,
                &user.org_id,
                &cmd.auth_method,
                &cmd.user_agent,
                &cmd.ip_address,
                &cmd.fingerprint,
            )
            .await
            .map_err(AppError::Internal)?;

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::SessionStarted {
                    session_id: session.session_id.clone(),
                    user_id: cmd.user_id,
                    auth_method: cmd.auth_method,
                },
                None,
                Some(&session.session_id),
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(session)
    }
}

// ─── RevokeSession ───

pub struct RevokeSession {
    repos: Arc<Repositories>,
}

impl RevokeSession {
    pub fn new(repos: Arc<Repositories>) -> Self {
        Self { repos }
    }

    #[tracing::instrument(
        name = "use_case.revoke_session",
        skip_all,
        fields(event_type = "session.revoked", category = "session")
    )]
    pub async fn execute(&self, ctx: &ActorContext, session_id: &str) -> Result<(), AppError> {
        // Only operator admins can revoke arbitrary sessions by ID.
        // Regular users revoke their own session by logging out (separate path).
        if !ctx.is_operator_admin() {
            return Err(AppError::PermissionDenied {
                reason: "session revocation requires operator admin".to_string(),
            });
        }

        let revoked = self
            .repos
            .sessions
            .revoke(ctx.instance_id(), session_id)
            .await
            .map_err(AppError::Internal)?;

        if !revoked {
            return Err(AppError::not_found("session", session_id));
        }

        self.repos
            .events
            .append(
                ctx.instance_id(),
                &DomainEvent::SessionRevoked {
                    session_id: session_id.to_string(),
                    actor_id: ctx.user_id().to_string(),
                },
                None,
                Some(session_id),
                None,
            )
            .await
            .map_err(AppError::Internal)?;

        Ok(())
    }
}
