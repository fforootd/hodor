-- +goose Up

-- Step 1: Ensure we have a provider schema in schemas table.
INSERT OR IGNORE INTO schemas (id, type, org_id, schema, version, is_default, message, created_by, created_at)
VALUES (
  'provider_v1',
  'provider',
  '',
  '{"$schema":"https://zitadel.com/schemas/v1/entity","$version":"1.0","type":"object","description":"External identity provider","x-storage":"entities","x-display":{"alias":"Providers","path":"providers","icon":"shield"}}',
  1,
  true,
  'Initial provider schema (migrated from dedicated table)',
  'migration',
  datetime('now')
);

-- Step 2: Migrate existing providers → entities.
-- Map provider columns into entity data JSONB.
INSERT OR IGNORE INTO entities (id, org_id, identifier, display_name, state, schema_id, data, created_at, updated_at)
SELECT
  id,
  CAST(org_id AS TEXT),
  name,
  name,
  CASE WHEN enabled = 1 THEN 'active' ELSE 'inactive' END,
  'provider_v1',
  json_object(
    'protocol', protocol,
    'template', template,
    'config', json(config),
    'claim_overrides', json(claim_overrides),
    'auto_register', CASE WHEN auto_register = 1 THEN json('true') ELSE json('false') END,
    'enabled', CASE WHEN enabled = 1 THEN json('true') ELSE json('false') END,
    'display_order', display_order
  ),
  created_at,
  updated_at
FROM providers;

-- Step 3: Drop providers table.
DROP TABLE IF EXISTS providers;

-- Step 4: Migrate notification_templates → settings cascade.
INSERT OR IGNORE INTO settings (id, type, scope, scope_id, data, created_at, updated_at)
SELECT
  CAST(id AS TEXT),
  'notification_template_' || channel || '_' || event,
  CASE WHEN org_id IS NULL OR org_id = '' THEN 'instance' ELSE 'org' END,
  COALESCE(org_id, ''),
  json_object('language', language, 'subject', subject, 'body', body),
  datetime('now'),
  datetime('now')
FROM notification_templates;

-- Step 5: Drop notification_templates.
DROP TABLE IF EXISTS notification_templates;

-- +goose Down

-- Recreate providers table.
CREATE TABLE IF NOT EXISTS providers (
    id            TEXT PRIMARY KEY,
    org_id        INTEGER NOT NULL DEFAULT 1,
    name          TEXT NOT NULL,
    protocol      TEXT NOT NULL DEFAULT 'oidc',
    template      TEXT NOT NULL DEFAULT 'custom',
    config        TEXT NOT NULL DEFAULT '{}',
    claim_overrides TEXT NOT NULL DEFAULT '{}',
    auto_register BOOLEAN NOT NULL DEFAULT true,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    display_order INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Recreate notification_templates table.
CREATE TABLE IF NOT EXISTS notification_templates (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    org_id   TEXT,
    channel  TEXT NOT NULL DEFAULT 'email',
    event    TEXT NOT NULL DEFAULT 'welcome',
    language TEXT NOT NULL DEFAULT 'en',
    subject  TEXT NOT NULL DEFAULT '',
    body     TEXT NOT NULL DEFAULT '',
    UNIQUE(org_id, channel, event, language)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_notif_tpl_unique ON notification_templates(org_id, channel, event, language);
