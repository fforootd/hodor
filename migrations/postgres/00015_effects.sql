-- +goose Up

CREATE TABLE IF NOT EXISTS effects (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    event_id      TEXT NOT NULL DEFAULT '',
    source_key    TEXT NOT NULL,
    effect_type   TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending',
    config        JSONB NOT NULL DEFAULT '{}',
    payload       JSONB NOT NULL DEFAULT '{}',
    attempt       INTEGER NOT NULL DEFAULT 0,
    max_attempts  INTEGER NOT NULL DEFAULT 5,
    next_retry_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_error    TEXT NOT NULL DEFAULT '',
    lease_owner   TEXT NOT NULL DEFAULT '',
    lease_expires_at TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at  TIMESTAMPTZ,
    PRIMARY KEY (instance_id, id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_effects_source_key
    ON effects(instance_id, source_key);

CREATE INDEX IF NOT EXISTS idx_effects_due
    ON effects(instance_id, status, next_retry_at, lease_expires_at);

CREATE INDEX IF NOT EXISTS idx_effects_event
    ON effects(instance_id, event_id);

CREATE INDEX IF NOT EXISTS idx_effects_cleanup
    ON effects(instance_id, status, completed_at);

-- +goose Down

DROP INDEX IF EXISTS idx_effects_cleanup;
DROP INDEX IF EXISTS idx_effects_event;
DROP INDEX IF EXISTS idx_effects_due;
DROP INDEX IF EXISTS idx_effects_source_key;
DROP TABLE IF EXISTS effects;
