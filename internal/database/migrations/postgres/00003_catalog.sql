-- +goose Up
-- Catalog cache for offline restarts (ADR-015).
-- Stores the last-known remote catalog index and individual template payloads.
CREATE TABLE IF NOT EXISTS catalog_cache (
    key        TEXT PRIMARY KEY,
    data       TEXT NOT NULL,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- +goose Down
DROP TABLE IF EXISTS catalog_cache;
