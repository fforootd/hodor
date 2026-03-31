-- +goose Up
CREATE TABLE IF NOT EXISTS notification_requests (
    id                 TEXT PRIMARY KEY,
    org_id             TEXT NOT NULL DEFAULT '0',
    aggregate_id       TEXT DEFAULT '',
    aggregate_type     TEXT DEFAULT '',
    event_type         TEXT NOT NULL DEFAULT 'notification.requested',
    medium             TEXT NOT NULL,
    channel_id         TEXT NOT NULL DEFAULT '',
    recipient          TEXT NOT NULL,
    template_key       TEXT NOT NULL,
    locale             TEXT NOT NULL DEFAULT '',
    state              TEXT NOT NULL DEFAULT 'pending',
    attempts           INTEGER NOT NULL DEFAULT 0,
    max_attempts       INTEGER NOT NULL DEFAULT 3,
    last_error         TEXT NOT NULL DEFAULT '',
    payload_ciphertext BYTEA NOT NULL,
    payload_nonce      BYTEA,
    payload_key_id     TEXT NOT NULL DEFAULT '',
    next_attempt_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_attempt_at    TIMESTAMPTZ,
    sent_at            TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_notification_requests_state_next
    ON notification_requests(state, next_attempt_at);
CREATE INDEX IF NOT EXISTS idx_notification_requests_org_created
    ON notification_requests(org_id, created_at);
CREATE INDEX IF NOT EXISTS idx_notification_requests_template
    ON notification_requests(template_key, created_at);

-- +goose Down
DROP INDEX IF EXISTS idx_notification_requests_template;
DROP INDEX IF EXISTS idx_notification_requests_org_created;
DROP INDEX IF EXISTS idx_notification_requests_state_next;
DROP TABLE IF EXISTS notification_requests;
