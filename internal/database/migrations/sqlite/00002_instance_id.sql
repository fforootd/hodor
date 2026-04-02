-- +goose Up
-- Add instance_id to all tenant-scoped tables for multi-tenant cloud deployment.
-- Default 'default' means existing data and self-hosted deployments work unchanged.
-- schemas table excluded (global). FGA tables excluded (use store_id).

-- ── Add instance_id column ─────────────────────────────────────────────

ALTER TABLE orgs ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE credentials ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE actions ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE login_flows ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE login_flow_assets ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE linked_identities ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE memberships ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE sessions ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE tokens ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE auth_states ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE secrets ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE events ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE fingerprints ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE saved_queries ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE jobs ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE cache ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE consumer_cursors ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE retention_policies ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';

-- ── Indexes ────────────────────────────────────────────────────────────

CREATE INDEX IF NOT EXISTS idx_orgs_instance ON orgs(instance_id);
CREATE INDEX IF NOT EXISTS idx_credentials_instance ON credentials(instance_id, user_id);
CREATE INDEX IF NOT EXISTS idx_actions_instance ON actions(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_login_flows_instance ON login_flows(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_linked_identities_instance ON linked_identities(instance_id);
CREATE INDEX IF NOT EXISTS idx_memberships_instance ON memberships(instance_id);
CREATE INDEX IF NOT EXISTS idx_sessions_instance ON sessions(instance_id);
CREATE INDEX IF NOT EXISTS idx_tokens_instance ON tokens(instance_id);
CREATE INDEX IF NOT EXISTS idx_tokens_instance_hash ON tokens(instance_id, token_hash);
CREATE INDEX IF NOT EXISTS idx_auth_states_instance ON auth_states(instance_id);
CREATE INDEX IF NOT EXISTS idx_secrets_instance ON secrets(instance_id);
CREATE INDEX IF NOT EXISTS idx_events_instance ON events(instance_id, created_at);
CREATE INDEX IF NOT EXISTS idx_fingerprints_instance ON fingerprints(instance_id);
CREATE INDEX IF NOT EXISTS idx_jobs_instance ON jobs(instance_id);
CREATE INDEX IF NOT EXISTS idx_cache_instance ON cache(instance_id);

-- ── Table rebuilds for UNIQUE constraint changes ───────────────────────
-- SQLite cannot ALTER CONSTRAINT, so we rebuild tables.

-- users: UNIQUE(org_id, identifier) → UNIQUE(instance_id, org_id, identifier)
CREATE TABLE users_new (
    id            TEXT PRIMARY KEY,
    instance_id   TEXT NOT NULL DEFAULT 'default',
    org_id        TEXT NOT NULL DEFAULT '1',
    identifier    TEXT NOT NULL,
    display_name  TEXT DEFAULT '',
    user_type     TEXT NOT NULL DEFAULT 'human',
    state         TEXT NOT NULL DEFAULT 'active',
    schema_id     TEXT DEFAULT '',
    metadata      TEXT DEFAULT '{}',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instance_id, org_id, identifier)
);
INSERT INTO users_new (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
    SELECT id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at FROM users;
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;
CREATE INDEX idx_users_instance ON users(instance_id, org_id);

-- providers: UNIQUE(org_id, name) → UNIQUE(instance_id, org_id, name)
CREATE TABLE providers_new (
    id               TEXT PRIMARY KEY,
    instance_id      TEXT NOT NULL DEFAULT 'default',
    org_id           TEXT NOT NULL DEFAULT '1',
    name             TEXT NOT NULL,
    protocol         TEXT NOT NULL DEFAULT 'oidc',
    template         TEXT NOT NULL DEFAULT 'custom',
    config           TEXT NOT NULL DEFAULT '{}',
    claim_overrides  TEXT NOT NULL DEFAULT '{}',
    auto_register    BOOLEAN NOT NULL DEFAULT 1,
    enabled          BOOLEAN NOT NULL DEFAULT 1,
    display_order    INTEGER NOT NULL DEFAULT 0,
    schema_id        TEXT DEFAULT '',
    target_schema_id TEXT DEFAULT '',
    target_schema_type TEXT DEFAULT '',
    metadata         TEXT DEFAULT '{}',
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instance_id, org_id, name)
);
INSERT INTO providers_new (id, org_id, name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, schema_id, target_schema_id, target_schema_type, metadata, created_at, updated_at)
    SELECT id, org_id, name, protocol, template, config, claim_overrides, auto_register, enabled, display_order, schema_id, target_schema_id, target_schema_type, metadata, created_at, updated_at FROM providers;
DROP TABLE providers;
ALTER TABLE providers_new RENAME TO providers;
CREATE INDEX idx_providers_instance ON providers(instance_id, org_id);

-- apps: UNIQUE(client_id) → UNIQUE(instance_id, client_id)
CREATE TABLE apps_new (
    id             TEXT PRIMARY KEY,
    instance_id    TEXT NOT NULL DEFAULT 'default',
    org_id         TEXT NOT NULL DEFAULT '1',
    name           TEXT NOT NULL,
    app_type       TEXT NOT NULL DEFAULT 'oidc',
    client_id      TEXT NOT NULL,
    client_secret  TEXT DEFAULT '',
    redirect_uris  TEXT DEFAULT '[]',
    grant_types    TEXT DEFAULT '["authorization_code"]',
    response_types TEXT DEFAULT '["code"]',
    state          TEXT NOT NULL DEFAULT 'active',
    schema_id      TEXT DEFAULT '',
    metadata       TEXT DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instance_id, client_id)
);
INSERT INTO apps_new (id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state, schema_id, metadata, created_at, updated_at)
    SELECT id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state, schema_id, metadata, created_at, updated_at FROM apps;
DROP TABLE apps;
ALTER TABLE apps_new RENAME TO apps;
CREATE INDEX idx_apps_instance ON apps(instance_id);
CREATE INDEX idx_apps_instance_client ON apps(instance_id, client_id);

-- domains: UNIQUE(domain) → UNIQUE(instance_id, domain)
CREATE TABLE domains_new (
    id          TEXT PRIMARY KEY,
    instance_id TEXT NOT NULL DEFAULT 'default',
    org_id      TEXT NOT NULL REFERENCES orgs(id) ON DELETE CASCADE,
    domain      TEXT NOT NULL,
    is_primary  BOOLEAN DEFAULT 0,
    verified    BOOLEAN DEFAULT 0,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instance_id, domain)
);
INSERT INTO domains_new (id, org_id, domain, is_primary, verified, created_at)
    SELECT id, org_id, domain, is_primary, verified, created_at FROM domains;
DROP TABLE domains;
ALTER TABLE domains_new RENAME TO domains;
CREATE INDEX idx_domains_instance ON domains(instance_id);

-- groups: UNIQUE(org_id, name) → UNIQUE(instance_id, org_id, name)
CREATE TABLE groups_new (
    id          TEXT PRIMARY KEY,
    instance_id TEXT NOT NULL DEFAULT 'default',
    org_id      TEXT NOT NULL DEFAULT '1',
    name        TEXT NOT NULL,
    description TEXT DEFAULT '',
    state       TEXT NOT NULL DEFAULT 'active',
    schema_id   TEXT DEFAULT '',
    metadata    TEXT DEFAULT '{}',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instance_id, org_id, name)
);
INSERT INTO groups_new (id, org_id, name, description, state, schema_id, metadata, created_at, updated_at)
    SELECT id, org_id, name, description, state, schema_id, metadata, created_at, updated_at FROM groups;
DROP TABLE groups;
ALTER TABLE groups_new RENAME TO groups;
CREATE INDEX idx_groups_instance ON groups(instance_id, org_id);

-- projects: UNIQUE(org_id, name) → UNIQUE(instance_id, org_id, name)
CREATE TABLE projects_new (
    id          TEXT PRIMARY KEY,
    instance_id TEXT NOT NULL DEFAULT 'default',
    org_id      TEXT NOT NULL DEFAULT '1',
    name        TEXT NOT NULL,
    description TEXT DEFAULT '',
    state       TEXT NOT NULL DEFAULT 'active',
    schema_id   TEXT DEFAULT '',
    metadata    TEXT DEFAULT '{}',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instance_id, org_id, name)
);
INSERT INTO projects_new (id, org_id, name, description, state, schema_id, metadata, created_at, updated_at)
    SELECT id, org_id, name, description, state, schema_id, metadata, created_at, updated_at FROM projects;
DROP TABLE projects;
ALTER TABLE projects_new RENAME TO projects;
CREATE INDEX idx_projects_instance ON projects(instance_id, org_id);

-- unique_fields: UNIQUE(scope_id, field_name, normalized_value) → UNIQUE(instance_id, ...)
CREATE TABLE unique_fields_new (
    instance_id      TEXT NOT NULL DEFAULT 'default',
    scope_id         TEXT NOT NULL DEFAULT '',
    field_name       TEXT NOT NULL,
    normalized_value TEXT NOT NULL,
    resource_type    TEXT NOT NULL DEFAULT '',
    user_id          TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    UNIQUE(instance_id, scope_id, field_name, normalized_value)
);
INSERT INTO unique_fields_new (scope_id, field_name, normalized_value, resource_type, user_id)
    SELECT scope_id, field_name, normalized_value, resource_type, user_id FROM unique_fields;
DROP TABLE unique_fields;
ALTER TABLE unique_fields_new RENAME TO unique_fields;
CREATE INDEX idx_unique_fields_instance ON unique_fields(instance_id, scope_id, field_name);

-- settings: UNIQUE(type, scope, scope_id) → UNIQUE(instance_id, type, scope, scope_id)
CREATE TABLE settings_new (
    id          TEXT PRIMARY KEY,
    instance_id TEXT NOT NULL DEFAULT 'default',
    type        TEXT NOT NULL,
    scope       TEXT NOT NULL DEFAULT 'instance',
    scope_id    TEXT NOT NULL DEFAULT '',
    data        TEXT NOT NULL DEFAULT '{}',
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instance_id, type, scope, scope_id)
);
INSERT INTO settings_new (id, type, scope, scope_id, data, created_at, updated_at)
    SELECT id, type, scope, scope_id, data, created_at, updated_at FROM settings;
DROP TABLE settings;
ALTER TABLE settings_new RENAME TO settings;
CREATE INDEX idx_settings_instance ON settings(instance_id, type, scope, scope_id);

-- ── FGA instance store mapping ─────────────────────────────────────────
CREATE TABLE IF NOT EXISTS fga_instance_stores (
    instance_id TEXT PRIMARY KEY,
    store_id    TEXT NOT NULL
);

-- +goose Down
DROP TABLE IF EXISTS fga_instance_stores;
