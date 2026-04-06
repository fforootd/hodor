-- +goose Up

CREATE TABLE IF NOT EXISTS effects (
    instance_id       STRING(MAX) NOT NULL,
    id                STRING(MAX) NOT NULL,
    event_id          STRING(MAX) NOT NULL DEFAULT '',
    source_key        STRING(MAX) NOT NULL,
    effect_type       STRING(MAX) NOT NULL,
    status            STRING(MAX) NOT NULL DEFAULT 'pending',
    config            STRING(MAX) NOT NULL DEFAULT ('{}'),
    payload           STRING(MAX) NOT NULL DEFAULT ('{}'),
    attempt           INT64 NOT NULL DEFAULT 0,
    max_attempts      INT64 NOT NULL DEFAULT 5,
    next_retry_at     TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    last_error        STRING(MAX) NOT NULL DEFAULT '',
    lease_owner       STRING(MAX) NOT NULL DEFAULT '',
    lease_expires_at  TIMESTAMP,
    created_at        TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    completed_at      TIMESTAMP,
    PRIMARY KEY (instance_id, id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_effects_source_key
    ON effects(instance_id, source_key);
CREATE INDEX IF NOT EXISTS idx_effects_due
    ON effects(instance_id, status, next_retry_at, lease_expires_at);
CREATE INDEX IF NOT EXISTS idx_effects_event
    ON effects(instance_id, event_id);
CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_effects_cleanup
    ON effects(instance_id, status, completed_at);

-- +goose Down
