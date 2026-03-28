-- +goose Up
ALTER TABLE events ADD COLUMN category TEXT NOT NULL DEFAULT '';

UPDATE events SET category = CASE
    WHEN event_type LIKE 'entity.%' THEN 'entity'
    WHEN event_type LIKE 'identity.%' THEN 'entity'
    WHEN event_type LIKE 'provider.%' THEN 'entity'
    WHEN event_type LIKE 'settings.%' THEN 'entity'
    WHEN event_type LIKE 'schema.%' THEN 'entity'
    WHEN event_type LIKE 'auth.%' THEN 'auth'
    WHEN event_type LIKE 'session.%' THEN 'session'
    WHEN event_type LIKE 'token.%' THEN 'token'
    WHEN event_type LIKE 'api.%' THEN 'request'
    WHEN event_type LIKE 'request.%' THEN 'request'
    WHEN event_type LIKE 'log.%' THEN 'log'
    WHEN event_type LIKE 'signal.%' THEN 'signal'
    WHEN event_type LIKE 'threat.%' THEN 'threat'
    WHEN event_type LIKE 'notification.%' THEN 'system'
    ELSE 'system'
END;

UPDATE events SET event_type = 'request.api' WHERE event_type = 'api.request';

CREATE INDEX IF NOT EXISTS idx_events_category ON events(category);

-- +goose Down
DROP INDEX IF EXISTS idx_events_category;
UPDATE events SET event_type = 'api.request' WHERE event_type = 'request.api';
ALTER TABLE events DROP COLUMN category;
