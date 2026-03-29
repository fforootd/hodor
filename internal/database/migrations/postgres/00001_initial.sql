-- +goose Up
-- Zitadel baseline schema — PostgreSQL (ADR-022: dedicated resource tables, consolidated)
-- 22 tables total. See docs/adr/022-dedicated-resource-tables.md

-- ============================================================================
-- INSTANCES
-- ============================================================================
CREATE TABLE IF NOT EXISTS instances (
    id         TEXT PRIMARY KEY,
    name       TEXT NOT NULL,
    state      TEXT NOT NULL DEFAULT 'active',
    settings   JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- ORGS
-- ============================================================================
CREATE TABLE IF NOT EXISTS orgs (
    id          TEXT PRIMARY KEY,
    instance_id TEXT NOT NULL REFERENCES instances(id),
    name        TEXT NOT NULL,
    state       TEXT NOT NULL DEFAULT 'active',
    schema_id   TEXT DEFAULT '',
    metadata    JSONB DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_orgs_instance ON orgs(instance_id);

-- ============================================================================
-- SCHEMAS — registry for validation, UI generation, engine bindings
-- ============================================================================
CREATE TABLE IF NOT EXISTS schemas (
    id         TEXT PRIMARY KEY,
    type       TEXT NOT NULL,
    org_id     TEXT NOT NULL DEFAULT '1',
    schema     JSONB NOT NULL,
    version    INTEGER DEFAULT 1,
    is_default BOOLEAN DEFAULT FALSE,
    visibility TEXT NOT NULL DEFAULT 'private',
    message    TEXT DEFAULT '',
    created_by TEXT DEFAULT '',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_schema_type_org ON schemas(type, org_id);
CREATE INDEX IF NOT EXISTS idx_schema_default ON schemas(type, org_id, is_default);
CREATE INDEX IF NOT EXISTS idx_schema_version ON schemas(type, org_id, version);

-- ============================================================================
-- USERS — all identity types (human, service, machine)
-- ============================================================================
CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY,
    org_id        TEXT NOT NULL DEFAULT '1',
    identifier    TEXT NOT NULL,
    display_name  TEXT DEFAULT '',
    user_type     TEXT NOT NULL DEFAULT 'human',
    state         TEXT NOT NULL DEFAULT 'active',
    schema_id     TEXT DEFAULT '',
    metadata      JSONB DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(org_id, identifier)
);
CREATE INDEX IF NOT EXISTS idx_users_org ON users(org_id);
CREATE INDEX IF NOT EXISTS idx_users_type ON users(user_type);
CREATE INDEX IF NOT EXISTS idx_users_state ON users(state);

-- ============================================================================
-- CREDENTIALS — typed credentials (password, passkey, otp, jwt_profile, etc.)
-- ============================================================================
CREATE TABLE IF NOT EXISTS credentials (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    type       TEXT NOT NULL,         -- 'password', 'passkey', 'otp', 'jwt_profile'
    data       JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_creds_user ON credentials(user_id);

-- ============================================================================
-- PROVIDERS — SSO / IdP configurations
-- ============================================================================
CREATE TABLE IF NOT EXISTS providers (
    id              TEXT PRIMARY KEY,
    org_id          TEXT NOT NULL DEFAULT '1',
    name            TEXT NOT NULL,
    protocol        TEXT NOT NULL DEFAULT 'oidc',
    template        TEXT NOT NULL DEFAULT 'custom',
    config          JSONB NOT NULL DEFAULT '{}',
    claim_overrides JSONB NOT NULL DEFAULT '{}',
    auto_register   BOOLEAN NOT NULL DEFAULT TRUE,
    enabled         BOOLEAN NOT NULL DEFAULT TRUE,
    display_order   INTEGER NOT NULL DEFAULT 0,
    schema_id       TEXT DEFAULT '',
    metadata        JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(org_id, name)
);
CREATE INDEX IF NOT EXISTS idx_providers_org ON providers(org_id);

-- ============================================================================
-- APPS — OIDC/API client applications
-- ============================================================================
CREATE TABLE IF NOT EXISTS apps (
    id             TEXT PRIMARY KEY,
    org_id         TEXT NOT NULL DEFAULT '1',
    name           TEXT NOT NULL,
    app_type       TEXT NOT NULL DEFAULT 'oidc',
    client_id      TEXT NOT NULL UNIQUE,
    client_secret  TEXT DEFAULT '',
    redirect_uris  JSONB DEFAULT '[]',
    grant_types    JSONB DEFAULT '["authorization_code"]',
    response_types JSONB DEFAULT '["code"]',
    state          TEXT NOT NULL DEFAULT 'active',
    schema_id      TEXT DEFAULT '',
    metadata       JSONB DEFAULT '{}',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_apps_org ON apps(org_id);

-- ============================================================================
-- ACTIONS — event-driven hooks and automations
-- ============================================================================
CREATE TABLE IF NOT EXISTS actions (
    id          TEXT PRIMARY KEY,
    org_id      TEXT NOT NULL DEFAULT '1',
    name        TEXT NOT NULL,
    hook        TEXT NOT NULL DEFAULT 'on_event',
    action_type TEXT NOT NULL DEFAULT 'expr',
    trigger     TEXT DEFAULT '',
    config      JSONB NOT NULL DEFAULT '{}',
    priority    INTEGER DEFAULT 0,
    enabled     BOOLEAN DEFAULT TRUE,
    schema_id   TEXT DEFAULT '',
    metadata    JSONB DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_actions_org ON actions(org_id);
CREATE INDEX IF NOT EXISTS idx_actions_hook ON actions(hook, enabled);

-- ============================================================================
-- LOGIN FLOWS — composable login experience definitions (state machine config)
-- ============================================================================
CREATE TABLE IF NOT EXISTS login_flows (
    id           TEXT PRIMARY KEY,
    org_id       TEXT,
    name         TEXT NOT NULL,
    preset       TEXT DEFAULT 'identifier_first',
    steps        JSONB NOT NULL DEFAULT '[]',
    config       JSONB NOT NULL DEFAULT '{}',
    is_default   BOOLEAN DEFAULT FALSE,
    enabled      BOOLEAN DEFAULT TRUE,
    state        TEXT NOT NULL DEFAULT 'draft',
    priority     INTEGER DEFAULT 0,
    audience     JSONB NOT NULL DEFAULT '{}',
    auth_methods JSONB NOT NULL DEFAULT '{}',
    schema_id    TEXT DEFAULT '',
    metadata     JSONB DEFAULT '{}',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_lf_org ON login_flows(org_id);
CREATE INDEX IF NOT EXISTS idx_lf_state ON login_flows(state, enabled);

-- ============================================================================
-- LINKED IDENTITIES — external IdP account links
-- ============================================================================
CREATE TABLE IF NOT EXISTS linked_identities (
    id             TEXT PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    provider_id    TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
    external_sub   TEXT NOT NULL,
    external_email TEXT DEFAULT '',
    raw_claims     JSONB DEFAULT '{}',
    linked_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at   TIMESTAMPTZ,
    UNIQUE(provider_id, external_sub)
);
CREATE INDEX IF NOT EXISTS idx_linked_user ON linked_identities(user_id);
CREATE INDEX IF NOT EXISTS idx_linked_provider ON linked_identities(provider_id, external_sub);

-- ============================================================================
-- SESSIONS
-- ============================================================================
CREATE TABLE IF NOT EXISTS sessions (
    id             TEXT PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    org_id         TEXT NOT NULL DEFAULT '1',
    token_hash     TEXT NOT NULL DEFAULT '',
    user_agent     TEXT DEFAULT '',
    ip_address     TEXT DEFAULT '',
    metadata       JSONB DEFAULT '{}',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at     TIMESTAMPTZ,
    revoked_at     TIMESTAMPTZ,
    fingerprint    TEXT DEFAULT ''
);
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash);

-- ============================================================================
-- TOKENS — all token types: PAT, session, magic_link, oidc_access, oidc_refresh
-- ============================================================================
CREATE TABLE IF NOT EXISTS tokens (
    id         TEXT PRIMARY KEY,
    type       TEXT NOT NULL,       -- 'pat', 'session', 'magic_link', 'oidc_access', 'oidc_refresh'
    token_hash TEXT NOT NULL,
    user_id    TEXT REFERENCES users(id) ON DELETE CASCADE,
    session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    client_id  TEXT DEFAULT '',
    name       TEXT DEFAULT '',
    scopes     JSONB DEFAULT '[]',
    audience   TEXT DEFAULT '',
    subject    TEXT DEFAULT '',
    amr        TEXT DEFAULT '',
    auth_time  TIMESTAMPTZ,
    expires_at TIMESTAMPTZ,
    last_used  TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_tokens_hash ON tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_tokens_user ON tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_tokens_session ON tokens(session_id);
CREATE INDEX IF NOT EXISTS idx_tokens_type ON tokens(type);

-- ============================================================================
-- AUTH STATES — transient authentication flow state (SSO, OIDC, magic links)
-- ============================================================================
CREATE TABLE IF NOT EXISTS auth_states (
    id                    TEXT PRIMARY KEY,
    type                  TEXT NOT NULL,     -- 'sso', 'oidc_auth', 'magic_link', 'registration'
    state                 TEXT DEFAULT '',
    user_id               TEXT DEFAULT '',
    client_id             TEXT DEFAULT '',
    redirect_uri          TEXT DEFAULT '',
    scopes                TEXT DEFAULT '',
    nonce                 TEXT DEFAULT '',
    response_type         TEXT DEFAULT 'code',
    code_challenge        TEXT DEFAULT '',
    code_challenge_method TEXT DEFAULT '',
    pkce_verifier         TEXT DEFAULT '',
    provider_id           TEXT DEFAULT '',
    code                  TEXT DEFAULT '',
    step                  TEXT DEFAULT '',
    done                  BOOLEAN DEFAULT FALSE,
    auth_time             TIMESTAMPTZ,
    expires_at            TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_auth_states_type ON auth_states(type);
CREATE INDEX IF NOT EXISTS idx_auth_states_code ON auth_states(code) WHERE code != '';
CREATE INDEX IF NOT EXISTS idx_auth_states_state ON auth_states(state) WHERE state != '';

-- ============================================================================
-- KEYS — cryptographic keys (OIDC signing, etc.)
-- ============================================================================
CREATE TABLE IF NOT EXISTS keys (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL DEFAULT 'oidc_signing',
    algorithm   TEXT NOT NULL DEFAULT 'RS256',
    private_key BYTEA NOT NULL,
    expires_at  TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_keys_type ON keys(type);

-- ============================================================================
-- EVENTS — audit log / event stream
-- ============================================================================
CREATE TABLE IF NOT EXISTS events (
    id             TEXT PRIMARY KEY,
    event_type     TEXT NOT NULL,
    category       TEXT NOT NULL DEFAULT '',
    org_id         TEXT NOT NULL DEFAULT '0',
    actor_id       TEXT,
    actor_type     TEXT,
    aggregate_id   TEXT,
    aggregate_type TEXT,
    resource_type  TEXT,
    payload        JSONB DEFAULT '{}',
    metadata       JSONB DEFAULT '{}',
    trace_id       TEXT DEFAULT '',
    span_id        TEXT DEFAULT '',
    parent_span_id TEXT DEFAULT '',
    session_id     TEXT DEFAULT '',
    flow_id        TEXT DEFAULT '',
    sequence       BIGINT,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    shipped_at     TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_category ON events(category);
CREATE INDEX IF NOT EXISTS idx_events_org ON events(org_id);
CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(aggregate_type, aggregate_id);
CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at);
CREATE INDEX IF NOT EXISTS idx_events_flow ON events(flow_id) WHERE flow_id IS NOT NULL AND flow_id != '';

-- ============================================================================
-- DOMAINS — verified domain ownership
-- ============================================================================
CREATE TABLE IF NOT EXISTS domains (
    domain      TEXT PRIMARY KEY,
    org_id      TEXT NOT NULL,
    instance_id TEXT DEFAULT '0',
    verified    BOOLEAN NOT NULL DEFAULT FALSE,
    is_primary  BOOLEAN NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_domains_org ON domains(org_id);
CREATE INDEX IF NOT EXISTS idx_domains_instance ON domains(instance_id);

-- ============================================================================
-- UNIQUE FIELDS — cross-type uniqueness enforcement (ADR-016)
-- ============================================================================
CREATE TABLE IF NOT EXISTS unique_fields (
    scope_id         TEXT NOT NULL DEFAULT '',
    field_name       TEXT NOT NULL,
    normalized_value TEXT NOT NULL,
    resource_type    TEXT NOT NULL DEFAULT '',
    user_id          TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE(scope_id, field_name, normalized_value)
);
CREATE INDEX IF NOT EXISTS idx_unique_fields_resource ON unique_fields(user_id);
CREATE INDEX IF NOT EXISTS idx_unique_fields_lookup ON unique_fields(normalized_value, field_name);

-- ============================================================================
-- SETTINGS — instance/org/user-scoped configuration
-- ============================================================================
CREATE TABLE IF NOT EXISTS settings (
    id         TEXT PRIMARY KEY,
    type       TEXT NOT NULL,
    scope      TEXT NOT NULL DEFAULT 'instance',
    scope_id   TEXT NOT NULL DEFAULT '',
    data       JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(type, scope, scope_id)
);
CREATE INDEX IF NOT EXISTS idx_settings_type ON settings(type, scope);

-- ============================================================================
-- CACHE — generic key/value cache with namespace (replaces catalog_cache)
-- ============================================================================
CREATE TABLE IF NOT EXISTS cache (
    namespace  TEXT NOT NULL DEFAULT 'default',
    key        TEXT NOT NULL,
    data       JSONB NOT NULL,
    expires_at TIMESTAMPTZ,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (namespace, key)
);
CREATE INDEX IF NOT EXISTS idx_cache_expires ON cache(expires_at) WHERE expires_at IS NOT NULL;

-- ============================================================================
-- CONSUMER CURSORS — event consumer offsets
-- ============================================================================
CREATE TABLE IF NOT EXISTS consumer_cursors (
    consumer_name TEXT PRIMARY KEY,
    last_event_id TEXT NOT NULL DEFAULT '',
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- JOBS — scheduled task registry
-- ============================================================================
CREATE TABLE IF NOT EXISTS jobs (
    name         TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    description  TEXT DEFAULT '',
    cron         TEXT NOT NULL,
    enabled      BOOLEAN DEFAULT TRUE,
    last_run_at  TIMESTAMPTZ,
    next_run_at  TIMESTAMPTZ,
    last_status  TEXT DEFAULT 'idle',
    last_error   TEXT DEFAULT '',
    run_count    INTEGER DEFAULT 0,
    config_json  JSONB DEFAULT '{}',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- RETENTION POLICIES — event lifecycle management
-- ============================================================================
CREATE TABLE IF NOT EXISTS retention_policies (
    id            TEXT PRIMARY KEY,
    event_pattern TEXT NOT NULL,
    oltp_ttl      TEXT NOT NULL,
    lake_ttl      TEXT NOT NULL,
    priority      INTEGER DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- SEED DATA
-- ============================================================================
INSERT INTO jobs (name, display_name, description, cron)
VALUES
    ('lake_writer', 'Lake Writer', 'Drains events from OLTP buffer to Parquet files', '*/1 * * * *'),
    ('session_gc',  'Session GC',  'Cleans revoked and expired sessions',             '*/15 * * * *'),
    ('event_gc',    'Event GC',    'Deletes OLTP events past retention (shipped to lake)', '0 * * * *')
ON CONFLICT (name) DO NOTHING;

INSERT INTO retention_policies (id, event_pattern, oltp_ttl, lake_ttl, priority)
VALUES
    ('rp_auth_login_failure', 'auth.login_failure', '30d', '365d', 100),
    ('rp_auth',               'auth.*',             '14d', '365d', 90),
    ('rp_session',            'session.*',          '7d',  '90d',  80),
    ('rp_identity',           'identity.*',         '30d', '0',    70),
    ('rp_event',              'event.*',            '3d',  '30d',  60),
    ('rp_default',            '*',                  '14d', '365d', 0)
ON CONFLICT (id) DO NOTHING;

INSERT INTO instances (id, name, created_at, updated_at)
VALUES ('inst_default', 'default', NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

INSERT INTO orgs (id, instance_id, name, created_at, updated_at)
VALUES ('org_default', 'inst_default', 'default', NOW(), NOW())
ON CONFLICT (id) DO NOTHING;

-- +goose Down
DROP TABLE IF EXISTS retention_policies;
DROP TABLE IF EXISTS jobs;
DROP TABLE IF EXISTS consumer_cursors;
DROP TABLE IF EXISTS cache;
DROP TABLE IF EXISTS settings;
DROP TABLE IF EXISTS unique_fields;
DROP TABLE IF EXISTS domains;
DROP TABLE IF EXISTS events;
DROP TABLE IF EXISTS keys;
DROP TABLE IF EXISTS auth_states;
DROP TABLE IF EXISTS tokens;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS linked_identities;
DROP TABLE IF EXISTS login_flows;
DROP TABLE IF EXISTS actions;
DROP TABLE IF EXISTS apps;
DROP TABLE IF EXISTS providers;
DROP TABLE IF EXISTS credentials;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS schemas;
DROP TABLE IF EXISTS orgs;
DROP TABLE IF EXISTS instances;
