-- +goose Up
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS lease_owner TEXT NOT NULL DEFAULT '';
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ;
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP;
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS last_rows_removed BIGINT NOT NULL DEFAULT 0;

UPDATE jobs
SET updated_at = COALESCE(last_run_at, next_run_at, created_at, CURRENT_TIMESTAMP)
WHERE updated_at IS NULL;

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

ALTER TABLE events RENAME TO events_legacy;

CREATE TABLE events (
    id              TEXT NOT NULL,
    instance_id     TEXT NOT NULL DEFAULT 'default',
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
    request_id      TEXT,
    session_id      TEXT,
    flow_id         TEXT,
    fingerprint     TEXT DEFAULT '',
    client_id       TEXT DEFAULT '',
    token_id        TEXT DEFAULT '',
    delegation_type TEXT DEFAULT '',
    sdk_name        TEXT DEFAULT '',
    sdk_version     TEXT DEFAULT '',
    sequence        BIGINT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    shipped_at      TIMESTAMPTZ,
    PRIMARY KEY (instance_id, created_at, id)
) PARTITION BY RANGE (created_at);

CREATE TABLE events_default PARTITION OF events DEFAULT;

INSERT INTO events (
    id, instance_id, event_type, category, org_id, actor_id, actor_type,
    aggregate_id, aggregate_type, resource_type, payload, metadata,
    request_id, session_id, flow_id, fingerprint, client_id, token_id,
    delegation_type, sdk_name, sdk_version, sequence, created_at, shipped_at
)
SELECT
    id, instance_id, event_type, category, org_id, actor_id, actor_type,
    aggregate_id, aggregate_type, resource_type, payload, metadata,
    request_id, session_id, flow_id, fingerprint, client_id, token_id,
    delegation_type, sdk_name, sdk_version, sequence, created_at, shipped_at
FROM events_legacy;

DROP TABLE events_legacy;

CREATE INDEX idx_events_type ON events(event_type);
CREATE INDEX idx_events_aggregate ON events(aggregate_id, aggregate_type);
CREATE INDEX idx_events_request ON events(request_id) WHERE request_id IS NOT NULL;
CREATE INDEX idx_events_ship ON events(shipped_at) WHERE shipped_at IS NULL;
CREATE INDEX idx_events_category ON events(category, created_at);
CREATE INDEX idx_events_actor ON events(actor_id) WHERE actor_id IS NOT NULL;
CREATE INDEX idx_events_flow ON events(flow_id) WHERE flow_id IS NOT NULL;
CREATE INDEX idx_events_org ON events(org_id, created_at);
CREATE INDEX idx_events_client ON events(client_id) WHERE client_id != '';
CREATE INDEX idx_events_delegation ON events(delegation_type) WHERE delegation_type != '';
CREATE INDEX idx_events_instance ON events(instance_id, created_at);

-- +goose Down
