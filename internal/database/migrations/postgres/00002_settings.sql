-- +goose Up
-- Hierarchical settings — cascading configuration (ADR-009)
CREATE TABLE IF NOT EXISTS settings (
    id         TEXT PRIMARY KEY,
    type       TEXT NOT NULL,
    scope      TEXT NOT NULL DEFAULT 'instance',
    scope_id   TEXT NOT NULL DEFAULT '',
    data       JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(type, scope, scope_id)
);
CREATE INDEX IF NOT EXISTS idx_settings_type ON settings(type, scope);

-- +goose Down
DROP INDEX IF EXISTS idx_settings_type;
DROP TABLE IF EXISTS settings;
