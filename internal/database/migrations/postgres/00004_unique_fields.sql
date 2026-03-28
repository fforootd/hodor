-- +goose Up
-- Schema-driven uniqueness constraints (ADR-016).
-- Stores normalized unique field values for schema-driven enforcement.
-- Uniqueness is cross-type: an email is unique regardless of entity type.

CREATE TABLE IF NOT EXISTS unique_fields (
    scope_id         TEXT NOT NULL DEFAULT '',
    field_name       TEXT NOT NULL,
    normalized_value TEXT NOT NULL,
    entity_id        TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    UNIQUE(scope_id, field_name, normalized_value)
);

CREATE INDEX IF NOT EXISTS idx_unique_fields_entity ON unique_fields(entity_id);
CREATE INDEX IF NOT EXISTS idx_unique_fields_lookup ON unique_fields(normalized_value, field_name);

-- +goose Down
DROP TABLE IF EXISTS unique_fields;
