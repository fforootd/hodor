-- +goose Up
-- Canonical providers + dedicated OIDC transient tables.

ALTER TABLE providers ADD COLUMN display_name TEXT;
ALTER TABLE providers ADD COLUMN kind TEXT NOT NULL DEFAULT 'custom';
ALTER TABLE providers ADD COLUMN connection JSONB NOT NULL DEFAULT '{}';
ALTER TABLE providers ADD COLUMN mapping JSONB NOT NULL DEFAULT '{}';
ALTER TABLE providers ADD COLUMN target JSONB NOT NULL DEFAULT '{}';
ALTER TABLE providers ADD COLUMN linking JSONB NOT NULL DEFAULT '{}';
ALTER TABLE providers ADD COLUMN session JSONB NOT NULL DEFAULT '{}';
ALTER TABLE providers ADD COLUMN ui JSONB NOT NULL DEFAULT '{}';
ALTER TABLE providers ADD COLUMN catalog_ref JSONB NOT NULL DEFAULT '{}';

UPDATE providers
SET
    display_name = name,
    kind = COALESCE(NULLIF(template, ''), 'custom'),
    connection = COALESCE(config, '{}'::jsonb),
    mapping = jsonb_build_object('claims', COALESCE(claim_overrides, '{}'::jsonb)),
    target = jsonb_build_object(
        'schema_type', COALESCE(target_schema_type, ''),
        'schema_id', COALESCE(target_schema_id, '')
    ),
    linking = jsonb_build_object(
        'mode', CASE WHEN auto_register THEN 'create_or_link' ELSE 'link_only' END,
        'match_by', 'verified_email'
    ),
    session = '{}'::jsonb,
    ui = jsonb_build_object('display_order', COALESCE(display_order, 0)),
    catalog_ref = COALESCE(metadata->'_catalog', '{}'::jsonb);

ALTER TABLE providers ALTER COLUMN display_name SET NOT NULL;
ALTER TABLE providers DROP CONSTRAINT IF EXISTS providers_instance_org_name_key;
ALTER TABLE providers ADD CONSTRAINT providers_instance_org_display_name_key
    UNIQUE(instance_id, org_id, display_name);
DROP INDEX IF EXISTS idx_providers_instance;
CREATE INDEX idx_providers_instance ON providers(instance_id, org_id);
CREATE INDEX idx_providers_instance_protocol ON providers(instance_id, protocol, enabled);
CREATE INDEX idx_providers_instance_sort ON providers(instance_id, display_order, display_name);

ALTER TABLE providers DROP COLUMN name;
ALTER TABLE providers DROP COLUMN template;
ALTER TABLE providers DROP COLUMN config;
ALTER TABLE providers DROP COLUMN claim_overrides;
ALTER TABLE providers DROP COLUMN auto_register;
ALTER TABLE providers DROP COLUMN schema_id;
ALTER TABLE providers DROP COLUMN target_schema_id;
ALTER TABLE providers DROP COLUMN target_schema_type;
ALTER TABLE providers DROP COLUMN metadata;

CREATE TABLE IF NOT EXISTS oidc_auth_requests (
    id                    TEXT PRIMARY KEY,
    instance_id           TEXT NOT NULL DEFAULT 'default',
    client_id             TEXT NOT NULL,
    redirect_uri          TEXT NOT NULL DEFAULT '',
    scope                 TEXT NOT NULL DEFAULT '',
    state                 TEXT NOT NULL DEFAULT '',
    nonce                 TEXT NOT NULL DEFAULT '',
    response_type         TEXT NOT NULL DEFAULT 'code',
    code_challenge        TEXT NOT NULL DEFAULT '',
    code_challenge_method TEXT NOT NULL DEFAULT '',
    prompt                JSONB NOT NULL DEFAULT '[]',
    login_hint            TEXT NOT NULL DEFAULT '',
    user_id               TEXT NOT NULL DEFAULT '',
    code                  TEXT NOT NULL DEFAULT '',
    done                  BOOLEAN NOT NULL DEFAULT FALSE,
    auth_time             TIMESTAMPTZ,
    expires_at            TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes'),
    created_at            TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX idx_oidc_auth_requests_instance ON oidc_auth_requests(instance_id, created_at);
CREATE INDEX idx_oidc_auth_requests_code ON oidc_auth_requests(instance_id, code) WHERE code != '';
CREATE INDEX idx_oidc_auth_requests_client ON oidc_auth_requests(instance_id, client_id);

CREATE TABLE IF NOT EXISTS oidc_rp_auth_states (
    id              TEXT PRIMARY KEY,
    instance_id     TEXT NOT NULL DEFAULT 'default',
    provider_id     TEXT NOT NULL DEFAULT '',
    state           TEXT NOT NULL,
    nonce           TEXT NOT NULL DEFAULT '',
    pkce_verifier   TEXT NOT NULL DEFAULT '',
    flow_id         TEXT NOT NULL DEFAULT '',
    redirect_uri    TEXT NOT NULL DEFAULT '',
    expected_issuer TEXT NOT NULL DEFAULT '',
    callback_uri    TEXT NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at      TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes')
);
CREATE UNIQUE INDEX idx_oidc_rp_auth_states_state ON oidc_rp_auth_states(instance_id, state);
CREATE INDEX idx_oidc_rp_auth_states_provider ON oidc_rp_auth_states(instance_id, provider_id);

DELETE FROM auth_states WHERE type IN ('oidc_auth', 'sso');

-- +goose Down
DROP TABLE IF EXISTS oidc_rp_auth_states;
DROP TABLE IF EXISTS oidc_auth_requests;
