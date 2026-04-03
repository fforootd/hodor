-- +goose Up
ALTER TABLE jobs ADD COLUMN lease_owner TEXT NOT NULL DEFAULT '';
ALTER TABLE jobs ADD COLUMN lease_expires_at TEXT;
ALTER TABLE jobs ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
ALTER TABLE jobs ADD COLUMN last_rows_removed INTEGER NOT NULL DEFAULT 0;

UPDATE jobs
SET updated_at = CASE
    WHEN updated_at = '' THEN COALESCE(last_run_at, next_run_at, created_at, datetime('now'))
    ELSE updated_at
END;

DELETE FROM jobs WHERE name IN ('lake_writer', 'event_gc');

CREATE INDEX IF NOT EXISTS idx_jobs_instance_due_lease
    ON jobs(instance_id, enabled, next_run_at, lease_expires_at);

CREATE INDEX IF NOT EXISTS idx_sessions_instance_expires
    ON sessions(instance_id, expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sessions_instance_revoked
    ON sessions(instance_id, revoked_at) WHERE revoked_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_tokens_instance_expires
    ON tokens(instance_id, expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tokens_instance_revoked
    ON tokens(instance_id, revoked_at) WHERE revoked_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_auth_states_instance_expires
    ON auth_states(instance_id, expires_at);
CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_expires
    ON oidc_auth_requests(instance_id, expires_at);
CREATE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_instance_expires
    ON oidc_rp_auth_states(instance_id, expires_at);

-- +goose Down
