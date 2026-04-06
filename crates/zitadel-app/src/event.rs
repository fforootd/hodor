use serde::{Deserialize, Serialize};

/// Typed domain events emitted by use cases.
///
/// Each variant represents a business state change that was persisted
/// in the same DB transaction as the state change itself (per ADR-010).
///
/// These are distinct from observability events (request logs, auth metrics)
/// which flow through the tracing → ObservabilityLayer → analytics pipeline.
/// Both end up in the same `events` table via different write paths.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "event_type")]
pub enum DomainEvent {
    // ── Users ──
    #[serde(rename = "user.created")]
    UserCreated {
        user_id: String,
        org_id: String,
        identifier: String,
        schema_type: String,
        actor_id: String,
    },
    #[serde(rename = "user.updated")]
    UserUpdated {
        user_id: String,
        fields_changed: Vec<String>,
        actor_id: String,
    },
    #[serde(rename = "user.deactivated")]
    UserDeactivated { user_id: String, actor_id: String },
    #[serde(rename = "user.deleted")]
    UserDeleted { user_id: String, actor_id: String },

    // ── Credentials ──
    #[serde(rename = "credential.password_set")]
    PasswordSet { user_id: String, actor_id: String },
    #[serde(rename = "credential.identity_linked")]
    IdentityLinked {
        user_id: String,
        provider_id: String,
        external_sub: String,
        actor_id: String,
    },
    #[serde(rename = "credential.identity_unlinked")]
    IdentityUnlinked {
        user_id: String,
        provider_id: String,
        actor_id: String,
    },

    // ── Auth / Sessions ──
    #[serde(rename = "session.started")]
    SessionStarted {
        session_id: String,
        user_id: String,
        auth_method: String,
    },
    #[serde(rename = "session.revoked")]
    SessionRevoked {
        session_id: String,
        actor_id: String,
    },
    #[serde(rename = "auth.login_completed")]
    LoginFlowCompleted {
        flow_id: String,
        user_id: String,
        outcome: String,
    },
    #[serde(rename = "auth.otp_verified")]
    OtpVerified { user_id: String, method: String },

    // ── Tokens ──
    #[serde(rename = "token.issued")]
    TokenIssued {
        token_id: String,
        client_id: String,
        subject: String,
        grant_type: String,
    },
    #[serde(rename = "token.revoked")]
    TokenRevoked { token_id: String, actor_id: String },

    // ── Organizations ──
    #[serde(rename = "org.created")]
    OrgCreated {
        org_id: String,
        name: String,
        actor_id: String,
    },
    #[serde(rename = "org.updated")]
    OrgUpdated {
        org_id: String,
        fields_changed: Vec<String>,
        actor_id: String,
    },
    #[serde(rename = "org.deleted")]
    OrgDeleted {
        org_id: String,
        actor_id: String,
    },

    // ── Groups ──
    #[serde(rename = "group.created")]
    GroupCreated {
        group_id: String,
        org_id: String,
        name: String,
        actor_id: String,
    },
    #[serde(rename = "group.updated")]
    GroupUpdated {
        group_id: String,
        fields_changed: Vec<String>,
        actor_id: String,
    },
    #[serde(rename = "group.deleted")]
    GroupDeleted {
        group_id: String,
        actor_id: String,
    },

    // ── Apps ──
    #[serde(rename = "app.created")]
    AppCreated {
        app_id: String,
        group_id: String,
        protocol: String,
        actor_id: String,
    },
    #[serde(rename = "app.updated")]
    AppUpdated {
        app_id: String,
        fields_changed: Vec<String>,
        actor_id: String,
    },

    // ── Instances ──
    #[serde(rename = "instance.created")]
    InstanceCreated {
        instance_id: String,
        parent_instance_id: Option<String>,
        owner_org_id: String,
        kind: String,
        actor_id: String,
    },
    #[serde(rename = "instance.updated")]
    InstanceUpdated {
        instance_id: String,
        fields_changed: Vec<String>,
        actor_id: String,
    },
    #[serde(rename = "instance.deprovisioned")]
    InstanceDeprovisioned {
        instance_id: String,
        actor_id: String,
    },

    // ── Settings ──
    #[serde(rename = "settings.updated")]
    SettingsUpdated {
        scope: String,
        settings_type: String,
        actor_id: String,
    },
    #[serde(rename = "settings.deleted")]
    SettingsDeleted {
        settings_type: String,
        actor_id: String,
    },

    // ── Providers ──
    #[serde(rename = "provider.configured")]
    ProviderConfigured {
        provider_id: String,
        protocol: String,
        actor_id: String,
    },
    #[serde(rename = "provider.removed")]
    ProviderRemoved {
        provider_id: String,
        actor_id: String,
    },

    // ── Schemas ──
    #[serde(rename = "schema.registered")]
    SchemaRegistered {
        schema_id: String,
        schema_type: String,
        actor_id: String,
    },
    #[serde(rename = "schema.updated")]
    SchemaUpdated { schema_id: String, actor_id: String },

    // ── Login Flows ──
    #[serde(rename = "login_flow.configured")]
    LoginFlowConfigured { flow_id: String, actor_id: String },

    // ── Security ──
    #[serde(rename = "security.bot_detection")]
    BotDetection {
        fingerprint: String,
        payload: serde_json::Value,
        metadata: serde_json::Value,
    },

    // ── Actions ──
    #[serde(rename = "action.created")]
    ActionCreated {
        action_id: String,
        name: String,
        hook: String,
        actor_id: String,
    },
    #[serde(rename = "action.updated")]
    ActionUpdated {
        action_id: String,
        fields_changed: Vec<String>,
        actor_id: String,
    },
    #[serde(rename = "action.deleted")]
    ActionDeleted {
        action_id: String,
        actor_id: String,
    },

    // ── Resources (generic named) ──
    #[serde(rename = "resource.created")]
    ResourceCreated {
        resource_id: String,
        kind: String,
        name: String,
        actor_id: String,
    },
    #[serde(rename = "resource.updated")]
    ResourceUpdated {
        resource_id: String,
        kind: String,
        fields_changed: Vec<String>,
        actor_id: String,
    },
    #[serde(rename = "resource.deleted")]
    ResourceDeleted {
        resource_id: String,
        kind: String,
        actor_id: String,
    },

    // ── Memberships ──
    #[serde(rename = "membership.changed")]
    MembershipChanged {
        entity_type: String,
        entity_id: String,
        user_id: String,
        action: String, // "added" or "removed"
        role: String,
        actor_id: String,
    },

    // ── PATs ──
    #[serde(rename = "pat.created")]
    PatCreated {
        pat_id: String,
        user_id: String,
        actor_id: String,
    },
    #[serde(rename = "pat.revoked")]
    PatRevoked { pat_id: String, actor_id: String },
}

impl DomainEvent {
    /// Returns the event_type string for the events table.
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::UserCreated { .. } => "user.created",
            Self::UserUpdated { .. } => "user.updated",
            Self::UserDeactivated { .. } => "user.deactivated",
            Self::UserDeleted { .. } => "user.deleted",
            Self::PasswordSet { .. } => "credential.password_set",
            Self::IdentityLinked { .. } => "credential.identity_linked",
            Self::IdentityUnlinked { .. } => "credential.identity_unlinked",
            Self::SessionStarted { .. } => "session.started",
            Self::SessionRevoked { .. } => "session.revoked",
            Self::LoginFlowCompleted { .. } => "auth.login_completed",
            Self::OtpVerified { .. } => "auth.otp_verified",
            Self::TokenIssued { .. } => "token.issued",
            Self::TokenRevoked { .. } => "token.revoked",
            Self::OrgCreated { .. } => "org.created",
            Self::OrgUpdated { .. } => "org.updated",
            Self::OrgDeleted { .. } => "org.deleted",
            Self::GroupCreated { .. } => "group.created",
            Self::GroupUpdated { .. } => "group.updated",
            Self::GroupDeleted { .. } => "group.deleted",
            Self::AppCreated { .. } => "app.created",
            Self::AppUpdated { .. } => "app.updated",
            Self::InstanceCreated { .. } => "instance.created",
            Self::InstanceUpdated { .. } => "instance.updated",
            Self::InstanceDeprovisioned { .. } => "instance.deprovisioned",
            Self::SettingsUpdated { .. } => "settings.updated",
            Self::SettingsDeleted { .. } => "settings.deleted",
            Self::ProviderConfigured { .. } => "provider.configured",
            Self::ProviderRemoved { .. } => "provider.removed",
            Self::SchemaRegistered { .. } => "schema.registered",
            Self::SchemaUpdated { .. } => "schema.updated",
            Self::LoginFlowConfigured { .. } => "login_flow.configured",
            Self::BotDetection { .. } => "security.bot_detection",
            Self::ActionCreated { .. } => "action.created",
            Self::ActionUpdated { .. } => "action.updated",
            Self::ActionDeleted { .. } => "action.deleted",
            Self::ResourceCreated { .. } => "resource.created",
            Self::ResourceUpdated { .. } => "resource.updated",
            Self::ResourceDeleted { .. } => "resource.deleted",
            Self::MembershipChanged { .. } => "membership.changed",
            Self::PatCreated { .. } => "pat.created",
            Self::PatRevoked { .. } => "pat.revoked",
        }
    }

    /// Returns the category (first segment of event_type).
    pub fn category(&self) -> &'static str {
        match self {
            Self::UserCreated { .. }
            | Self::UserUpdated { .. }
            | Self::UserDeactivated { .. }
            | Self::UserDeleted { .. } => "user",
            Self::PasswordSet { .. }
            | Self::IdentityLinked { .. }
            | Self::IdentityUnlinked { .. } => "credential",
            Self::SessionStarted { .. } | Self::SessionRevoked { .. } => "session",
            Self::LoginFlowCompleted { .. } | Self::OtpVerified { .. } => "auth",
            Self::TokenIssued { .. } | Self::TokenRevoked { .. } => "token",
            Self::OrgCreated { .. } | Self::OrgUpdated { .. } | Self::OrgDeleted { .. } => "org",
            Self::GroupCreated { .. } | Self::GroupUpdated { .. } | Self::GroupDeleted { .. } => "group",
            Self::AppCreated { .. } | Self::AppUpdated { .. } => "app",
            Self::InstanceCreated { .. }
            | Self::InstanceUpdated { .. }
            | Self::InstanceDeprovisioned { .. } => "instance",
            Self::SettingsUpdated { .. } | Self::SettingsDeleted { .. } => "settings",
            Self::ProviderConfigured { .. } | Self::ProviderRemoved { .. } => "provider",
            Self::SchemaRegistered { .. } | Self::SchemaUpdated { .. } => "schema",
            Self::LoginFlowConfigured { .. } => "login_flow",
            Self::BotDetection { .. } => "security",
            Self::ActionCreated { .. }
            | Self::ActionUpdated { .. }
            | Self::ActionDeleted { .. } => "action",
            Self::ResourceCreated { .. }
            | Self::ResourceUpdated { .. }
            | Self::ResourceDeleted { .. } => "resource",
            Self::MembershipChanged { .. } => "membership",
            Self::PatCreated { .. } | Self::PatRevoked { .. } => "pat",
        }
    }

    /// Returns the aggregate_id (the primary entity affected).
    pub fn aggregate_id(&self) -> &str {
        match self {
            Self::UserCreated { user_id, .. }
            | Self::UserUpdated { user_id, .. }
            | Self::UserDeactivated { user_id, .. }
            | Self::UserDeleted { user_id, .. } => user_id,
            Self::PasswordSet { user_id, .. }
            | Self::IdentityLinked { user_id, .. }
            | Self::IdentityUnlinked { user_id, .. } => user_id,
            Self::SessionStarted { session_id, .. } | Self::SessionRevoked { session_id, .. } => {
                session_id
            }
            Self::LoginFlowCompleted { flow_id, .. } => flow_id,
            Self::OtpVerified { user_id, .. } => user_id,
            Self::TokenIssued { token_id, .. } | Self::TokenRevoked { token_id, .. } => token_id,
            Self::OrgCreated { org_id, .. }
            | Self::OrgUpdated { org_id, .. }
            | Self::OrgDeleted { org_id, .. } => org_id,
            Self::GroupCreated { group_id, .. }
            | Self::GroupUpdated { group_id, .. }
            | Self::GroupDeleted { group_id, .. } => group_id,
            Self::AppCreated { app_id, .. } | Self::AppUpdated { app_id, .. } => app_id,
            Self::InstanceCreated { instance_id, .. }
            | Self::InstanceUpdated { instance_id, .. }
            | Self::InstanceDeprovisioned { instance_id, .. } => instance_id,
            Self::SettingsUpdated { scope, .. } => scope,
            Self::SettingsDeleted { settings_type, .. } => settings_type,
            Self::ProviderConfigured { provider_id, .. }
            | Self::ProviderRemoved { provider_id, .. } => provider_id,
            Self::SchemaRegistered { schema_id, .. } | Self::SchemaUpdated { schema_id, .. } => {
                schema_id
            }
            Self::LoginFlowConfigured { flow_id, .. } => flow_id,
            Self::BotDetection { fingerprint, .. } => fingerprint,
            Self::ActionCreated { action_id, .. }
            | Self::ActionUpdated { action_id, .. }
            | Self::ActionDeleted { action_id, .. } => action_id,
            Self::ResourceCreated { resource_id, .. }
            | Self::ResourceUpdated { resource_id, .. }
            | Self::ResourceDeleted { resource_id, .. } => resource_id,
            Self::MembershipChanged { entity_id, .. } => entity_id,
            Self::PatCreated { pat_id, .. } | Self::PatRevoked { pat_id, .. } => pat_id,
        }
    }

    /// Returns the actor_id (who performed the action).
    pub fn actor_id(&self) -> &str {
        match self {
            Self::UserCreated { actor_id, .. }
            | Self::UserUpdated { actor_id, .. }
            | Self::UserDeactivated { actor_id, .. }
            | Self::UserDeleted { actor_id, .. }
            | Self::PasswordSet { actor_id, .. }
            | Self::IdentityLinked { actor_id, .. }
            | Self::IdentityUnlinked { actor_id, .. }
            | Self::SessionRevoked { actor_id, .. }
            | Self::TokenRevoked { actor_id, .. }
            | Self::OrgCreated { actor_id, .. }
            | Self::OrgUpdated { actor_id, .. }
            | Self::OrgDeleted { actor_id, .. }
            | Self::GroupCreated { actor_id, .. }
            | Self::GroupUpdated { actor_id, .. }
            | Self::GroupDeleted { actor_id, .. }
            | Self::AppCreated { actor_id, .. }
            | Self::AppUpdated { actor_id, .. }
            | Self::InstanceCreated { actor_id, .. }
            | Self::InstanceUpdated { actor_id, .. }
            | Self::InstanceDeprovisioned { actor_id, .. }
            | Self::SettingsUpdated { actor_id, .. }
            | Self::SettingsDeleted { actor_id, .. }
            | Self::ProviderConfigured { actor_id, .. }
            | Self::ProviderRemoved { actor_id, .. }
            | Self::SchemaRegistered { actor_id, .. }
            | Self::SchemaUpdated { actor_id, .. }
            | Self::LoginFlowConfigured { actor_id, .. }
            | Self::ActionCreated { actor_id, .. }
            | Self::ActionUpdated { actor_id, .. }
            | Self::ActionDeleted { actor_id, .. }
            | Self::ResourceCreated { actor_id, .. }
            | Self::ResourceUpdated { actor_id, .. }
            | Self::ResourceDeleted { actor_id, .. }
            | Self::MembershipChanged { actor_id, .. }
            | Self::PatCreated { actor_id, .. }
            | Self::PatRevoked { actor_id, .. } => actor_id,
            // Events without explicit actor
            Self::SessionStarted { user_id, .. } => user_id,
            Self::LoginFlowCompleted { user_id, .. } => user_id,
            Self::OtpVerified { user_id, .. } => user_id,
            Self::TokenIssued { subject, .. } => subject,
            Self::BotDetection { fingerprint, .. } => fingerprint,
        }
    }
}
