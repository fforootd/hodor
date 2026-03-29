-- +goose Up
-- Zitadel baseline schema — PostgreSQL (ADR-022: dedicated resource tables)

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
-- SCHEMAS
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
-- USERS
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
-- USER CREDENTIALS
-- ============================================================================
CREATE TABLE IF NOT EXISTS user_credentials (
    id              TEXT PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    credential_type TEXT NOT NULL,
    credential_data JSONB DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_creds_user ON user_credentials(user_id);

-- ============================================================================
-- PROVIDERS
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
-- APPS
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
-- ACTIONS
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
-- LOGIN FLOWS
-- ============================================================================
CREATE TABLE IF NOT EXISTS login_flows (
    id         TEXT PRIMARY KEY,
    org_id     TEXT NOT NULL DEFAULT '1',
    name       TEXT NOT NULL,
    preset     TEXT DEFAULT 'identifier_first',
    steps      JSONB NOT NULL DEFAULT '[]',
    config     JSONB NOT NULL DEFAULT '{}',
    state      TEXT NOT NULL DEFAULT 'active',
    schema_id  TEXT DEFAULT '',
    metadata   JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_lf_org ON login_flows(org_id);

-- ============================================================================
-- LINKED ACCOUNTS
-- ============================================================================
CREATE TABLE IF NOT EXISTS linked_accounts (
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
CREATE INDEX IF NOT EXISTS idx_linked_user ON linked_accounts(user_id);
CREATE INDEX IF NOT EXISTS idx_linked_provider ON linked_accounts(provider_id, external_sub);

-- ============================================================================
-- SESSIONS
-- ============================================================================
CREATE TABLE IF NOT EXISTS sessions (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    org_id     TEXT NOT NULL DEFAULT '0',
    token_hash TEXT NOT NULL,
    user_agent TEXT,
    ip_address TEXT,
    metadata   JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash);

-- ============================================================================
-- TOKENS
-- ============================================================================
CREATE TABLE IF NOT EXISTS tokens (
    id         TEXT PRIMARY KEY,
    type       TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    user_id    TEXT REFERENCES users(id) ON DELETE CASCADE,
    session_id TEXT REFERENCES sessions(id) ON DELETE CASCADE,
    name       TEXT,
    scopes     JSONB NOT NULL DEFAULT '[]',
    expires_at TIMESTAMPTZ,
    last_used  TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_tokens_hash ON tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_tokens_user ON tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_tokens_session ON tokens(session_id);

-- ============================================================================
-- EVENTS
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
    payload        JSONB DEFAULT '{}',
    metadata       JSONB DEFAULT '{}',
    trace_id       TEXT DEFAULT '',
    span_id        TEXT DEFAULT '',
    parent_span_id TEXT DEFAULT '',
    session_id     TEXT DEFAULT '',
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_category ON events(category);
CREATE INDEX IF NOT EXISTS idx_events_org ON events(org_id);
CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(aggregate_type, aggregate_id);
CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at);

-- ============================================================================
-- DOMAINS
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
-- MAGIC TOKENS
-- ============================================================================
CREATE TABLE IF NOT EXISTS magic_tokens (
    token      TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ,
    session_id TEXT
);

-- ============================================================================
-- CONSUMER CURSORS
-- ============================================================================
CREATE TABLE IF NOT EXISTS consumer_cursors (
    consumer_name TEXT PRIMARY KEY,
    last_event_id TEXT NOT NULL DEFAULT '',
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- JOBS
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
-- RETENTION POLICIES
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
-- SETTINGS
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
-- CATALOG CACHE
-- ============================================================================
CREATE TABLE IF NOT EXISTS catalog_cache (
    key        TEXT PRIMARY KEY,
    data       JSONB NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- UNIQUE FIELDS
-- ============================================================================
CREATE TABLE IF NOT EXISTS unique_fields (
    scope_id         TEXT NOT NULL DEFAULT '',
    field_name       TEXT NOT NULL,
    normalized_value TEXT NOT NULL,
    resource_type    TEXT NOT NULL DEFAULT '',
    resource_id      TEXT NOT NULL,
    UNIQUE(scope_id, field_name, normalized_value)
);
CREATE INDEX IF NOT EXISTS idx_unique_fields_resource ON unique_fields(resource_id);
CREATE INDEX IF NOT EXISTS idx_unique_fields_lookup ON unique_fields(normalized_value, field_name);

-- ============================================================================
-- RESOURCE INDEXES
-- ============================================================================
CREATE TABLE IF NOT EXISTS resource_indexes (
    resource_type TEXT NOT NULL,
    resource_id   TEXT NOT NULL,
    field         TEXT NOT NULL,
    value         TEXT NOT NULL,
    PRIMARY KEY (resource_type, resource_id, field)
);
CREATE INDEX IF NOT EXISTS idx_ri_lookup ON resource_indexes(resource_type, field, value);

-- ============================================================================
-- SSO STATES
-- ============================================================================
CREATE TABLE IF NOT EXISTS sso_states (
    state         TEXT PRIMARY KEY,
    provider_id   TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    nonce         TEXT NOT NULL,
    redirect_uri  TEXT DEFAULT '',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- OIDC AUTH REQUESTS
-- ============================================================================
CREATE TABLE IF NOT EXISTS oidc_auth_requests (
    id                    TEXT PRIMARY KEY,
    client_id             TEXT NOT NULL,
    redirect_uri          TEXT NOT NULL,
    scopes                TEXT NOT NULL DEFAULT '',
    state                 TEXT DEFAULT '',
    nonce                 TEXT DEFAULT '',
    response_type         TEXT DEFAULT 'code',
    code_challenge        TEXT DEFAULT '',
    code_challenge_method TEXT DEFAULT '',
    user_id               TEXT DEFAULT '',
    auth_time             TIMESTAMPTZ,
    done                  BOOLEAN DEFAULT FALSE,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- OIDC CODES
-- ============================================================================
CREATE TABLE IF NOT EXISTS oidc_codes (
    code       TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- OIDC TOKENS
-- ============================================================================
CREATE TABLE IF NOT EXISTS oidc_tokens (
    id               TEXT PRIMARY KEY,
    application_id   TEXT NOT NULL,
    subject          TEXT NOT NULL,
    audience         TEXT NOT NULL DEFAULT '',
    scopes           TEXT NOT NULL DEFAULT '',
    refresh_token_id TEXT DEFAULT '',
    expiration       TIMESTAMPTZ NOT NULL,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- OIDC REFRESH TOKENS
-- ============================================================================
CREATE TABLE IF NOT EXISTS oidc_refresh_tokens (
    id             TEXT PRIMARY KEY,
    token          TEXT NOT NULL UNIQUE,
    application_id TEXT NOT NULL,
    user_id        TEXT NOT NULL,
    audience       TEXT NOT NULL DEFAULT '',
    scopes         TEXT NOT NULL DEFAULT '',
    auth_time      TIMESTAMPTZ NOT NULL,
    amr            TEXT DEFAULT '',
    access_token   TEXT NOT NULL,
    expiration     TIMESTAMPTZ NOT NULL
);

-- ============================================================================
-- OIDC SIGNING KEYS
-- ============================================================================
CREATE TABLE IF NOT EXISTS oidc_signing_keys (
    id          TEXT PRIMARY KEY,
    algorithm   TEXT NOT NULL DEFAULT 'RS256',
    private_key BYTEA NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
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
DROP TABLE IF EXISTS oidc_signing_keys;
DROP TABLE IF EXISTS oidc_refresh_tokens;
DROP TABLE IF EXISTS oidc_tokens;
DROP TABLE IF EXISTS oidc_codes;
DROP TABLE IF EXISTS oidc_auth_requests;
DROP TABLE IF EXISTS sso_states;
DROP TABLE IF EXISTS resource_indexes;
DROP TABLE IF EXISTS unique_fields;
DROP TABLE IF EXISTS catalog_cache;
DROP TABLE IF EXISTS settings;
DROP TABLE IF EXISTS linked_accounts;
DROP TABLE IF EXISTS magic_tokens;
DROP TABLE IF EXISTS tokens;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS login_flows;
DROP TABLE IF EXISTS actions;
DROP TABLE IF EXISTS apps;
DROP TABLE IF EXISTS providers;
DROP TABLE IF EXISTS user_credentials;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS domains;
DROP TABLE IF EXISTS events;
DROP TABLE IF EXISTS retention_policies;
DROP TABLE IF EXISTS jobs;
DROP TABLE IF EXISTS consumer_cursors;
DROP TABLE IF EXISTS schemas;
DROP TABLE IF EXISTS orgs;
DROP TABLE IF EXISTS instances;
