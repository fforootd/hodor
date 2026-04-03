-- +goose Up
-- Composite index for event queries filtered by instance + type + time ordering.
-- Supports the common pattern: WHERE instance_id = ? AND event_type NOT LIKE 'log.%' ORDER BY created_at DESC
CREATE INDEX IF NOT EXISTS idx_events_instance_type_created ON events(instance_id, event_type, created_at DESC);

-- +goose Down
DROP INDEX IF EXISTS idx_events_instance_type_created;
