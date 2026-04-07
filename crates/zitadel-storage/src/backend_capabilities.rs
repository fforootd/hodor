use zitadel_db::BackendKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StorageBackendCapabilities {
    pub default_read_backend: &'static str,
    pub default_kv_backend: &'static str,
    pub default_sink_backend: &'static str,
    pub supports_memory_kv: bool,
    pub supports_postgres_unlogged_kv: bool,
    pub supports_shared_sql_kv: bool,
    pub supports_channel_sink: bool,
    pub supports_postgres_sink: bool,
    pub supports_noop_sink: bool,
}

const SQLITE_CAPABILITIES: StorageBackendCapabilities = StorageBackendCapabilities {
    default_read_backend: "same_connection",
    default_kv_backend: "memory",
    default_sink_backend: "channel",
    supports_memory_kv: true,
    supports_postgres_unlogged_kv: false,
    supports_shared_sql_kv: false,
    supports_channel_sink: true,
    supports_postgres_sink: false,
    supports_noop_sink: true,
};

const POSTGRES_CAPABILITIES: StorageBackendCapabilities = StorageBackendCapabilities {
    default_read_backend: "same_primary",
    default_kv_backend: "postgres_unlogged",
    default_sink_backend: "postgres",
    supports_memory_kv: true,
    supports_postgres_unlogged_kv: true,
    supports_shared_sql_kv: true,
    supports_channel_sink: true,
    supports_postgres_sink: true,
    supports_noop_sink: true,
};

const SPANNER_CAPABILITIES: StorageBackendCapabilities = StorageBackendCapabilities {
    default_read_backend: "same_primary",
    default_kv_backend: "shared_sql",
    default_sink_backend: "noop",
    supports_memory_kv: false,
    supports_postgres_unlogged_kv: false,
    supports_shared_sql_kv: true,
    supports_channel_sink: false,
    supports_postgres_sink: false,
    supports_noop_sink: true,
};

pub const fn storage_backend_capabilities(backend: BackendKind) -> StorageBackendCapabilities {
    match backend {
        BackendKind::Sqlite => SQLITE_CAPABILITIES,
        BackendKind::Postgres => POSTGRES_CAPABILITIES,
        BackendKind::Spanner => SPANNER_CAPABILITIES,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spanner_capabilities_capture_intentional_runtime_differences() {
        let caps = storage_backend_capabilities(BackendKind::Spanner);
        assert_eq!(caps.default_read_backend, "same_primary");
        assert_eq!(caps.default_kv_backend, "shared_sql");
        assert_eq!(caps.default_sink_backend, "noop");
        assert!(!caps.supports_memory_kv);
        assert!(!caps.supports_channel_sink);
    }
}
