-- Zitadel POC Schema — single-file DDL (SQLite)
-- This replaces the 7 individual goose migration files (00001–00007).
-- All statements use IF NOT EXISTS for idempotent re-runs.

-- ============================================================================
-- ENTITIES — the universal identity table (ADR-001)
-- ============================================================================
CREATE TABLE IF NOT EXISTS entities (
    id           INTEGER PRIMARY KEY,
    org_id       INTEGER NOT NULL DEFAULT 0,
    identifier   TEXT NOT NULL,
    display_name TEXT,
    state        TEXT NOT NULL DEFAULT 'active',
    schema_id    TEXT DEFAULT '',
    profile      TEXT DEFAULT '{}',
    metadata     TEXT DEFAULT '{}',
    data         TEXT DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_entities_org ON entities(org_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_entities_identifier ON entities(org_id, identifier);

-- ============================================================================
-- ENTITY CAPABILITIES — junction table for hot-path indexed checks
-- ============================================================================
CREATE TABLE IF NOT EXISTS entity_capabilities (
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    capability  TEXT NOT NULL,
    PRIMARY KEY (entity_id, capability)
);
CREATE INDEX IF NOT EXISTS idx_caps_capability ON entity_capabilities(capability);

-- ============================================================================
-- ENTITY CREDENTIALS — type-specific credential data
-- ============================================================================
CREATE TABLE IF NOT EXISTS entity_credentials (
    id              INTEGER PRIMARY KEY,
    entity_id     INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    credential_type TEXT NOT NULL,
    credential_data TEXT DEFAULT '{}',
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_creds_identity ON entity_credentials(entity_id);

-- ============================================================================
-- SESSIONS
-- ============================================================================
CREATE TABLE IF NOT EXISTS sessions (
    id          INTEGER PRIMARY KEY,
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    org_id      INTEGER NOT NULL DEFAULT 0,
    token_hash  TEXT NOT NULL,
    user_agent  TEXT,
    ip_address  TEXT,
    metadata    TEXT DEFAULT '{}',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at  TEXT NOT NULL,
    revoked_at  TEXT
);
CREATE INDEX IF NOT EXISTS idx_sessions_identity ON sessions(entity_id);
CREATE INDEX IF NOT EXISTS idx_sessions_token ON sessions(token_hash);

-- ============================================================================
-- EVENTS — append-only event log (the queue IS the table)
-- ============================================================================
CREATE TABLE IF NOT EXISTS events (
    id             INTEGER PRIMARY KEY,
    event_type     TEXT NOT NULL,
    org_id         INTEGER NOT NULL DEFAULT 0,
    actor_id       INTEGER,
    actor_type     TEXT,
    aggregate_id   INTEGER,
    aggregate_type TEXT,
    payload        TEXT DEFAULT '{}',
    metadata       TEXT DEFAULT '{}',
    trace_id       TEXT DEFAULT '',
    session_id     INTEGER DEFAULT 0,
    created_at     TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_events_type ON events(event_type);
CREATE INDEX IF NOT EXISTS idx_events_org ON events(org_id);
CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(aggregate_type, aggregate_id);
CREATE INDEX IF NOT EXISTS idx_events_created ON events(created_at);

-- ============================================================================
-- DOMAINS — external domain → org mapping
-- ============================================================================
CREATE TABLE IF NOT EXISTS domains (
    domain      TEXT PRIMARY KEY,
    org_id      INTEGER NOT NULL,
    instance_id INTEGER DEFAULT 0,
    verified    INTEGER NOT NULL DEFAULT 0,
    is_primary  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_domains_org ON domains(org_id);
CREATE INDEX IF NOT EXISTS idx_domains_instance ON domains(instance_id);

-- ============================================================================
-- NOTIFICATION TEMPLATES — per-org, per-language
-- ============================================================================
CREATE TABLE IF NOT EXISTS notification_templates (
    id       INTEGER PRIMARY KEY,
    org_id   INTEGER,
    channel  TEXT NOT NULL,
    event    TEXT NOT NULL,
    language TEXT NOT NULL DEFAULT 'en',
    subject  TEXT,
    body     TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_notif_tpl_unique ON notification_templates(org_id, channel, event, language);

-- ============================================================================
-- MAGIC TOKENS — single-use tokens for magic link auth
-- ============================================================================
CREATE TABLE IF NOT EXISTS magic_tokens (
    token       TEXT PRIMARY KEY,
    entity_id INTEGER NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    expires_at  TEXT NOT NULL,
    used_at     TEXT,
    session_id  INTEGER
);

-- ============================================================================
-- CONSUMER CURSORS — async worker position tracking
-- ============================================================================
CREATE TABLE IF NOT EXISTS consumer_cursors (
    consumer_name TEXT PRIMARY KEY,
    last_event_id INTEGER NOT NULL DEFAULT 0,
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- JOBS — background worker registrations
-- ============================================================================
CREATE TABLE IF NOT EXISTS jobs (
    name         TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    description  TEXT DEFAULT '',
    cron         TEXT NOT NULL,
    enabled      INTEGER DEFAULT 1,
    last_run_at  TEXT,
    next_run_at  TEXT,
    last_status  TEXT DEFAULT 'idle',
    last_error   TEXT DEFAULT '',
    run_count    INTEGER DEFAULT 0,
    config_json  TEXT DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- RETENTION POLICIES — per-event-type TTLs
-- ============================================================================
CREATE TABLE IF NOT EXISTS retention_policies (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    event_pattern TEXT NOT NULL,
    oltp_ttl      TEXT NOT NULL,
    lake_ttl      TEXT NOT NULL,
    priority      INTEGER DEFAULT 0,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- INSTANCES — virtual identity systems (top-level isolation)
-- ============================================================================
CREATE TABLE IF NOT EXISTS instances (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    state      TEXT NOT NULL DEFAULT 'active',
    settings   TEXT DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- ORGS — groupings within an instance
-- ============================================================================
CREATE TABLE IF NOT EXISTS orgs (
    id          INTEGER PRIMARY KEY,
    instance_id INTEGER NOT NULL REFERENCES instances(id),
    name        TEXT NOT NULL,
    state       TEXT NOT NULL DEFAULT 'active',
    metadata    TEXT DEFAULT '{}',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_orgs_instance ON orgs(instance_id);

-- ============================================================================
-- GROUPS — RBAC grouping within an org
-- ============================================================================
CREATE TABLE IF NOT EXISTS groups (
    id         INTEGER PRIMARY KEY,
    org_id     INTEGER NOT NULL REFERENCES orgs(id),
    name       TEXT NOT NULL,
    metadata   TEXT DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_groups_org ON groups(org_id);

-- ============================================================================
-- SCHEMAS — JSON Schema registry per org/type
-- ============================================================================
CREATE TABLE IF NOT EXISTS schemas (
    id         TEXT PRIMARY KEY,
    type       TEXT NOT NULL,
    org_id     INTEGER NOT NULL DEFAULT 1,
    schema     TEXT NOT NULL,
    version    INTEGER DEFAULT 1,
    is_default BOOLEAN DEFAULT false,
    message    TEXT DEFAULT '',
    created_by TEXT DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_schema_type_org ON schemas(type, org_id);
CREATE INDEX IF NOT EXISTS idx_schema_default ON schemas(type, org_id, is_default);
CREATE INDEX IF NOT EXISTS idx_schema_version ON schemas(type, org_id, version);

-- ============================================================================
-- ENTITY INDEXES — promoted fields for O(log N) lookups
-- ============================================================================
CREATE TABLE IF NOT EXISTS entity_indexes (
    entity_type TEXT NOT NULL,
    entity_id   INTEGER NOT NULL,
    field       TEXT NOT NULL,
    value       TEXT NOT NULL,
    PRIMARY KEY (entity_type, entity_id, field)
);
CREATE INDEX IF NOT EXISTS idx_ei_lookup ON entity_indexes(entity_type, field, value);

-- ============================================================================
-- PROVIDERS — external identity sources (OIDC, SAML, SCIM)
-- ============================================================================
CREATE TABLE IF NOT EXISTS providers (
    id              TEXT PRIMARY KEY,
    org_id          INTEGER NOT NULL DEFAULT 1,
    name            TEXT NOT NULL,
    protocol        TEXT NOT NULL DEFAULT 'oidc',
    template        TEXT NOT NULL DEFAULT 'custom',
    config          TEXT NOT NULL DEFAULT '{}',
    claim_overrides TEXT NOT NULL DEFAULT '{}',
    auto_register   BOOLEAN NOT NULL DEFAULT 1,
    enabled         BOOLEAN NOT NULL DEFAULT 1,
    display_order   INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_providers_org ON providers(org_id);

-- ============================================================================
-- LINKED ACCOUNTS — user ↔ external provider links
-- ============================================================================
CREATE TABLE IF NOT EXISTS linked_accounts (
    id             INTEGER PRIMARY KEY,
    entity_id    INTEGER NOT NULL,
    provider_id    TEXT NOT NULL,
    external_sub   TEXT NOT NULL,
    external_email TEXT DEFAULT '',
    raw_claims     TEXT DEFAULT '{}',
    linked_at      TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at   TEXT,
    UNIQUE(provider_id, external_sub)
);
CREATE INDEX IF NOT EXISTS idx_linked_identity ON linked_accounts(entity_id);
CREATE INDEX IF NOT EXISTS idx_linked_provider ON linked_accounts(provider_id, external_sub);

-- ============================================================================
-- SSO STATES — ephemeral OIDC authorization flow state
-- ============================================================================
CREATE TABLE IF NOT EXISTS sso_states (
    state         TEXT PRIMARY KEY,
    provider_id   TEXT NOT NULL,
    pkce_verifier TEXT NOT NULL,
    nonce         TEXT NOT NULL,
    redirect_uri  TEXT DEFAULT '',
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- OIDC AUTH REQUESTS — ephemeral authorization requests (ADR-004)
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
    auth_time             TEXT,
    done                  INTEGER DEFAULT 0,
    created_at            TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- OIDC CODES — authorization codes → request mapping
-- ============================================================================
CREATE TABLE IF NOT EXISTS oidc_codes (
    code       TEXT PRIMARY KEY,
    request_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- OIDC TOKENS — opaque access tokens
-- ============================================================================
CREATE TABLE IF NOT EXISTS oidc_tokens (
    id               TEXT PRIMARY KEY,
    application_id   TEXT NOT NULL,
    subject          TEXT NOT NULL,
    audience         TEXT NOT NULL DEFAULT '',
    scopes           TEXT NOT NULL DEFAULT '',
    refresh_token_id TEXT DEFAULT '',
    expiration       TEXT NOT NULL,
    created_at       TEXT NOT NULL DEFAULT (datetime('now'))
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
    auth_time      TEXT NOT NULL,
    amr            TEXT DEFAULT '',
    access_token   TEXT NOT NULL,
    expiration     TEXT NOT NULL
);

-- ============================================================================
-- OIDC SIGNING KEYS — RSA keys for JWT signing
-- ============================================================================
CREATE TABLE IF NOT EXISTS oidc_signing_keys (
    id          TEXT PRIMARY KEY,
    algorithm   TEXT NOT NULL DEFAULT 'RS256',
    private_key BLOB NOT NULL,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ============================================================================
-- SEED DATA
-- ============================================================================

-- Default jobs
INSERT OR IGNORE INTO jobs (name, display_name, description, cron) VALUES
    ('lake_writer', 'Lake Writer', 'Drains events from OLTP buffer to Parquet files', '*/1 * * * *'),
    ('session_gc',  'Session GC',  'Cleans revoked and expired sessions',             '*/15 * * * *'),
    ('event_gc',    'Event GC',    'Deletes OLTP events past retention (shipped to lake)', '0 * * * *');

-- Default retention policies (higher priority = matched first)
INSERT OR IGNORE INTO retention_policies (event_pattern, oltp_ttl, lake_ttl, priority) VALUES
    ('auth.login_failure', '30d', '365d', 100),
    ('auth.*',             '14d', '365d', 90),
    ('session.*',          '7d',  '90d',  80),
    ('identity.*',         '30d', '0',    70),
    ('event.*',            '3d',  '30d',  60),
    ('*',                  '14d', '365d', 0);

-- Default instance and org
INSERT OR IGNORE INTO instances (id, name, created_at, updated_at)
    VALUES (1, 'default', datetime('now'), datetime('now'));

INSERT OR IGNORE INTO orgs (id, instance_id, name, created_at, updated_at)
    VALUES (1, 1, 'default', datetime('now'), datetime('now'));
