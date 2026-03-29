-- +goose Up

-- Step 1: Ensure we have a provider schema in schemas table.
INSERT INTO schemas (id, type, org_id, schema, version, is_default, message, created_by, created_at)
VALUES (
  'provider_v1',
  'provider',
  '',
  '{"$schema":"https://zitadel.com/schemas/v1/entity","$version":"1.0","type":"object","description":"External identity provider","x-storage":"entities","x-display":{"alias":"Providers","path":"providers","icon":"shield"}}',
  1,
  true,
  'Initial provider schema (migrated from dedicated table)',
  'migration',
  NOW()
)
ON CONFLICT (id) DO NOTHING;

-- Step 2: Migrate existing providers → entities.
INSERT INTO entities (id, org_id, identifier, display_name, state, schema_id, data, created_at, updated_at)
SELECT
  id,
  CAST(org_id AS TEXT),
  name,
  name,
  CASE WHEN enabled THEN 'active' ELSE 'inactive' END,
  'provider_v1',
  jsonb_build_object(
    'protocol', protocol,
    'template', template,
    'config', config::jsonb,
    'claim_overrides', claim_overrides::jsonb,
    'auto_register', auto_register,
    'enabled', enabled,
    'display_order', display_order
  ),
  created_at,
  updated_at
FROM providers
ON CONFLICT (id) DO NOTHING;

-- Step 3: Drop providers table.
DROP TABLE IF EXISTS providers;

-- Step 4: Migrate notification_templates → settings cascade.
INSERT INTO settings (id, type, scope, scope_id, data, created_at, updated_at)
SELECT
  CAST(id AS TEXT),
  'notification_template_' || channel || '_' || event,
  CASE WHEN org_id IS NULL OR org_id = '' THEN 'instance' ELSE 'org' END,
  COALESCE(org_id, ''),
  jsonb_build_object('language', language, 'subject', subject, 'body', body),
  NOW(),
  NOW()
FROM notification_templates
ON CONFLICT DO NOTHING;

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
    config        JSONB NOT NULL DEFAULT '{}',
    claim_overrides JSONB NOT NULL DEFAULT '{}',
    auto_register BOOLEAN NOT NULL DEFAULT true,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    display_order INTEGER NOT NULL DEFAULT 0,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Recreate notification_templates table.
CREATE TABLE IF NOT EXISTS notification_templates (
    id       SERIAL PRIMARY KEY,
    org_id   TEXT,
    channel  TEXT NOT NULL DEFAULT 'email',
    event    TEXT NOT NULL DEFAULT 'welcome',
    language TEXT NOT NULL DEFAULT 'en',
    subject  TEXT NOT NULL DEFAULT '',
    body     TEXT NOT NULL DEFAULT ''
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_notif_tpl_unique ON notification_templates(org_id, channel, event, language);
