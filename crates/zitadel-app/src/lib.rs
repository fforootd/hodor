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

pub mod authz;
pub mod context;
pub mod error;
pub mod event;
pub mod hook;
pub mod hook_engine;
pub mod mock;
pub mod repo;
pub mod usecase;

// Use case modules
pub mod actions;
pub mod apps;
pub mod auth;
pub mod credentials;
pub mod groups;
pub mod jobs;
pub mod instances;
pub mod memberships;
pub mod orgs;
pub mod pats;
pub mod providers;
pub mod console;
pub mod login_flows;
pub mod resources;
pub mod saved_queries;
pub mod schemas;
pub mod search;
pub mod sessions;
pub mod settings;
pub mod support;
pub mod telemetry;
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
    pub repos: Arc<Repositories>,

    // Users
    pub create_user: users::CreateUser,
    pub get_user: users::GetUser,
    pub list_users: users::ListUsers,
    pub update_user: users::UpdateUser,
    pub deactivate_user: users::DeactivateUser,
    pub delete_user: users::DeleteUser,

    // Credentials
    pub set_password: credentials::SetPassword,
    pub verify_password: credentials::VerifyPassword,
    pub link_identity: credentials::LinkIdentity,

    // Auth
    pub start_login: auth::StartLogin,
    pub submit_login_step: auth::SubmitLoginStep,
    pub issue_session: auth::IssueSession,
    pub revoke_session: auth::RevokeSession,

    // Sessions
    pub list_sessions: sessions::ListSessions,
    pub get_session: sessions::GetSession,

    // Orgs
    pub create_org: orgs::CreateOrg,
    pub get_org: orgs::GetOrg,
    pub list_orgs: orgs::ListOrgs,
    pub update_org: orgs::UpdateOrg,
    pub delete_org: orgs::DeleteOrg,

    // Groups
    pub create_group: groups::CreateGroup,
    pub get_group: groups::GetGroup,
    pub list_groups: groups::ListGroups,
    pub update_group: groups::UpdateGroup,
    pub delete_group: groups::DeleteGroup,

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
    pub list_domains: instances::ListDomains,
    pub add_domain: instances::AddDomain,
    pub remove_domain: instances::RemoveDomain,

    // Support
    pub create_support_grant: support::CreateSupportGrant,
    pub list_support_grants: support::ListSupportGrants,
    pub revoke_support_grant: support::RevokeSupportGrant,

    // Settings
    pub get_settings: settings::GetSettings,
    pub update_settings: settings::UpdateSettings,
    pub delete_settings: settings::DeleteSettings,

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
    pub update_schema: schemas::UpdateSchema,
    pub promote_schema: schemas::PromoteSchema,
    pub count_schema_users: schemas::CountSchemaUsers,

    // PATs
    pub create_pat: pats::CreatePat,
    pub list_pats: pats::ListPats,
    pub revoke_pat: pats::RevokePat,

    // Actions
    pub list_actions: actions::ListActions,
    pub get_action: actions::GetAction,
    pub create_action: actions::CreateAction,
    pub update_action: actions::UpdateAction,
    pub delete_action: actions::DeleteAction,

    // Search
    pub search_entities: search::SearchEntities,

    // Jobs
    pub list_jobs: jobs::ListJobs,

    // Memberships
    pub list_memberships: memberships::ListMemberships,
    pub add_membership: memberships::AddMembership,
    pub remove_membership: memberships::RemoveMembership,

    // Saved Queries
    pub list_saved_queries: saved_queries::ListSavedQueries,
    pub create_saved_query: saved_queries::CreateSavedQuery,
    pub delete_saved_query: saved_queries::DeleteSavedQuery,

    // Telemetry
    pub list_fingerprints: telemetry::ListFingerprints,
    pub upsert_fingerprint: telemetry::UpsertFingerprint,

    // Login Flows
    pub create_login_flow: login_flows::CreateLoginFlow,
    pub get_login_flow: login_flows::GetLoginFlow,
    pub list_login_flows: login_flows::ListLoginFlows,
    pub update_login_flow: login_flows::UpdateLoginFlow,
    pub delete_login_flow: login_flows::DeleteLoginFlow,
    pub promote_login_flow: login_flows::PromoteLoginFlow,
    pub archive_login_flow: login_flows::ArchiveLoginFlow,
    pub resolve_login_flow: login_flows::ResolveLoginFlow,

    // Console
    pub load_console_bootstrap: console::LoadConsoleBootstrap,
    pub load_entity_counts: console::LoadEntityCounts,

    // Resources
    pub create_named_resource: resources::CreateNamedResource,
    pub get_named_resource: resources::GetNamedResource,
    pub list_named_resources: resources::ListNamedResources,
    pub update_named_resource: resources::UpdateNamedResource,
    pub delete_named_resource: resources::DeleteNamedResource,

    // Hook pipeline
    pub hooks: Arc<HookPipeline>,

    // Use case runner (wraps use case calls with hook phases)
    pub runner: UseCaseRunner,
}

impl ApplicationServices {
    /// Build all use cases from a set of repositories and a hook pipeline.
    pub fn new(repos: Arc<Repositories>, hooks: Arc<HookPipeline>) -> Self {
        Self {
            repos: repos.clone(),
            // Users
            create_user: users::CreateUser::new(repos.clone()),
            get_user: users::GetUser::new(repos.clone()),
            list_users: users::ListUsers::new(repos.clone()),
            update_user: users::UpdateUser::new(repos.clone()),
            deactivate_user: users::DeactivateUser::new(repos.clone()),
            delete_user: users::DeleteUser::new(repos.clone()),

            // Credentials
            set_password: credentials::SetPassword::new(repos.clone()),
            verify_password: credentials::VerifyPassword::new(repos.clone()),
            link_identity: credentials::LinkIdentity::new(repos.clone()),

            // Auth
            start_login: auth::StartLogin::new(repos.clone()),
            submit_login_step: auth::SubmitLoginStep::new(repos.clone()),
            issue_session: auth::IssueSession::new(repos.clone()),
            revoke_session: auth::RevokeSession::new(repos.clone()),

            // Sessions
            list_sessions: sessions::ListSessions::new(repos.clone()),
            get_session: sessions::GetSession::new(repos.clone()),

            // Orgs
            create_org: orgs::CreateOrg::new(repos.clone()),
            get_org: orgs::GetOrg::new(repos.clone()),
            list_orgs: orgs::ListOrgs::new(repos.clone()),
            update_org: orgs::UpdateOrg::new(repos.clone()),
            delete_org: orgs::DeleteOrg::new(repos.clone()),

            // Groups
            create_group: groups::CreateGroup::new(repos.clone()),
            get_group: groups::GetGroup::new(repos.clone()),
            list_groups: groups::ListGroups::new(repos.clone()),
            update_group: groups::UpdateGroup::new(repos.clone()),
            delete_group: groups::DeleteGroup::new(repos.clone()),

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
            list_domains: instances::ListDomains::new(repos.clone()),
            add_domain: instances::AddDomain::new(repos.clone()),
            remove_domain: instances::RemoveDomain::new(repos.clone()),

            // Support
            create_support_grant: support::CreateSupportGrant::new(repos.clone()),
            list_support_grants: support::ListSupportGrants::new(repos.clone()),
            revoke_support_grant: support::RevokeSupportGrant::new(repos.clone()),

            // Settings
            get_settings: settings::GetSettings::new(repos.clone()),
            update_settings: settings::UpdateSettings::new(repos.clone()),
            delete_settings: settings::DeleteSettings::new(repos.clone()),

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
            update_schema: schemas::UpdateSchema::new(repos.clone()),
            promote_schema: schemas::PromoteSchema::new(repos.clone()),
            count_schema_users: schemas::CountSchemaUsers::new(repos.clone()),

            // PATs
            create_pat: pats::CreatePat::new(repos.clone()),
            list_pats: pats::ListPats::new(repos.clone()),
            revoke_pat: pats::RevokePat::new(repos.clone()),

            // Actions
            list_actions: actions::ListActions::new(repos.clone()),
            get_action: actions::GetAction::new(repos.clone()),
            create_action: actions::CreateAction::new(repos.clone()),
            update_action: actions::UpdateAction::new(repos.clone()),
            delete_action: actions::DeleteAction::new(repos.clone()),

            // Search
            search_entities: search::SearchEntities::new(repos.clone()),

            // Jobs
            list_jobs: jobs::ListJobs::new(repos.clone()),

            // Memberships
            list_memberships: memberships::ListMemberships::new(repos.clone()),
            add_membership: memberships::AddMembership::new(repos.clone()),
            remove_membership: memberships::RemoveMembership::new(repos.clone()),

            // Saved Queries
            list_saved_queries: saved_queries::ListSavedQueries::new(repos.clone()),
            create_saved_query: saved_queries::CreateSavedQuery::new(repos.clone()),
            delete_saved_query: saved_queries::DeleteSavedQuery::new(repos.clone()),

            // Telemetry
            list_fingerprints: telemetry::ListFingerprints::new(repos.clone()),
            upsert_fingerprint: telemetry::UpsertFingerprint::new(repos.clone()),

            // Login Flows
            create_login_flow: login_flows::CreateLoginFlow::new(repos.clone()),
            get_login_flow: login_flows::GetLoginFlow::new(repos.clone()),
            list_login_flows: login_flows::ListLoginFlows::new(repos.clone()),
            update_login_flow: login_flows::UpdateLoginFlow::new(repos.clone()),
            delete_login_flow: login_flows::DeleteLoginFlow::new(repos.clone()),
            promote_login_flow: login_flows::PromoteLoginFlow::new(repos.clone()),
            archive_login_flow: login_flows::ArchiveLoginFlow::new(repos.clone()),
            resolve_login_flow: login_flows::ResolveLoginFlow::new(repos.clone()),

            // Console
            load_console_bootstrap: console::LoadConsoleBootstrap::new(repos.clone()),
            load_entity_counts: console::LoadEntityCounts::new(repos.clone()),

            // Resources
            create_named_resource: resources::CreateNamedResource::new(repos.clone()),
            get_named_resource: resources::GetNamedResource::new(repos.clone()),
            list_named_resources: resources::ListNamedResources::new(repos.clone()),
            update_named_resource: resources::UpdateNamedResource::new(repos.clone()),
            delete_named_resource: resources::DeleteNamedResource::new(repos.clone()),

            runner: UseCaseRunner::new(
                hooks.pre_validate_interceptors.clone(),
                hooks.pre_commit_interceptors.clone(),
                hooks.post_commit_effects.clone(),
            ),

            hooks,
        }
    }
}
