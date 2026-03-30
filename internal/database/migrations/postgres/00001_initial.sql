-- +goose Up
-- Zitadel baseline schema — PostgreSQL (single-tenant, dedicated resource tables)
-- Synced with SQLite migration, adapted for Postgres types.

-- ============================================================================
-- ORGS
-- ============================================================================
CREATE TABLE IF NOT EXISTS orgs (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    state       TEXT NOT NULL DEFAULT 'active',
    schema_id   TEXT DEFAULT '',
    metadata    JSONB DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ============================================================================
-- SCHEMAS — registry for validation, UI generation, engine bindings
-- ============================================================================
CREATE TABLE IF NOT EXISTS schemas (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL,
    org_id      TEXT NOT NULL DEFAULT '1',
    schema      JSONB NOT NULL,
    version     INTEGER DEFAULT 1,
    is_default  BOOLEAN DEFAULT FALSE,
    visibility  TEXT NOT NULL DEFAULT 'private',
    message     TEXT DEFAULT '',
    created_by  TEXT DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_schema_type_org ON schemas(type, org_id);
CREATE INDEX IF NOT EXISTS idx_schema_default ON schemas(type, org_id, is_default);
CREATE INDEX IF NOT EXISTS idx_schema_version ON schemas(type, org_id, version);

-- ============================================================================
-- USERS — human users, service accounts, AI agents (discriminated by user_type)
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
-- CREDENTIALS — passwords, passkeys, OTP, JWT profiles, certificates
-- ============================================================================
CREATE TABLE IF NOT EXISTS credentials (
    id         TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    type       TEXT NOT NULL,         -- 'password', 'passkey', 'otp', 'jwt_profile', 'certificate'
    data       JSONB DEFAULT '{}',
    name       TEXT DEFAULT '',       -- display name for this credential (e.g. passkey label)
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_creds_user ON credentials(user_id);
CREATE INDEX IF NOT EXISTS idx_creds_type ON credentials(user_id, type);

-- ============================================================================
-- PROVIDERS — identity providers (OIDC, SAML, OAuth2, LDAP)
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
-- APPS — OIDC / SAML / API clients
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
-- ACTIONS — pipeline extensions (expr, webhook, rate_limit, captcha)
-- ============================================================================
CREATE TABLE IF NOT EXISTS actions (
    id           TEXT PRIMARY KEY,
    org_id       TEXT NOT NULL DEFAULT '1',
    name         TEXT NOT NULL,
    hook         TEXT NOT NULL DEFAULT 'on_event',
    action_type  TEXT NOT NULL DEFAULT 'expr',
    trigger_expr TEXT DEFAULT 'true',
    config       JSONB NOT NULL DEFAULT '{}',
    priority     INTEGER DEFAULT 0,
    enabled      BOOLEAN DEFAULT TRUE,
    fail_open    BOOLEAN DEFAULT FALSE,
    timeout_ms   INTEGER DEFAULT 5000,
    schema_id    TEXT DEFAULT '',
    metadata     JSONB DEFAULT '{}',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
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
    strategy     TEXT DEFAULT 'identifier_first',
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
-- LINKED IDENTITIES — external identity provider links (user ↔ provider)
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
-- SESSIONS — user sessions
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
    id               TEXT PRIMARY KEY,
    type             TEXT NOT NULL,       -- 'pat', 'session', 'magic_link', 'oidc_access', 'oidc_refresh'
    token_hash       TEXT NOT NULL UNIQUE,
    user_id          TEXT REFERENCES users(id) ON DELETE CASCADE,
    session_id       TEXT REFERENCES sessions(id) ON DELETE SET NULL,
    name             TEXT DEFAULT '',           -- PAT: token name
    scopes           JSONB NOT NULL DEFAULT '[]',
    audience         TEXT DEFAULT '',           -- OIDC: audience
    application_id   TEXT DEFAULT '',           -- OIDC: client that issued it
    auth_method      TEXT DEFAULT '',           -- OIDC: amr (authentication method reference)
    auth_time        TIMESTAMPTZ,               -- OIDC: when the user authenticated
    refresh_token_id TEXT DEFAULT '',           -- OIDC access: link to refresh token
    expires_at       TIMESTAMPTZ,
    last_used        TIMESTAMPTZ,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at       TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_tokens_hash    ON tokens(token_hash);
CREATE INDEX IF NOT EXISTS idx_tokens_user    ON tokens(user_id);
CREATE INDEX IF NOT EXISTS idx_tokens_type    ON tokens(type, user_id);
CREATE INDEX IF NOT EXISTS idx_tokens_session ON tokens(session_id);
CREATE INDEX IF NOT EXISTS idx_tokens_app     ON tokens(application_id) WHERE application_id != '';

-- ============================================================================
-- SECRETS — encrypted cryptographic material (signing keys, TLS certs, IdP secrets)
-- ============================================================================
CREATE TABLE IF NOT EXISTS secrets (
    id                TEXT PRIMARY KEY,
    secret_type       TEXT NOT NULL,
    algorithm         TEXT NOT NULL DEFAULT 'RS256',
    encryption_key_id TEXT DEFAULT '',
    ciphertext        BYTEA NOT NULL,
    nonce             BYTEA DEFAULT NULL,
    public_key        BYTEA,
    expires_at        TIMESTAMPTZ,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_secrets_type ON secrets(secret_type);
CREATE INDEX IF NOT EXISTS idx_secrets_enc_key ON secrets(encryption_key_id);

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
    data                  JSONB DEFAULT '{}',   -- type-specific overflow
    expires_at            TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes'),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_auth_states_state ON auth_states(state) WHERE state != '';
CREATE INDEX IF NOT EXISTS idx_auth_states_code  ON auth_states(code)  WHERE code != '';
CREATE INDEX IF NOT EXISTS idx_auth_states_type  ON auth_states(type);

-- ============================================================================
-- FINGERPRINTS — unique hardware identifiers
-- ============================================================================
CREATE TABLE IF NOT EXISTS fingerprints (
    id         TEXT PRIMARY KEY,
    type       TEXT NOT NULL,
    raw_data   JSONB DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_fingerprints_type ON fingerprints(type);

-- ============================================================================
-- EVENTS — append-only wide event log (ADR-023)
-- ============================================================================
CREATE TABLE IF NOT EXISTS events (
    id              TEXT PRIMARY KEY,
    event_type      TEXT NOT NULL,
    category        TEXT NOT NULL DEFAULT '',
    org_id          TEXT NOT NULL DEFAULT '0',
    actor_id        TEXT,
    actor_type      TEXT,
    aggregate_id    TEXT,
    aggregate_type  TEXT,
    resource_type   TEXT,
    payload         JSONB DEFAULT '{}',
    metadata        JSONB DEFAULT '{}',
    -- correlation scopes (ADR-023: six orthogonal dimensions)
    request_id      TEXT,               -- all events from one HTTP request
    session_id      TEXT,               -- all events from one user session
    flow_id         TEXT,               -- all events from one login flow
    fingerprint     TEXT DEFAULT '',     -- all events from one device
    client_id       TEXT DEFAULT '',     -- app/agent that made the call
    token_id        TEXT DEFAULT '',     -- specific token used
    delegation_type TEXT DEFAULT '',     -- 'direct', 'delegated', 'pat_shared', 'exchanged'
    sdk_name        TEXT DEFAULT '',     -- 'zitadel-js', 'zitadel-go', etc.
    sdk_version     TEXT DEFAULT '',     -- '1.4.0'
    sequence        BIGINT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    shipped_at      TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at);
CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(aggregate_id, aggregate_type);
CREATE INDEX IF NOT EXISTS idx_events_request ON events(request_id) WHERE request_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_ship ON events(shipped_at) WHERE shipped_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_events_category ON events(category, created_at);
CREATE INDEX IF NOT EXISTS idx_events_actor ON events(actor_id) WHERE actor_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_flow ON events(flow_id) WHERE flow_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_org ON events(org_id, created_at);
CREATE INDEX IF NOT EXISTS idx_events_client ON events(client_id) WHERE client_id != '';
CREATE INDEX IF NOT EXISTS idx_events_delegation ON events(delegation_type) WHERE delegation_type != '';

-- ============================================================================
-- DOMAINS — org → domain mappings
-- ============================================================================
CREATE TABLE IF NOT EXISTS domains (
    id          TEXT PRIMARY KEY,
    org_id      TEXT NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    domain      TEXT NOT NULL,
    is_primary  BOOLEAN DEFAULT FALSE,
    verified    BOOLEAN DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(domain)
);
CREATE INDEX IF NOT EXISTS idx_domains_org ON domains(org_id);

-- ============================================================================
-- UNIQUE FIELDS — schema-driven uniqueness (ADR-016)
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
-- SETTINGS — hierarchical cascade (ADR-009)
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
-- JOBS — background scheduler state
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
-- CACHE — generic key/value cache with namespace
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
-- RETENTION POLICIES — event lifecycle configuration
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
-- GROUPS — org-scoped user grouping (SCIM-compliant, ADR-020)
-- ============================================================================
CREATE TABLE IF NOT EXISTS groups (
    id          TEXT PRIMARY KEY,
    org_id      TEXT NOT NULL DEFAULT '1',
    name        TEXT NOT NULL,
    description TEXT DEFAULT '',
    state       TEXT NOT NULL DEFAULT 'active',
    metadata    JSONB DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(org_id, name)
);
CREATE INDEX IF NOT EXISTS idx_groups_org ON groups(org_id);

-- MEMBERSHIPS — unified join table for org, group, and project membership
CREATE TABLE IF NOT EXISTS memberships (
    resource_type TEXT NOT NULL,     -- 'org', 'group', 'project'
    resource_id   TEXT NOT NULL,
    user_id       TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    role          TEXT NOT NULL DEFAULT 'member',
    added_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (resource_type, resource_id, user_id)
);
CREATE INDEX IF NOT EXISTS idx_memberships_user ON memberships(user_id, resource_type);
CREATE INDEX IF NOT EXISTS idx_memberships_resource ON memberships(resource_type, resource_id);

-- ============================================================================
-- PROJECTS — org-scoped resource containers (ADR-020)
-- ============================================================================
CREATE TABLE IF NOT EXISTS projects (
    id          TEXT PRIMARY KEY,
    org_id      TEXT NOT NULL DEFAULT '1',
    name        TEXT NOT NULL,
    description TEXT DEFAULT '',
    state       TEXT NOT NULL DEFAULT 'active',
    metadata    JSONB DEFAULT '{}',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(org_id, name)
);
CREATE INDEX IF NOT EXISTS idx_projects_org ON projects(org_id);

-- (project_members removed — use memberships table with resource_type='project')

-- ============================================================================
-- SAVED QUERIES — server-side analytics query persistence
-- ============================================================================
CREATE TABLE IF NOT EXISTS saved_queries (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    description TEXT DEFAULT '',
    sql_text    TEXT NOT NULL,
    created_by  TEXT DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_saved_queries_name ON saved_queries(name);

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

-- +goose Down
DROP TABLE IF EXISTS saved_queries;
DROP TABLE IF EXISTS memberships;
DROP TABLE IF EXISTS projects;
DROP TABLE IF EXISTS groups;
DROP TABLE IF EXISTS retention_policies;
DROP TABLE IF EXISTS consumer_cursors;
DROP TABLE IF EXISTS cache;
DROP TABLE IF EXISTS jobs;
DROP TABLE IF EXISTS settings;
DROP TABLE IF EXISTS unique_fields;
DROP TABLE IF EXISTS domains;
DROP TABLE IF EXISTS events;
DROP TABLE IF EXISTS fingerprints;
DROP TABLE IF EXISTS auth_states;
DROP TABLE IF EXISTS secrets;
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
