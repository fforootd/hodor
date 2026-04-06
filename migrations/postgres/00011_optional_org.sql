-- Make org_id optional on users and sessions.
-- Deleting an org now sets org_id to NULL instead of cascade-deleting rows.

-- ── users ──
ALTER TABLE users ALTER COLUMN org_id DROP NOT NULL;
ALTER TABLE users ALTER COLUMN org_id SET DEFAULT NULL;
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_instance_id_org_id_fkey;
ALTER TABLE users ADD CONSTRAINT users_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;
-- Ensure identifier uniqueness for org-less users.
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_instance_identifier_no_org
    ON users(instance_id, identifier) WHERE org_id IS NULL;

-- ── sessions ──
ALTER TABLE sessions ALTER COLUMN org_id DROP NOT NULL;
ALTER TABLE sessions ALTER COLUMN org_id SET DEFAULT NULL;
ALTER TABLE sessions DROP CONSTRAINT IF EXISTS sessions_instance_id_org_id_fkey;
ALTER TABLE sessions ADD CONSTRAINT sessions_instance_id_org_id_fkey
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;
