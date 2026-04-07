-- +goose Up

CREATE TABLE IF NOT EXISTS actions (
    action_type TEXT NOT NULL DEFAULT 'expr',
    config TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    enabled BOOLEAN NOT NULL DEFAULT 1,
    fail_open BOOLEAN NOT NULL DEFAULT 0,
    hook TEXT NOT NULL DEFAULT 'on_event',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    metadata TEXT DEFAULT '{}',
    name TEXT NOT NULL,
    org_id TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    schema_id TEXT DEFAULT '',
    timeout_ms INTEGER NOT NULL DEFAULT 5000,
    trigger_expr TEXT DEFAULT 'true',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS apps (
    app_type TEXT NOT NULL DEFAULT 'oidc',
    client_id TEXT NOT NULL,
    client_secret TEXT DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    grant_types TEXT DEFAULT '["authorization_code"]',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    metadata TEXT DEFAULT '{}',
    name TEXT NOT NULL,
    org_id TEXT,
    post_logout_redirect_uris TEXT NOT NULL DEFAULT '[]',
    redirect_uris TEXT DEFAULT '[]',
    response_types TEXT DEFAULT '["code"]',
    schema_id TEXT DEFAULT '',
    state TEXT NOT NULL DEFAULT 'active',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS auth_states (
    auth_time TEXT,
    client_id TEXT DEFAULT '',
    code TEXT DEFAULT '',
    code_challenge TEXT DEFAULT '',
    code_challenge_method TEXT DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    data TEXT DEFAULT '{}',
    done BOOLEAN NOT NULL DEFAULT 0,
    expires_at TEXT NOT NULL DEFAULT (datetime('now', '+10 minutes')),
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    nonce TEXT DEFAULT '',
    pkce_verifier TEXT DEFAULT '',
    provider_id TEXT DEFAULT '',
    redirect_uri TEXT DEFAULT '',
    response_type TEXT DEFAULT 'code',
    scopes TEXT DEFAULT '',
    state TEXT DEFAULT '',
    step TEXT DEFAULT '',
    type TEXT NOT NULL,
    user_id TEXT DEFAULT '',
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS cache (
    data TEXT NOT NULL,
    expires_at TEXT,
    fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
    instance_id TEXT NOT NULL,
    key TEXT NOT NULL,
    namespace TEXT NOT NULL DEFAULT 'default',
    PRIMARY KEY (instance_id, namespace, key)
);

CREATE TABLE IF NOT EXISTS consumer_cursors (
    consumer_name TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    last_event_id TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, consumer_name)
);

CREATE TABLE IF NOT EXISTS credentials (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    data TEXT DEFAULT '{}',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    name TEXT DEFAULT '',
    type TEXT NOT NULL,
    user_id TEXT NOT NULL,
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS domains (
    certificate_dns_record_name TEXT NOT NULL DEFAULT '',
    certificate_dns_record_type TEXT NOT NULL DEFAULT '',
    certificate_dns_record_value TEXT NOT NULL DEFAULT '',
    certificate_id TEXT NOT NULL DEFAULT '',
    certificate_map_entry TEXT NOT NULL DEFAULT '',
    certificate_state TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    dns_authorization_id TEXT NOT NULL DEFAULT '',
    dns_challenge_host TEXT NOT NULL DEFAULT '',
    domain TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT 0,
    org_id TEXT,
    origin_trust_state TEXT NOT NULL DEFAULT '',
    provisioning_error TEXT NOT NULL DEFAULT '',
    purpose TEXT NOT NULL DEFAULT 'served',
    state TEXT NOT NULL DEFAULT 'active',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    verification_token TEXT NOT NULL DEFAULT '',
    verified BOOLEAN NOT NULL DEFAULT 0,
    PRIMARY KEY (domain),
    FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS effects (
    attempt INTEGER NOT NULL DEFAULT 0,
    completed_at TEXT,
    config TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    effect_type TEXT NOT NULL,
    event_id TEXT NOT NULL DEFAULT '',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    last_error TEXT NOT NULL DEFAULT '',
    lease_expires_at TEXT,
    lease_owner TEXT NOT NULL DEFAULT '',
    max_attempts INTEGER NOT NULL DEFAULT 5,
    next_retry_at TEXT NOT NULL DEFAULT (datetime('now')),
    payload TEXT NOT NULL DEFAULT '{}',
    source_key TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS events (
    actor_id TEXT,
    actor_type TEXT,
    aggregate_id TEXT,
    aggregate_type TEXT,
    category TEXT NOT NULL DEFAULT '',
    client_id TEXT DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    delegation_type TEXT DEFAULT '',
    event_type TEXT NOT NULL,
    fingerprint TEXT DEFAULT '',
    flow_id TEXT,
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    metadata TEXT DEFAULT '{}',
    org_id TEXT NOT NULL DEFAULT '0',
    payload TEXT DEFAULT '{}',
    request_id TEXT,
    resource_type TEXT,
    sdk_name TEXT DEFAULT '',
    sdk_version TEXT DEFAULT '',
    sequence INTEGER,
    session_id TEXT,
    shipped_at TEXT,
    token_id TEXT DEFAULT '',
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS fga_authorization_models (
    compiled_model TEXT NOT NULL,
    core_model_version TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    custom_model TEXT NOT NULL DEFAULT '{}',
    is_active INTEGER NOT NULL DEFAULT 0,
    model_id TEXT NOT NULL,
    module_fragments TEXT NOT NULL DEFAULT '[]',
    schema_version TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    store_id TEXT NOT NULL,
    PRIMARY KEY (scope_id, store_id, model_id)
);

CREATE TABLE IF NOT EXISTS fga_stores (
    scope_id TEXT NOT NULL,
    store_id TEXT NOT NULL,
    PRIMARY KEY (scope_id)
);

CREATE TABLE IF NOT EXISTS fga_tuple_changes (
    authorization_model_id TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    object_id TEXT NOT NULL,
    object_type TEXT NOT NULL,
    operation TEXT NOT NULL,
    raw_object TEXT NOT NULL,
    raw_user TEXT NOT NULL,
    relation TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    store_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    user_relation TEXT NOT NULL DEFAULT '',
    user_type TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS fga_tuples (
    inserted_at TEXT NOT NULL DEFAULT (datetime('now')),
    object_id TEXT NOT NULL,
    object_type TEXT NOT NULL,
    raw_object TEXT NOT NULL,
    raw_user TEXT NOT NULL,
    relation TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    store_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    user_relation TEXT NOT NULL DEFAULT '',
    user_type TEXT NOT NULL,
    PRIMARY KEY (scope_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation)
);

CREATE TABLE IF NOT EXISTS fingerprints (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    raw_data TEXT NOT NULL,
    type TEXT NOT NULL,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS groups (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    description TEXT DEFAULT '',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    metadata TEXT DEFAULT '{}',
    name TEXT NOT NULL,
    org_id TEXT,
    schema_id TEXT DEFAULT '',
    state TEXT NOT NULL DEFAULT 'active',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS instance_trust_links (
    allowed_scopes TEXT NOT NULL DEFAULT '[]',
    audience TEXT NOT NULL,
    child_instance_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    issuer TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'active',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (child_instance_id, issuer, audience),
    FOREIGN KEY (child_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS instances (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    feature_overrides TEXT NOT NULL DEFAULT '{}',
    instance_id TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'managed',
    last_heartbeat_at TEXT,
    last_heartbeat_status TEXT NOT NULL DEFAULT '',
    owner_org_id TEXT,
    parent_instance_id TEXT,
    placement_mode TEXT NOT NULL DEFAULT 'global',
    region_key TEXT,
    registration_token_hash TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT 'active',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id),
    CHECK ((parent_instance_id IS NULL AND owner_org_id IS NULL AND kind = 'root') OR (parent_instance_id IS NOT NULL AND owner_org_id IS NOT NULL AND kind IN ('managed', 'federated'))),
    CHECK (kind IN ('root', 'managed', 'federated')),
    CHECK (placement_mode IN ('global', 'regional')),
    FOREIGN KEY (parent_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE,
    FOREIGN KEY (parent_instance_id, owner_org_id) REFERENCES orgs(instance_id, id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS jobs (
    config_json TEXT DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    cron TEXT NOT NULL,
    description TEXT DEFAULT '',
    display_name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    instance_id TEXT NOT NULL,
    last_error TEXT NOT NULL DEFAULT '',
    last_rows_removed INTEGER NOT NULL DEFAULT 0,
    last_run_at TEXT,
    last_status TEXT NOT NULL DEFAULT 'idle',
    lease_expires_at TEXT,
    lease_owner TEXT NOT NULL DEFAULT '',
    name TEXT NOT NULL,
    next_run_at TEXT,
    run_count INTEGER NOT NULL DEFAULT 0,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, name)
);

CREATE TABLE IF NOT EXISTS linked_identities (
    external_email TEXT DEFAULT '',
    external_sub TEXT NOT NULL,
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    last_used_at TEXT,
    linked_at TEXT NOT NULL DEFAULT (datetime('now')),
    provider_id TEXT NOT NULL,
    raw_claims TEXT DEFAULT '{}',
    user_id TEXT NOT NULL,
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, provider_id) REFERENCES providers(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS login_flow_assets (
    content_type TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    data TEXT NOT NULL,
    etag TEXT NOT NULL,
    filename TEXT NOT NULL DEFAULT '',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    login_flow_id TEXT NOT NULL,
    metadata TEXT DEFAULT '{}',
    org_id TEXT,
    sha256 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL DEFAULT 0,
    slot TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, login_flow_id) REFERENCES login_flows(instance_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS login_flows (
    audience TEXT NOT NULL DEFAULT '{}',
    auth_methods TEXT NOT NULL DEFAULT '{}',
    config TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    enabled BOOLEAN NOT NULL DEFAULT 1,
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT 0,
    metadata TEXT DEFAULT '{}',
    name TEXT NOT NULL,
    org_id TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    schema_id TEXT DEFAULT '',
    state TEXT NOT NULL DEFAULT 'draft',
    steps TEXT NOT NULL DEFAULT '[]',
    strategy TEXT NOT NULL DEFAULT 'identifier_first',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS memberships (
    added_at TEXT NOT NULL DEFAULT (datetime('now')),
    instance_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    role TEXT NOT NULL DEFAULT 'member',
    user_id TEXT NOT NULL,
    PRIMARY KEY (instance_id, resource_type, resource_id, user_id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS oidc_auth_requests (
    auth_time TEXT,
    client_id TEXT NOT NULL,
    code TEXT NOT NULL DEFAULT '',
    code_challenge TEXT NOT NULL DEFAULT '',
    code_challenge_method TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    done BOOLEAN NOT NULL DEFAULT 0,
    expires_at TEXT NOT NULL DEFAULT (datetime('now', '+10 minutes')),
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    login_hint TEXT NOT NULL DEFAULT '',
    max_age INTEGER,
    nonce TEXT NOT NULL DEFAULT '',
    prompt TEXT NOT NULL DEFAULT '[]',
    redirect_uri TEXT NOT NULL DEFAULT '',
    response_type TEXT NOT NULL DEFAULT 'code',
    scope TEXT NOT NULL DEFAULT '',
    session_id TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL DEFAULT '',
    user_id TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS oidc_rp_auth_states (
    callback_uri TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expected_issuer TEXT NOT NULL DEFAULT '',
    expires_at TEXT NOT NULL DEFAULT (datetime('now', '+10 minutes')),
    flow_id TEXT NOT NULL DEFAULT '',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    nonce TEXT NOT NULL DEFAULT '',
    pkce_verifier TEXT NOT NULL DEFAULT '',
    provider_id TEXT NOT NULL DEFAULT '',
    redirect_uri TEXT NOT NULL DEFAULT '',
    state TEXT NOT NULL,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS orgs (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    metadata TEXT DEFAULT '{}',
    name TEXT NOT NULL,
    schema_id TEXT DEFAULT '',
    state TEXT NOT NULL DEFAULT 'active',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS projects (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    description TEXT DEFAULT '',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    metadata TEXT DEFAULT '{}',
    name TEXT NOT NULL,
    org_id TEXT,
    schema_id TEXT DEFAULT '',
    state TEXT NOT NULL DEFAULT 'active',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS providers (
    catalog_ref TEXT NOT NULL DEFAULT '{}',
    connection TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    display_name TEXT NOT NULL,
    display_order INTEGER NOT NULL DEFAULT 0,
    enabled BOOLEAN NOT NULL DEFAULT 1,
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'custom',
    linking TEXT NOT NULL DEFAULT '{}',
    mapping TEXT NOT NULL DEFAULT '{}',
    org_id TEXT,
    protocol TEXT NOT NULL DEFAULT 'oidc',
    session TEXT NOT NULL DEFAULT '{}',
    target TEXT NOT NULL DEFAULT '{}',
    ui TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS retention_policies (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    event_pattern TEXT NOT NULL,
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    lake_ttl TEXT NOT NULL,
    oltp_ttl TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS role_assignments (
    approved_by TEXT,
    assignment_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    enforcement_instance_id TEXT NOT NULL,
    expires_at TEXT,
    origin_instance_id TEXT,
    principal_ref TEXT NOT NULL,
    reason TEXT,
    revoked_at TEXT,
    role_key TEXT NOT NULL,
    scope_id TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    source_kind TEXT NOT NULL DEFAULT 'manual',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (assignment_id),
    FOREIGN KEY (enforcement_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE,
    FOREIGN KEY (origin_instance_id) REFERENCES instances(instance_id) ON DELETE SET NULL,
    FOREIGN KEY (role_key) REFERENCES role_definitions(role_key) ON DELETE NO ACTION
);

CREATE TABLE IF NOT EXISTS role_definitions (
    builtin BOOLEAN NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    permissions_json TEXT NOT NULL DEFAULT '[]',
    relation_name TEXT NOT NULL,
    role_key TEXT NOT NULL,
    scope_kind TEXT NOT NULL,
    source_version TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (role_key)
);

CREATE TABLE IF NOT EXISTS saved_queries (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_by TEXT DEFAULT '',
    description TEXT DEFAULT '',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    name TEXT NOT NULL,
    sql_text TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS schemas (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_by TEXT DEFAULT '',
    id TEXT NOT NULL,
    is_default BOOLEAN NOT NULL DEFAULT 0,
    message TEXT DEFAULT '',
    schema TEXT NOT NULL,
    type TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    visibility TEXT NOT NULL DEFAULT 'private',
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS secrets (
    algorithm TEXT NOT NULL DEFAULT 'RS256',
    ciphertext BLOB NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    encryption_key_id TEXT DEFAULT '',
    expires_at TEXT,
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    nonce BLOB,
    public_key BLOB,
    secret_type TEXT NOT NULL,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS sessions (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    fingerprint TEXT DEFAULT '',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    ip_address TEXT DEFAULT '',
    last_active_at TEXT NOT NULL DEFAULT (datetime('now')),
    metadata TEXT DEFAULT '{}',
    org_id TEXT,
    revoked_at TEXT,
    token_hash TEXT NOT NULL DEFAULT '',
    user_agent TEXT DEFAULT '',
    user_id TEXT NOT NULL,
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS settings (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    data TEXT NOT NULL DEFAULT '{}',
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    scope TEXT NOT NULL DEFAULT 'instance',
    scope_id TEXT NOT NULL DEFAULT '',
    type TEXT NOT NULL,
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS tokens (
    application_id TEXT DEFAULT '',
    audience TEXT DEFAULT '',
    auth_method TEXT DEFAULT '',
    auth_time TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at TEXT,
    id TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    last_used TEXT,
    name TEXT DEFAULT '',
    refresh_token_id TEXT DEFAULT '',
    revoked_at TEXT,
    scopes TEXT NOT NULL DEFAULT '[]',
    session_id TEXT,
    token_hash TEXT NOT NULL,
    type TEXT NOT NULL,
    user_id TEXT,
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, session_id) REFERENCES sessions(instance_id, id) ON DELETE SET NULL,
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS unique_fields (
    field_name TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    normalized_value TEXT NOT NULL,
    resource_type TEXT NOT NULL DEFAULT '',
    scope_id TEXT NOT NULL DEFAULT '',
    user_id TEXT NOT NULL,
    PRIMARY KEY (instance_id, scope_id, field_name, normalized_value),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS users (
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    display_name TEXT DEFAULT '',
    id TEXT NOT NULL,
    identifier TEXT NOT NULL,
    instance_id TEXT NOT NULL,
    metadata TEXT DEFAULT '{}',
    org_id TEXT,
    schema_id TEXT DEFAULT '',
    state TEXT NOT NULL DEFAULT 'active',
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    user_type TEXT NOT NULL DEFAULT 'human',
    PRIMARY KEY (instance_id, id)
);

CREATE INDEX IF NOT EXISTS idx_actions_instance_hook ON actions(instance_id, hook, enabled);

CREATE INDEX IF NOT EXISTS idx_actions_instance_org ON actions(instance_id, org_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_apps_7cc14b94fcd29467 ON apps(instance_id, client_id);

CREATE INDEX IF NOT EXISTS idx_apps_instance_client ON apps(instance_id, client_id);

CREATE INDEX IF NOT EXISTS idx_apps_instance_org ON apps(instance_id, org_id);

CREATE INDEX IF NOT EXISTS idx_auth_states_instance_code ON auth_states(instance_id, code) WHERE code != '';

CREATE INDEX IF NOT EXISTS idx_auth_states_instance_expires ON auth_states(instance_id, expires_at);

CREATE INDEX IF NOT EXISTS idx_auth_states_instance_state ON auth_states(instance_id, state) WHERE state != '';

CREATE INDEX IF NOT EXISTS idx_auth_states_instance_type ON auth_states(instance_id, type);

CREATE INDEX IF NOT EXISTS idx_cache_instance_expires ON cache(instance_id, expires_at) WHERE expires_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_credentials_instance_type ON credentials(instance_id, user_id, type);

CREATE INDEX IF NOT EXISTS idx_credentials_instance_user ON credentials(instance_id, user_id);

CREATE INDEX IF NOT EXISTS idx_domains_instance ON domains(instance_id);

CREATE INDEX IF NOT EXISTS idx_domains_instance_org ON domains(instance_id, org_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_domains_instance_primary ON domains(instance_id) WHERE org_id IS NULL AND is_primary = TRUE;

CREATE UNIQUE INDEX IF NOT EXISTS idx_domains_org_primary ON domains(instance_id, org_id) WHERE org_id IS NOT NULL AND is_primary = TRUE;

CREATE INDEX IF NOT EXISTS idx_effects_cleanup ON effects(instance_id, status, completed_at);

CREATE INDEX IF NOT EXISTS idx_effects_due ON effects(instance_id, status, next_retry_at, lease_expires_at);

CREATE INDEX IF NOT EXISTS idx_effects_event ON effects(instance_id, event_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_effects_source_key ON effects(instance_id, source_key);

CREATE INDEX IF NOT EXISTS idx_events_actor ON events(instance_id, actor_id) WHERE actor_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(instance_id, aggregate_id, aggregate_type);

CREATE INDEX IF NOT EXISTS idx_events_category ON events(instance_id, category, created_at);

CREATE INDEX IF NOT EXISTS idx_events_client ON events(instance_id, client_id) WHERE client_id != '';

CREATE INDEX IF NOT EXISTS idx_events_delegation ON events(instance_id, delegation_type) WHERE delegation_type != '';

CREATE INDEX IF NOT EXISTS idx_events_flow ON events(instance_id, flow_id) WHERE flow_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_events_instance_created ON events(instance_id, created_at);

CREATE INDEX IF NOT EXISTS idx_events_instance_type_created ON events(instance_id, event_type, created_at);

CREATE INDEX IF NOT EXISTS idx_events_org ON events(instance_id, org_id, created_at);

CREATE INDEX IF NOT EXISTS idx_events_request ON events(instance_id, request_id) WHERE request_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_events_ship ON events(instance_id, shipped_at) WHERE shipped_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_fga_models_active ON fga_authorization_models(scope_id, store_id, is_active, created_at);

CREATE UNIQUE INDEX IF NOT EXISTS uk_fga_stores_a51aeb6f7be48385 ON fga_stores(store_id);

CREATE INDEX IF NOT EXISTS idx_fga_tuple_changes_lookup ON fga_tuple_changes(scope_id, store_id, seq);

CREATE INDEX IF NOT EXISTS idx_fga_tuples_lookup ON fga_tuples(scope_id, store_id, object_type, object_id, relation);

CREATE INDEX IF NOT EXISTS idx_fga_tuples_user ON fga_tuples(scope_id, store_id, user_type, user_id, user_relation);

CREATE INDEX IF NOT EXISTS idx_fingerprints_instance_type ON fingerprints(instance_id, type);

CREATE UNIQUE INDEX IF NOT EXISTS uk_groups_4f9e372ac213e76f ON groups(instance_id, org_id, name);

CREATE UNIQUE INDEX IF NOT EXISTS idx_groups_instance_name_no_org ON groups(instance_id, name) WHERE org_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_groups_instance_org ON groups(instance_id, org_id);

CREATE INDEX IF NOT EXISTS idx_jobs_instance_due_lease ON jobs(instance_id, enabled, next_run_at, lease_expires_at);

CREATE UNIQUE INDEX IF NOT EXISTS uk_linked_identities_9dc9bb1751f5e05b ON linked_identities(instance_id, provider_id, external_sub);

CREATE INDEX IF NOT EXISTS idx_linked_identities_instance_user ON linked_identities(instance_id, user_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_login_flow_assets_fe47ca5fd93c1632 ON login_flow_assets(instance_id, login_flow_id, slot);

CREATE INDEX IF NOT EXISTS idx_login_flow_assets_instance_flow ON login_flow_assets(instance_id, login_flow_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_login_flows_instance_default ON login_flows(instance_id) WHERE is_default = TRUE;

CREATE INDEX IF NOT EXISTS idx_login_flows_instance_org ON login_flows(instance_id, org_id);

CREATE INDEX IF NOT EXISTS idx_login_flows_instance_state ON login_flows(instance_id, state, enabled);

CREATE INDEX IF NOT EXISTS idx_memberships_instance_resource ON memberships(instance_id, resource_type, resource_id);

CREATE INDEX IF NOT EXISTS idx_memberships_instance_user ON memberships(instance_id, user_id, resource_type);

CREATE UNIQUE INDEX IF NOT EXISTS idx_oidc_auth_requests_code ON oidc_auth_requests(instance_id, code) WHERE code != '';

CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_client ON oidc_auth_requests(instance_id, client_id);

CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_created ON oidc_auth_requests(instance_id, created_at);

CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_expires ON oidc_auth_requests(instance_id, expires_at);

CREATE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_instance_expires ON oidc_rp_auth_states(instance_id, expires_at);

CREATE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_provider ON oidc_rp_auth_states(instance_id, provider_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_state ON oidc_rp_auth_states(instance_id, state);

CREATE UNIQUE INDEX IF NOT EXISTS uk_orgs_1f9e20efebadb052 ON orgs(instance_id, name);

CREATE INDEX IF NOT EXISTS idx_orgs_instance_state ON orgs(instance_id, state);

CREATE UNIQUE INDEX IF NOT EXISTS uk_projects_1da32ee6b9e06ee5 ON projects(instance_id, org_id, name);

CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_instance_name_no_org ON projects(instance_id, name) WHERE org_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_projects_instance_org ON projects(instance_id, org_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_providers_f22536697181dc2e ON providers(instance_id, org_id, display_name);

CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_instance_name_no_org ON providers(instance_id, display_name) WHERE org_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_providers_instance_org ON providers(instance_id, org_id);

CREATE INDEX IF NOT EXISTS idx_providers_instance_protocol ON providers(instance_id, protocol, enabled);

CREATE INDEX IF NOT EXISTS idx_providers_instance_sort ON providers(instance_id, display_order, display_name);

CREATE INDEX IF NOT EXISTS idx_retention_policies_instance_priority ON retention_policies(instance_id, priority);

CREATE INDEX IF NOT EXISTS idx_role_assignments_instance_scope ON role_assignments(enforcement_instance_id, scope_kind, scope_id);

CREATE INDEX IF NOT EXISTS idx_role_assignments_principal ON role_assignments(principal_ref, revoked_at, expires_at);

CREATE INDEX IF NOT EXISTS idx_role_assignments_role_source ON role_assignments(role_key, source_kind);

CREATE INDEX IF NOT EXISTS idx_saved_queries_instance_name ON saved_queries(instance_id, name);

CREATE INDEX IF NOT EXISTS idx_schema_default ON schemas(type, is_default);

CREATE INDEX IF NOT EXISTS idx_schema_type ON schemas(type);

CREATE INDEX IF NOT EXISTS idx_schema_version ON schemas(type, version);

CREATE INDEX IF NOT EXISTS idx_secrets_instance_enc_key ON secrets(instance_id, encryption_key_id);

CREATE INDEX IF NOT EXISTS idx_secrets_instance_type ON secrets(instance_id, secret_type);

CREATE INDEX IF NOT EXISTS idx_sessions_instance_expires ON sessions(instance_id, expires_at) WHERE expires_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_sessions_instance_revoked ON sessions(instance_id, revoked_at) WHERE revoked_at IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_instance_token_unique ON sessions(instance_id, token_hash) WHERE token_hash != '';

CREATE INDEX IF NOT EXISTS idx_sessions_instance_user ON sessions(instance_id, user_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_settings_b80d13a3fd9118ff ON settings(instance_id, type, scope, scope_id);

CREATE INDEX IF NOT EXISTS idx_settings_instance_type ON settings(instance_id, type, scope);

CREATE UNIQUE INDEX IF NOT EXISTS uk_tokens_4c7bc010dabe07b4 ON tokens(instance_id, token_hash);

CREATE INDEX IF NOT EXISTS idx_tokens_instance_app ON tokens(instance_id, application_id) WHERE application_id != '';

CREATE INDEX IF NOT EXISTS idx_tokens_instance_expires ON tokens(instance_id, expires_at) WHERE expires_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_tokens_instance_revoked ON tokens(instance_id, revoked_at) WHERE revoked_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_tokens_instance_session ON tokens(instance_id, session_id);

CREATE INDEX IF NOT EXISTS idx_tokens_instance_type ON tokens(instance_id, type, user_id);

CREATE INDEX IF NOT EXISTS idx_tokens_instance_user ON tokens(instance_id, user_id);

CREATE INDEX IF NOT EXISTS idx_unique_fields_instance_lookup ON unique_fields(instance_id, normalized_value, field_name);

CREATE INDEX IF NOT EXISTS idx_unique_fields_instance_resource ON unique_fields(instance_id, user_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_users_8fa36ca7c7768f49 ON users(instance_id, org_id, identifier);

CREATE UNIQUE INDEX IF NOT EXISTS idx_users_instance_identifier_no_org ON users(instance_id, identifier) WHERE org_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_users_instance_org ON users(instance_id, org_id);

CREATE INDEX IF NOT EXISTS idx_users_instance_state ON users(instance_id, state);

CREATE INDEX IF NOT EXISTS idx_users_instance_type ON users(instance_id, user_type);
