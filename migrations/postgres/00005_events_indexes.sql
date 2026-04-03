-- +goose Up
CREATE INDEX IF NOT EXISTS idx_events_instance_type_created ON events(instance_id, event_type, created_at DESC);

-- +goose Down
DROP INDEX IF EXISTS idx_events_instance_type_created;
