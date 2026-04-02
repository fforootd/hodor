-- +goose Up
-- Add instance_id column to all tenant-scoped tables.
-- Default 'default' means existing data and self-hosted deployments
-- require zero migration effort.

-- ── Add instance_id column to all scoped tables ────────────────────────

ALTER TABLE orgs ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE users ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE credentials ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE providers ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE apps ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE actions ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE login_flows ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE login_flow_assets ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE linked_identities ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE groups ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE projects ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE memberships ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE domains ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE sessions ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE tokens ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE auth_states ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE secrets ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE events ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE fingerprints ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE unique_fields ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE settings ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE saved_queries ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE jobs ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE cache ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE consumer_cursors ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE retention_policies ADD COLUMN instance_id TEXT NOT NULL DEFAULT 'default';

-- ── Composite indexes ──────────────────────────────────────────────────

CREATE INDEX idx_orgs_instance ON orgs(instance_id);
CREATE INDEX idx_users_instance ON users(instance_id, org_id);
CREATE INDEX idx_users_instance_identifier ON users(instance_id, org_id, identifier);
CREATE INDEX idx_users_instance_state ON users(instance_id, state);
CREATE INDEX idx_users_instance_type ON users(instance_id, user_type);
CREATE INDEX idx_credentials_instance ON credentials(instance_id, user_id);
CREATE INDEX idx_providers_instance ON providers(instance_id, org_id);
CREATE INDEX idx_apps_instance ON apps(instance_id);
CREATE INDEX idx_apps_instance_client ON apps(instance_id, client_id);
CREATE INDEX idx_apps_instance_org ON apps(instance_id, org_id);
CREATE INDEX idx_actions_instance ON actions(instance_id, org_id);
CREATE INDEX idx_login_flows_instance ON login_flows(instance_id, org_id);
CREATE INDEX idx_linked_identities_instance ON linked_identities(instance_id);
CREATE INDEX idx_groups_instance ON groups(instance_id, org_id);
CREATE INDEX idx_projects_instance ON projects(instance_id, org_id);
CREATE INDEX idx_memberships_instance ON memberships(instance_id);
CREATE INDEX idx_domains_instance ON domains(instance_id);
CREATE INDEX idx_domains_instance_org ON domains(instance_id, org_id);
CREATE INDEX idx_sessions_instance ON sessions(instance_id);
CREATE INDEX idx_tokens_instance ON tokens(instance_id);
CREATE INDEX idx_tokens_instance_hash ON tokens(instance_id, token_hash);
CREATE INDEX idx_auth_states_instance ON auth_states(instance_id);
CREATE INDEX idx_secrets_instance ON secrets(instance_id);
CREATE INDEX idx_events_instance ON events(instance_id, created_at);
CREATE INDEX idx_fingerprints_instance ON fingerprints(instance_id);
CREATE INDEX idx_unique_fields_instance ON unique_fields(instance_id, scope_id, field_name);
CREATE INDEX idx_unique_fields_instance_resource ON unique_fields(instance_id, user_id);
CREATE INDEX idx_unique_fields_instance_lookup ON unique_fields(instance_id, normalized_value, field_name);
CREATE INDEX idx_settings_instance ON settings(instance_id, type, scope, scope_id);
CREATE INDEX idx_jobs_instance ON jobs(instance_id);
CREATE INDEX idx_cache_instance ON cache(instance_id);
CREATE INDEX idx_jobs_instance_next_run ON jobs(instance_id, enabled, next_run_at);
CREATE INDEX idx_cache_instance_expires ON cache(instance_id, expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX idx_consumer_cursors_instance ON consumer_cursors(instance_id);
CREATE INDEX idx_retention_policies_instance ON retention_policies(instance_id, priority DESC);

-- ── Unique constraint upgrades ─────────────────────────────────────────
-- Postgres supports ALTER TABLE ... DROP CONSTRAINT / ADD CONSTRAINT.

-- users: UNIQUE(org_id, identifier) → UNIQUE(instance_id, org_id, identifier)
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_org_id_identifier_key;
ALTER TABLE users ADD CONSTRAINT users_instance_org_identifier_key UNIQUE(instance_id, org_id, identifier);

-- providers: UNIQUE(org_id, name) → UNIQUE(instance_id, org_id, name)
ALTER TABLE providers DROP CONSTRAINT IF EXISTS providers_org_id_name_key;
ALTER TABLE providers ADD CONSTRAINT providers_instance_org_name_key UNIQUE(instance_id, org_id, name);

-- apps: UNIQUE(client_id) → UNIQUE(instance_id, client_id)
ALTER TABLE apps DROP CONSTRAINT IF EXISTS apps_client_id_key;
ALTER TABLE apps ADD CONSTRAINT apps_instance_client_id_key UNIQUE(instance_id, client_id);

-- domains: UNIQUE(domain) → UNIQUE(instance_id, domain)
ALTER TABLE domains DROP CONSTRAINT IF EXISTS domains_domain_key;
ALTER TABLE domains ADD CONSTRAINT domains_instance_domain_key UNIQUE(instance_id, domain);

-- groups: UNIQUE(org_id, name) → UNIQUE(instance_id, org_id, name)
ALTER TABLE groups DROP CONSTRAINT IF EXISTS groups_org_id_name_key;
ALTER TABLE groups ADD CONSTRAINT groups_instance_org_name_key UNIQUE(instance_id, org_id, name);

-- projects: UNIQUE(org_id, name) → UNIQUE(instance_id, org_id, name)
ALTER TABLE projects DROP CONSTRAINT IF EXISTS projects_org_id_name_key;
ALTER TABLE projects ADD CONSTRAINT projects_instance_org_name_key UNIQUE(instance_id, org_id, name);

-- unique_fields: UNIQUE(scope_id, field_name, normalized_value) → UNIQUE(instance_id, ...)
ALTER TABLE unique_fields DROP CONSTRAINT IF EXISTS unique_fields_scope_id_field_name_normalized_value_key;
ALTER TABLE unique_fields ADD CONSTRAINT unique_fields_instance_scope_field_value_key UNIQUE(instance_id, scope_id, field_name, normalized_value);

-- settings: UNIQUE(type, scope, scope_id) → UNIQUE(instance_id, type, scope, scope_id)
ALTER TABLE settings DROP CONSTRAINT IF EXISTS settings_type_scope_scope_id_key;
ALTER TABLE settings ADD CONSTRAINT settings_instance_type_scope_key UNIQUE(instance_id, type, scope, scope_id);

-- jobs/cache/consumer_cursors/retention_policies: allow per-instance rows
ALTER TABLE jobs DROP CONSTRAINT IF EXISTS jobs_pkey;
ALTER TABLE jobs ADD CONSTRAINT jobs_pkey PRIMARY KEY(instance_id, name);

ALTER TABLE cache DROP CONSTRAINT IF EXISTS cache_pkey;
ALTER TABLE cache ADD CONSTRAINT cache_pkey PRIMARY KEY(instance_id, namespace, key);

ALTER TABLE consumer_cursors DROP CONSTRAINT IF EXISTS consumer_cursors_pkey;
ALTER TABLE consumer_cursors ADD CONSTRAINT consumer_cursors_pkey PRIMARY KEY(instance_id, consumer_name);

ALTER TABLE retention_policies DROP CONSTRAINT IF EXISTS retention_policies_pkey;
ALTER TABLE retention_policies ADD CONSTRAINT retention_policies_pkey PRIMARY KEY(instance_id, id);

-- ── FGA instance store mapping ─────────────────────────────────────────
CREATE TABLE IF NOT EXISTS fga_instance_stores (
    instance_id TEXT PRIMARY KEY,
    store_id    TEXT NOT NULL
);

-- +goose Down
DROP TABLE IF EXISTS fga_instance_stores;
