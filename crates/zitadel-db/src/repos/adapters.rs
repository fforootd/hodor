//! Supported adapter surface for production repository implementations.
//!
//! Production code should import repository implementations from this module so
//! `zitadel_db::repos` remains the single supported database-facing boundary.

pub use crate::repos::auth_impl::{
    DbActionRepository, DbCredentialRepository, DbEventRepository, DbFgaRepository,
    DbLoginFlowRepository, DbOidcKeyRepository, DbOidcRepository, DbOidcTokenRepository,
    DbPatRepository, DbSessionRepository, SqlUnitOfWorkFactory,
};
pub use crate::repos::authorization_impl::DbAuthorizationRepository;
pub use crate::repos::auxiliary_named_impl::{DbAppRepository, DbProjectRepository};
pub use crate::repos::auxiliary_runtime_impl::{
    DbConsoleQueryRepository, DbJobRepository, DbMembershipRepository, DbSavedQueryRepository,
    DbTelemetryRepository,
};
pub use crate::repos::effects_impl::DbEffectRepository;
pub use crate::repos::entities_impl::{
    SqlGroupRepository, SqlInstanceRepository, SqlOrgRepository, SqlProviderRepository,
    SqlSchemaRepository, SqlSettingsRepository, SqlUserRepository,
};
pub use crate::repos::search_impl::SqlSearchRepository;
