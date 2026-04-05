//! Application layer for Zitadel (ADR-032).
//!
//! This crate owns all business orchestration. Transport adapters (HTTP handlers,
//! login routes, OIDC endpoints, CLI commands) parse input, build commands,
//! call use cases, and map results. Storage drivers implement repository traits.
//!
//! # Architecture
//!
//! ```text
//! Transport Adapter → ActorContext + Command → UseCase → Repository Ports → Storage Driver
//!                                                ↕
//!                                          Hook Pipeline
//!                                                ↓
//!                                          Domain Events
//! ```

pub mod context;
pub mod error;
pub mod event;
pub mod hook;
pub mod hook_engine;
pub mod mock;
pub mod repo;
pub mod usecase;

// Use case modules
pub mod apps;
pub mod auth;
pub mod credentials;
pub mod groups;
pub mod instances;
pub mod orgs;
pub mod providers;
pub mod schemas;
pub mod settings;
pub mod users;

// Re-exports for convenience
pub use context::{
    ActorContext, AuthContext, Capability, Identity, InstanceContext, RequestContext,
};
pub use error::AppError;
pub use event::DomainEvent;
pub use hook::{EffectHook, HookContext, HookPhase, HookPipeline, PolicyInterceptor};
pub use hook_engine::HookPipelineBuilder;
pub use repo::Repositories;
pub use usecase::{UseCase, UseCaseRunner};

use std::sync::Arc;

/// Holds all use case instances. Injected into transport adapter state.
pub struct ApplicationServices {
    // Users
    pub create_user: users::CreateUser,
    pub get_user: users::GetUser,
    pub list_users: users::ListUsers,
    pub update_user: users::UpdateUser,
    pub deactivate_user: users::DeactivateUser,

    // Credentials
    pub set_password: credentials::SetPassword,
    pub verify_password: credentials::VerifyPassword,
    pub link_identity: credentials::LinkIdentity,

    // Auth
    pub start_login: auth::StartLogin,
    pub submit_login_step: auth::SubmitLoginStep,
    pub issue_session: auth::IssueSession,
    pub revoke_session: auth::RevokeSession,

    // Orgs
    pub create_org: orgs::CreateOrg,
    pub get_org: orgs::GetOrg,
    pub list_orgs: orgs::ListOrgs,
    pub update_org: orgs::UpdateOrg,

    // Groups
    pub create_group: groups::CreateGroup,
    pub get_group: groups::GetGroup,
    pub list_groups: groups::ListGroups,
    pub update_group: groups::UpdateGroup,

    // Apps
    pub create_app: apps::CreateApp,
    pub get_app: apps::GetApp,
    pub list_apps: apps::ListApps,
    pub update_app: apps::UpdateApp,

    // Instances
    pub create_instance: instances::CreateInstance,
    pub get_instance: instances::GetInstance,
    pub list_instances: instances::ListInstances,
    pub update_instance: instances::UpdateInstance,
    pub deprovision_instance: instances::DeprovisionInstance,

    // Settings
    pub get_settings: settings::GetSettings,
    pub update_settings: settings::UpdateSettings,

    // Providers
    pub create_provider: providers::CreateProvider,
    pub get_provider: providers::GetProvider,
    pub list_providers: providers::ListProviders,
    pub update_provider: providers::UpdateProvider,
    pub delete_provider: providers::DeleteProvider,

    // Schemas
    pub register_schema: schemas::RegisterSchema,
    pub get_schema: schemas::GetSchema,
    pub list_schemas: schemas::ListSchemas,

    // Hook pipeline
    pub hooks: Arc<HookPipeline>,
}

impl ApplicationServices {
    /// Build all use cases from a set of repositories and a hook pipeline.
    pub fn new(repos: Arc<Repositories>, hooks: Arc<HookPipeline>) -> Self {
        Self {
            // Users
            create_user: users::CreateUser::new(repos.clone()),
            get_user: users::GetUser::new(repos.clone()),
            list_users: users::ListUsers::new(repos.clone()),
            update_user: users::UpdateUser::new(repos.clone()),
            deactivate_user: users::DeactivateUser::new(repos.clone()),

            // Credentials
            set_password: credentials::SetPassword::new(repos.clone()),
            verify_password: credentials::VerifyPassword::new(repos.clone()),
            link_identity: credentials::LinkIdentity::new(repos.clone()),

            // Auth
            start_login: auth::StartLogin::new(repos.clone()),
            submit_login_step: auth::SubmitLoginStep::new(repos.clone()),
            issue_session: auth::IssueSession::new(repos.clone()),
            revoke_session: auth::RevokeSession::new(repos.clone()),

            // Orgs
            create_org: orgs::CreateOrg::new(repos.clone()),
            get_org: orgs::GetOrg::new(repos.clone()),
            list_orgs: orgs::ListOrgs::new(repos.clone()),
            update_org: orgs::UpdateOrg::new(repos.clone()),

            // Groups
            create_group: groups::CreateGroup::new(repos.clone()),
            get_group: groups::GetGroup::new(repos.clone()),
            list_groups: groups::ListGroups::new(repos.clone()),
            update_group: groups::UpdateGroup::new(repos.clone()),

            // Apps
            create_app: apps::CreateApp::new(repos.clone()),
            get_app: apps::GetApp::new(repos.clone()),
            list_apps: apps::ListApps::new(repos.clone()),
            update_app: apps::UpdateApp::new(repos.clone()),

            // Instances
            create_instance: instances::CreateInstance::new(repos.clone()),
            get_instance: instances::GetInstance::new(repos.clone()),
            list_instances: instances::ListInstances::new(repos.clone()),
            update_instance: instances::UpdateInstance::new(repos.clone()),
            deprovision_instance: instances::DeprovisionInstance::new(repos.clone()),

            // Settings
            get_settings: settings::GetSettings::new(repos.clone()),
            update_settings: settings::UpdateSettings::new(repos.clone()),

            // Providers
            create_provider: providers::CreateProvider::new(repos.clone()),
            get_provider: providers::GetProvider::new(repos.clone()),
            list_providers: providers::ListProviders::new(repos.clone()),
            update_provider: providers::UpdateProvider::new(repos.clone()),
            delete_provider: providers::DeleteProvider::new(repos.clone()),

            // Schemas
            register_schema: schemas::RegisterSchema::new(repos.clone()),
            get_schema: schemas::GetSchema::new(repos.clone()),
            list_schemas: schemas::ListSchemas::new(repos.clone()),

            hooks,
        }
    }
}
