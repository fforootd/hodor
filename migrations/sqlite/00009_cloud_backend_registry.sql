-- +goose Up
CREATE TABLE IF NOT EXISTS cloud_backends (
    backend_key      TEXT PRIMARY KEY,
    kind             TEXT NOT NULL DEFAULT 'stateful',
    url              TEXT NOT NULL DEFAULT '',
    secret_ref       TEXT NOT NULL DEFAULT '',
    region_key       TEXT,
    state            TEXT NOT NULL DEFAULT 'active',
    global_default   INTEGER NOT NULL DEFAULT 0,
    created_at       TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at       TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_cloud_backends_region ON cloud_backends(region_key);
CREATE INDEX IF NOT EXISTS idx_cloud_backends_state ON cloud_backends(state);

INSERT OR IGNORE INTO cloud_backends (backend_key, kind, state, global_default)
VALUES ('default', 'stateful', 'active', 1);

-- +goose Down
DROP TABLE IF EXISTS cloud_backends;
