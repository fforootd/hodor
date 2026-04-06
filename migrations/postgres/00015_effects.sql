-- +goose Up

CREATE TABLE IF NOT EXISTS effects (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    event_id      TEXT NOT NULL DEFAULT '',
    effect_type   TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending',
    config        JSONB NOT NULL DEFAULT '{}',
    payload       JSONB NOT NULL DEFAULT '{}',
    attempt       INTEGER NOT NULL DEFAULT 0,
    max_attempts  INTEGER NOT NULL DEFAULT 5,
    next_retry_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_error    TEXT NOT NULL DEFAULT '',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    completed_at  TIMESTAMPTZ,
    PRIMARY KEY (instance_id, id)
);

CREATE INDEX IF NOT EXISTS idx_effects_pending
    ON effects(status, next_retry_at)
    WHERE status IN ('pending', 'failed');

CREATE INDEX IF NOT EXISTS idx_effects_event
    ON effects(instance_id, event_id);

-- +goose Down

DROP INDEX IF EXISTS idx_effects_event;
DROP INDEX IF EXISTS idx_effects_pending;
DROP TABLE IF EXISTS effects;
