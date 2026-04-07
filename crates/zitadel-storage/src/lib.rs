#![allow(async_fn_in_trait)]
#![allow(clippy::large_enum_variant)]

pub mod analytics;
pub mod backend_capabilities;
pub mod runtime;
pub mod stateful;
pub mod transient;

pub use analytics::{
    AnalyticsQuery, AnalyticsQueryBackend, AnalyticsQueryResult, AnalyticsSink, AnalyticsStorage,
    DefaultAnalyticsStorage, NoopAnalyticsSink, SpannerAnalyticsQueryBackend,
    SqlAnalyticsQueryBackend,
};
pub use backend_capabilities::{StorageBackendCapabilities, storage_backend_capabilities};
pub use runtime::{StorageRoleSummary, StorageRuntime, prepare_postgres_role_databases};
pub use stateful::{
    DefaultStatefulStorage, ReadStore, ResolvedPatIdentity, SpannerReadStore, SpannerStatefulStore,
    SqlReadStore, SqlStatefulStore, StatefulStorage, StatefulStore, UserIdentity,
};
pub use transient::{
    AuthRequestRedirect, AuthRequestRequirements, ChannelSink, CreatedSession, DefaultKvStore,
    DefaultSink, DefaultTransientStorage, KvStore, LoginFlowRuntimeState, MemoryKvStore,
    NewLoginFlowState, NoopSink, PersistedSessionRecord, ProviderAuthState, SessionRecord, Sink,
    SpannerKvStore, SqlKvStore, SqlSink, TransientRecord, TransientStorage,
    prepare_postgres_kv_schema, prepare_postgres_sink_schema,
};
