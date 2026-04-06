-- +goose Up
-- Change all org FK constraints from CASCADE to no constraint (SQLite).
-- Orgs are relationships, not ownership — deleting one should orphan resources, not destroy them.
--
-- SQLite does not support column-specific ON DELETE SET NULL for composite FKs.
-- Using ON DELETE SET NULL on (instance_id, org_id) would null BOTH columns,
-- violating the NOT NULL constraint on instance_id.
-- Solution: drop the org FK entirely on SQLite, the app layer handles integrity.
-- Also applies the 00011 fix for users/sessions that was a no-op due to missing goose marker.

-- SQLite cannot ALTER foreign keys, so we recreate each table.
-- Use explicit column lists in INSERT to guard against column-order drift.

-- ── users ──

CREATE TABLE users_new (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    org_id        TEXT DEFAULT NULL,
    identifier    TEXT NOT NULL,
    display_name  TEXT DEFAULT '',
    user_type     TEXT NOT NULL DEFAULT 'human',
    state         TEXT NOT NULL DEFAULT 'active',
    schema_id     TEXT DEFAULT '',
    metadata      TEXT DEFAULT '{}',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, identifier)
);

INSERT INTO users_new (instance_id, id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at FROM users;
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

CREATE INDEX idx_users_instance_org ON users(instance_id, org_id);
CREATE INDEX idx_users_instance_state ON users(instance_id, state);
CREATE INDEX idx_users_instance_type ON users(instance_id, user_type);
CREATE UNIQUE INDEX idx_users_instance_identifier_no_org
    ON users(instance_id, identifier) WHERE org_id IS NULL;

-- ── sessions ──

CREATE TABLE sessions_new (
    instance_id      TEXT NOT NULL,
    id               TEXT NOT NULL,
    user_id          TEXT NOT NULL,
    org_id           TEXT DEFAULT NULL,
    token_hash       TEXT NOT NULL DEFAULT '',
    user_agent       TEXT DEFAULT '',
    ip_address       TEXT DEFAULT '',
    metadata         TEXT DEFAULT '{}',
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    last_active_at   TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at       TEXT,
    revoked_at       TEXT,
    fingerprint      TEXT DEFAULT '',
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);

INSERT INTO sessions_new (instance_id, id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, last_active_at, expires_at, revoked_at, fingerprint)
    SELECT instance_id, id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, last_active_at, expires_at, revoked_at, fingerprint FROM sessions;
DROP TABLE sessions;
ALTER TABLE sessions_new RENAME TO sessions;

CREATE INDEX idx_sessions_instance_user ON sessions(instance_id, user_id);
CREATE INDEX idx_sessions_instance_expires
    ON sessions(instance_id, expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX idx_sessions_instance_revoked
    ON sessions(instance_id, revoked_at) WHERE revoked_at IS NOT NULL;
CREATE UNIQUE INDEX idx_sessions_instance_token_unique
    ON sessions(instance_id, token_hash) WHERE token_hash != '';

-- ── apps ──

CREATE TABLE apps_new (
    instance_id             TEXT NOT NULL,
    id                      TEXT NOT NULL,
    org_id                  TEXT DEFAULT NULL,
    name                    TEXT NOT NULL,
    app_type                TEXT NOT NULL DEFAULT 'oidc',
    client_id               TEXT NOT NULL,
    client_secret           TEXT DEFAULT '',
    redirect_uris           TEXT DEFAULT '[]',
    grant_types             TEXT DEFAULT '["authorization_code"]',
    response_types          TEXT DEFAULT '["code"]',
    state                   TEXT NOT NULL DEFAULT 'active',
    schema_id               TEXT DEFAULT '',
    metadata                TEXT DEFAULT '{}',
    created_at              TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at              TEXT NOT NULL DEFAULT (datetime('now')),
    post_logout_redirect_uris TEXT NOT NULL DEFAULT '[]',
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, client_id)
);

INSERT INTO apps_new (instance_id, id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state, schema_id, metadata, created_at, updated_at, post_logout_redirect_uris)
    SELECT instance_id, id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state, schema_id, metadata, created_at, updated_at, post_logout_redirect_uris FROM apps;
DROP TABLE apps;
ALTER TABLE apps_new RENAME TO apps;

CREATE INDEX idx_apps_instance_org ON apps(instance_id, org_id);
CREATE INDEX idx_apps_instance_client ON apps(instance_id, client_id);

-- ── providers ──

CREATE TABLE providers_new (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    org_id         TEXT DEFAULT NULL,
    display_name   TEXT NOT NULL,
    kind           TEXT NOT NULL DEFAULT 'custom',
    protocol       TEXT NOT NULL DEFAULT 'oidc',
    connection     TEXT NOT NULL DEFAULT '{}',
    mapping        TEXT NOT NULL DEFAULT '{}',
    target         TEXT NOT NULL DEFAULT '{}',
    linking        TEXT NOT NULL DEFAULT '{}',
    session        TEXT NOT NULL DEFAULT '{}',
    ui             TEXT NOT NULL DEFAULT '{}',
    enabled        BOOLEAN NOT NULL DEFAULT 1,
    display_order  INTEGER NOT NULL DEFAULT 0,
    catalog_ref    TEXT NOT NULL DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, display_name)
);

INSERT INTO providers_new (instance_id, id, org_id, display_name, kind, protocol, connection, mapping, target, linking, session, ui, enabled, display_order, catalog_ref, created_at, updated_at)
    SELECT instance_id, id, org_id, display_name, kind, protocol, connection, mapping, target, linking, session, ui, enabled, display_order, catalog_ref, created_at, updated_at FROM providers;
DROP TABLE providers;
ALTER TABLE providers_new RENAME TO providers;

CREATE INDEX idx_providers_instance_org ON providers(instance_id, org_id);
CREATE INDEX idx_providers_instance_protocol ON providers(instance_id, protocol, enabled);
CREATE INDEX idx_providers_instance_sort ON providers(instance_id, display_order, display_name);
CREATE UNIQUE INDEX idx_providers_instance_name_no_org
    ON providers(instance_id, display_name) WHERE org_id IS NULL;

-- ── login_flows ──

CREATE TABLE login_flows_new (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    org_id         TEXT DEFAULT NULL,
    name           TEXT NOT NULL,
    strategy       TEXT NOT NULL DEFAULT 'identifier_first',
    steps          TEXT NOT NULL DEFAULT '[]',
    config         TEXT NOT NULL DEFAULT '{}',
    is_default     BOOLEAN NOT NULL DEFAULT 0,
    enabled        BOOLEAN NOT NULL DEFAULT 1,
    state          TEXT NOT NULL DEFAULT 'draft',
    priority       INTEGER NOT NULL DEFAULT 0,
    audience       TEXT NOT NULL DEFAULT '{}',
    auth_methods   TEXT NOT NULL DEFAULT '{}',
    schema_id      TEXT DEFAULT '',
    metadata       TEXT DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

INSERT INTO login_flows_new (instance_id, id, org_id, name, strategy, steps, config, is_default, enabled, state, priority, audience, auth_methods, schema_id, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, name, strategy, steps, config, is_default, enabled, state, priority, audience, auth_methods, schema_id, metadata, created_at, updated_at FROM login_flows;
DROP TABLE login_flows;
ALTER TABLE login_flows_new RENAME TO login_flows;

CREATE INDEX idx_login_flows_instance_org ON login_flows(instance_id, org_id);
CREATE INDEX idx_login_flows_instance_state ON login_flows(instance_id, state, enabled);
CREATE UNIQUE INDEX idx_login_flows_instance_default
    ON login_flows(instance_id)
    WHERE is_default = 1;

-- ── login_flow_assets ──

CREATE TABLE login_flow_assets_new (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    org_id         TEXT DEFAULT NULL,
    login_flow_id  TEXT NOT NULL,
    slot           TEXT NOT NULL,
    filename       TEXT NOT NULL DEFAULT '',
    content_type   TEXT NOT NULL,
    size_bytes     INTEGER NOT NULL DEFAULT 0,
    sha256         TEXT NOT NULL,
    etag           TEXT NOT NULL,
    data           BLOB NOT NULL,
    metadata       TEXT DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, login_flow_id, slot),
    FOREIGN KEY (instance_id, login_flow_id) REFERENCES login_flows(instance_id, id) ON DELETE CASCADE
);

INSERT INTO login_flow_assets_new (instance_id, id, org_id, login_flow_id, slot, filename, content_type, size_bytes, sha256, etag, data, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, login_flow_id, slot, filename, content_type, size_bytes, sha256, etag, data, metadata, created_at, updated_at FROM login_flow_assets;
DROP TABLE login_flow_assets;
ALTER TABLE login_flow_assets_new RENAME TO login_flow_assets;

CREATE INDEX idx_login_flow_assets_instance_flow
    ON login_flow_assets(instance_id, login_flow_id);

-- ── groups ──

CREATE TABLE groups_new (
    instance_id  TEXT NOT NULL,
    id           TEXT NOT NULL,
    org_id       TEXT DEFAULT NULL,
    name         TEXT NOT NULL,
    description  TEXT DEFAULT '',
    state        TEXT NOT NULL DEFAULT 'active',
    schema_id    TEXT DEFAULT '',
    metadata     TEXT DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, name)
);

INSERT INTO groups_new (instance_id, id, org_id, name, description, state, schema_id, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, name, description, state, schema_id, metadata, created_at, updated_at FROM groups;
DROP TABLE groups;
ALTER TABLE groups_new RENAME TO groups;

CREATE INDEX idx_groups_instance_org ON groups(instance_id, org_id);
CREATE UNIQUE INDEX idx_groups_instance_name_no_org
    ON groups(instance_id, name) WHERE org_id IS NULL;

-- ── projects ──

CREATE TABLE projects_new (
    instance_id  TEXT NOT NULL,
    id           TEXT NOT NULL,
    org_id       TEXT DEFAULT NULL,
    name         TEXT NOT NULL,
    description  TEXT DEFAULT '',
    state        TEXT NOT NULL DEFAULT 'active',
    schema_id    TEXT DEFAULT '',
    metadata     TEXT DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, name)
);

INSERT INTO projects_new (instance_id, id, org_id, name, description, state, schema_id, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, name, description, state, schema_id, metadata, created_at, updated_at FROM projects;
DROP TABLE projects;
ALTER TABLE projects_new RENAME TO projects;

CREATE INDEX idx_projects_instance_org ON projects(instance_id, org_id);
CREATE UNIQUE INDEX idx_projects_instance_name_no_org
    ON projects(instance_id, name) WHERE org_id IS NULL;

-- ── actions ──

CREATE TABLE actions_new (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    org_id        TEXT DEFAULT NULL,
    name          TEXT NOT NULL,
    hook          TEXT NOT NULL DEFAULT 'on_event',
    action_type   TEXT NOT NULL DEFAULT 'expr',
    trigger_expr  TEXT DEFAULT 'true',
    config        TEXT NOT NULL DEFAULT '{}',
    priority      INTEGER NOT NULL DEFAULT 0,
    enabled       BOOLEAN NOT NULL DEFAULT 1,
    fail_open     BOOLEAN NOT NULL DEFAULT 0,
    timeout_ms    INTEGER NOT NULL DEFAULT 5000,
    schema_id     TEXT DEFAULT '',
    metadata      TEXT DEFAULT '{}',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

INSERT INTO actions_new (instance_id, id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, timeout_ms, schema_id, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, timeout_ms, schema_id, metadata, created_at, updated_at FROM actions;
DROP TABLE actions;
ALTER TABLE actions_new RENAME TO actions;

CREATE INDEX idx_actions_instance_org ON actions(instance_id, org_id);
CREATE INDEX idx_actions_instance_hook ON actions(instance_id, hook, enabled);

-- ── domains ──

CREATE TABLE domains_new (
    domain       TEXT PRIMARY KEY,
    instance_id  TEXT NOT NULL,
    org_id       TEXT,
    is_primary   BOOLEAN NOT NULL DEFAULT 0,
    state        TEXT NOT NULL DEFAULT 'active',
    verified     BOOLEAN NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);

INSERT INTO domains_new (domain, instance_id, org_id, is_primary, state, verified, created_at, updated_at)
    SELECT domain, instance_id, org_id, is_primary, state, verified, created_at, updated_at FROM domains;
DROP TABLE domains;
ALTER TABLE domains_new RENAME TO domains;

CREATE INDEX idx_domains_instance ON domains(instance_id);
CREATE INDEX idx_domains_instance_org ON domains(instance_id, org_id);
CREATE UNIQUE INDEX idx_domains_instance_primary
    ON domains(instance_id)
    WHERE org_id IS NULL AND is_primary = 1;
CREATE UNIQUE INDEX idx_domains_org_primary
    ON domains(instance_id, org_id)
    WHERE org_id IS NOT NULL AND is_primary = 1;

-- NOTE: SQLite cannot do column-specific ON DELETE SET NULL for composite FKs,
-- and trigger BEGIN..END blocks break the migration statement splitter.
-- The DeleteOrg use case handles nulling org_id in application code instead.

-- +goose Down
