-- +goose Up
-- Change all org FK constraints from CASCADE to SET NULL (org_id only).
-- Orgs are relationships, not ownership — deleting one should orphan resources, not destroy them.
-- Also applies the 00011 fix for users/sessions that was a no-op due to missing goose marker.
-- Uses column-specific SET NULL so only org_id is nulled, not instance_id.

-- ── users (00011 fix) ──
ALTER TABLE users ALTER COLUMN org_id DROP NOT NULL;
ALTER TABLE users ALTER COLUMN org_id SET DEFAULT NULL;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_instance_id_org_id_fkey;
ALTER TABLE users ADD CONSTRAINT users_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL (org_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_instance_identifier_no_org
    ON users(instance_id, identifier) WHERE org_id IS NULL;

-- ── sessions (00011 fix) ──
ALTER TABLE sessions ALTER COLUMN org_id DROP NOT NULL;
ALTER TABLE sessions ALTER COLUMN org_id SET DEFAULT NULL;
ALTER TABLE sessions DROP CONSTRAINT IF EXISTS sessions_instance_id_org_id_fkey;
ALTER TABLE sessions ADD CONSTRAINT sessions_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL (org_id);

-- ── apps ──
ALTER TABLE apps ALTER COLUMN org_id DROP NOT NULL;
ALTER TABLE apps ALTER COLUMN org_id SET DEFAULT NULL;
ALTER TABLE apps DROP CONSTRAINT IF EXISTS apps_instance_id_org_id_fkey;
ALTER TABLE apps ADD CONSTRAINT apps_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL (org_id);

-- ── providers ──
ALTER TABLE providers ALTER COLUMN org_id DROP NOT NULL;
ALTER TABLE providers ALTER COLUMN org_id SET DEFAULT NULL;
ALTER TABLE providers DROP CONSTRAINT IF EXISTS providers_instance_id_org_id_fkey;
ALTER TABLE providers ADD CONSTRAINT providers_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL (org_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_instance_name_no_org
    ON providers(instance_id, display_name) WHERE org_id IS NULL;

-- ── login_flows ──
ALTER TABLE login_flows DROP CONSTRAINT IF EXISTS login_flows_instance_id_org_id_fkey;
ALTER TABLE login_flows ADD CONSTRAINT login_flows_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL (org_id);

-- ── login_flow_assets ──
ALTER TABLE login_flow_assets ALTER COLUMN org_id DROP NOT NULL;
ALTER TABLE login_flow_assets ALTER COLUMN org_id SET DEFAULT NULL;
ALTER TABLE login_flow_assets DROP CONSTRAINT IF EXISTS login_flow_assets_instance_id_org_id_fkey;
ALTER TABLE login_flow_assets ADD CONSTRAINT login_flow_assets_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL (org_id);

-- ── groups ──
ALTER TABLE groups ALTER COLUMN org_id DROP NOT NULL;
ALTER TABLE groups ALTER COLUMN org_id SET DEFAULT NULL;
ALTER TABLE groups DROP CONSTRAINT IF EXISTS groups_instance_id_org_id_fkey;
ALTER TABLE groups ADD CONSTRAINT groups_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL (org_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_groups_instance_name_no_org
    ON groups(instance_id, name) WHERE org_id IS NULL;

-- ── projects ──
ALTER TABLE projects ALTER COLUMN org_id DROP NOT NULL;
ALTER TABLE projects ALTER COLUMN org_id SET DEFAULT NULL;
ALTER TABLE projects DROP CONSTRAINT IF EXISTS projects_instance_id_org_id_fkey;
ALTER TABLE projects ADD CONSTRAINT projects_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL (org_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_instance_name_no_org
    ON projects(instance_id, name) WHERE org_id IS NULL;

-- ── actions ──
ALTER TABLE actions ALTER COLUMN org_id DROP NOT NULL;
ALTER TABLE actions ALTER COLUMN org_id SET DEFAULT NULL;
ALTER TABLE actions DROP CONSTRAINT IF EXISTS actions_instance_id_org_id_fkey;
ALTER TABLE actions ADD CONSTRAINT actions_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL (org_id);

-- ── domains ──
ALTER TABLE domains DROP CONSTRAINT IF EXISTS domains_instance_id_org_id_fkey;
ALTER TABLE domains ADD CONSTRAINT domains_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL (org_id);

-- +goose Down
