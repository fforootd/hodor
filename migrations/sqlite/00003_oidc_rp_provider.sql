-- +goose Up
-- Canonical providers + dedicated OIDC transient tables.

CREATE TABLE providers_new (
    id              TEXT PRIMARY KEY,
    instance_id     TEXT NOT NULL DEFAULT 'default',
    org_id          TEXT NOT NULL DEFAULT '1',
    display_name    TEXT NOT NULL,
    kind            TEXT NOT NULL DEFAULT 'custom',
    protocol        TEXT NOT NULL DEFAULT 'oidc',
    connection      TEXT NOT NULL DEFAULT '{}',
    mapping         TEXT NOT NULL DEFAULT '{}',
    target          TEXT NOT NULL DEFAULT '{}',
    linking         TEXT NOT NULL DEFAULT '{}',
    session         TEXT NOT NULL DEFAULT '{}',
    ui              TEXT NOT NULL DEFAULT '{}',
    enabled         BOOLEAN NOT NULL DEFAULT 1,
    display_order   INTEGER NOT NULL DEFAULT 0,
    catalog_ref     TEXT NOT NULL DEFAULT '{}',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(instance_id, org_id, display_name)
);

INSERT INTO providers_new (
    id,
    instance_id,
    org_id,
    display_name,
    kind,
    protocol,
    connection,
    mapping,
    target,
    linking,
    session,
    ui,
    enabled,
    display_order,
    catalog_ref,
    created_at,
    updated_at
)
SELECT
    id,
    instance_id,
    org_id,
    name,
    COALESCE(NULLIF(template, ''), 'custom'),
    protocol,
    COALESCE(config, '{}'),
    json_object('claims', json(COALESCE(NULLIF(claim_overrides, ''), '{}'))),
    json_object(
        'schema_type', COALESCE(target_schema_type, ''),
        'schema_id', COALESCE(target_schema_id, '')
    ),
    json_object(
        'mode', CASE WHEN auto_register THEN 'create_or_link' ELSE 'link_only' END,
        'match_by', 'verified_email'
    ),
    '{}',
    json_object('display_order', COALESCE(display_order, 0)),
    enabled,
    COALESCE(display_order, 0),
    COALESCE(json_extract(metadata, '$._catalog'), '{}'),
    created_at,
    updated_at
FROM providers;

DROP TABLE providers;
ALTER TABLE providers_new RENAME TO providers;
CREATE INDEX idx_providers_instance ON providers(instance_id, org_id);
CREATE INDEX idx_providers_instance_protocol ON providers(instance_id, protocol, enabled);
CREATE INDEX idx_providers_instance_sort ON providers(instance_id, display_order, display_name);

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
    prompt                TEXT NOT NULL DEFAULT '[]',
    login_hint            TEXT NOT NULL DEFAULT '',
    user_id               TEXT NOT NULL DEFAULT '',
    code                  TEXT NOT NULL DEFAULT '',
    done                  INTEGER NOT NULL DEFAULT 0,
    auth_time             TEXT,
    expires_at            TEXT NOT NULL DEFAULT (datetime('now', '+10 minutes')),
    created_at            TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX idx_oidc_auth_requests_instance ON oidc_auth_requests(instance_id, created_at);
CREATE INDEX idx_oidc_auth_requests_code ON oidc_auth_requests(instance_id, code) WHERE code != '';
CREATE INDEX idx_oidc_auth_requests_client ON oidc_auth_requests(instance_id, client_id);

CREATE TABLE IF NOT EXISTS oidc_rp_auth_states (
    id             TEXT PRIMARY KEY,
    instance_id    TEXT NOT NULL DEFAULT 'default',
    provider_id    TEXT NOT NULL DEFAULT '',
    state          TEXT NOT NULL,
    nonce          TEXT NOT NULL DEFAULT '',
    pkce_verifier  TEXT NOT NULL DEFAULT '',
    flow_id        TEXT NOT NULL DEFAULT '',
    redirect_uri   TEXT NOT NULL DEFAULT '',
    expected_issuer TEXT NOT NULL DEFAULT '',
    callback_uri   TEXT NOT NULL DEFAULT '',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at     TEXT NOT NULL DEFAULT (datetime('now', '+10 minutes'))
);
CREATE UNIQUE INDEX idx_oidc_rp_auth_states_state ON oidc_rp_auth_states(instance_id, state);
CREATE INDEX idx_oidc_rp_auth_states_provider ON oidc_rp_auth_states(instance_id, provider_id);

DELETE FROM auth_states WHERE type IN ('oidc_auth', 'sso');

-- +goose Down
DROP TABLE IF EXISTS oidc_rp_auth_states;
DROP TABLE IF EXISTS oidc_auth_requests;
