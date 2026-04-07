-- +goose Up
-- Native Spanner baseline for the prototype.
-- This file is authoritative GoogleSQL DDL and intentionally avoids
-- Postgres-only syntax and seed DML.

CREATE TABLE IF NOT EXISTS instances (
    instance_id              STRING(MAX) PRIMARY KEY,
    parent_instance_id       STRING(MAX),
    owner_org_id             STRING(MAX),
    kind                     STRING(MAX) NOT NULL DEFAULT 'managed' CHECK (kind IN ('root', 'managed', 'federated')),
    state                    STRING(MAX) NOT NULL DEFAULT 'active',
    placement_mode           STRING(MAX) NOT NULL DEFAULT 'global' CHECK (placement_mode IN ('global', 'regional')),
    region_key               STRING(MAX),
    feature_overrides        STRING(MAX) NOT NULL DEFAULT ('{}'),
    registration_token_hash  STRING(MAX) NOT NULL DEFAULT '',
    last_heartbeat_at        TIMESTAMP,
    last_heartbeat_status    STRING(MAX) NOT NULL DEFAULT '',
    created_at               TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at               TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    CHECK (
        (parent_instance_id IS NULL AND owner_org_id IS NULL AND kind = 'root')
        OR (parent_instance_id IS NOT NULL AND owner_org_id IS NOT NULL AND kind IN ('managed', 'federated'))
    ),
    FOREIGN KEY (parent_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS schemas (
    id          STRING(MAX) PRIMARY KEY,
    type        STRING(MAX) NOT NULL,
    schema      STRING(MAX) NOT NULL,
    version     INT64 NOT NULL DEFAULT 1,
    is_default  BOOL NOT NULL DEFAULT FALSE,
    visibility  STRING(MAX) NOT NULL DEFAULT 'private',
    message     STRING(MAX) DEFAULT '',
    created_by  STRING(MAX) DEFAULT '',
    created_at  TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP())
);
CREATE INDEX IF NOT EXISTS idx_schema_type ON schemas(type);
CREATE INDEX IF NOT EXISTS idx_schema_default ON schemas(type, is_default);
CREATE INDEX IF NOT EXISTS idx_schema_version ON schemas(type, version);

CREATE TABLE IF NOT EXISTS orgs (
    instance_id  STRING(MAX) NOT NULL,
    id           STRING(MAX) NOT NULL,
    name         STRING(MAX) NOT NULL,
    state        STRING(MAX) NOT NULL DEFAULT 'active',
    schema_id    STRING(MAX) DEFAULT '',
    metadata     STRING(MAX) DEFAULT ('{}'),
    created_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_orgs_instance_name ON orgs(instance_id, name);
CREATE INDEX IF NOT EXISTS idx_orgs_instance_state ON orgs(instance_id, state);

ALTER TABLE instances
    ADD CONSTRAINT instances_owner_org_fk
    FOREIGN KEY (parent_instance_id, owner_org_id)
    REFERENCES orgs(instance_id, id)
    ON DELETE NO ACTION;

CREATE TABLE IF NOT EXISTS users (
    instance_id   STRING(MAX) NOT NULL,
    id            STRING(MAX) NOT NULL,
    org_id        STRING(MAX) NOT NULL DEFAULT '1',
    identifier    STRING(MAX) NOT NULL,
    display_name  STRING(MAX) DEFAULT '',
    user_type     STRING(MAX) NOT NULL DEFAULT 'human',
    state         STRING(MAX) NOT NULL DEFAULT 'active',
    schema_id     STRING(MAX) DEFAULT '',
    metadata      STRING(MAX) DEFAULT ('{}'),
    created_at    TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at    TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_users_instance_org_identifier
    ON users(instance_id, org_id, identifier);
CREATE INDEX IF NOT EXISTS idx_users_instance_org ON users(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_users_instance_state ON users(instance_id, state);
CREATE INDEX IF NOT EXISTS idx_users_instance_type ON users(instance_id, user_type);

CREATE TABLE IF NOT EXISTS credentials (
    instance_id  STRING(MAX) NOT NULL,
    id           STRING(MAX) NOT NULL,
    user_id      STRING(MAX) NOT NULL,
    type         STRING(MAX) NOT NULL,
    data         STRING(MAX) DEFAULT ('{}'),
    name         STRING(MAX) DEFAULT '',
    created_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_credentials_instance_user ON credentials(instance_id, user_id);
CREATE INDEX IF NOT EXISTS idx_credentials_instance_type ON credentials(instance_id, user_id, type);

CREATE TABLE IF NOT EXISTS providers (
    instance_id    STRING(MAX) NOT NULL,
    id             STRING(MAX) NOT NULL,
    org_id         STRING(MAX) NOT NULL DEFAULT '1',
    display_name   STRING(MAX) NOT NULL,
    kind           STRING(MAX) NOT NULL DEFAULT 'custom',
    protocol       STRING(MAX) NOT NULL DEFAULT 'oidc',
    connection     STRING(MAX) NOT NULL DEFAULT ('{}'),
    mapping        STRING(MAX) NOT NULL DEFAULT ('{}'),
    target         STRING(MAX) NOT NULL DEFAULT ('{}'),
    linking        STRING(MAX) NOT NULL DEFAULT ('{}'),
    session        STRING(MAX) NOT NULL DEFAULT ('{}'),
    ui             STRING(MAX) NOT NULL DEFAULT ('{}'),
    enabled        BOOL NOT NULL DEFAULT TRUE,
    display_order  INT64 NOT NULL DEFAULT 0,
    catalog_ref    STRING(MAX) NOT NULL DEFAULT ('{}'),
    created_at     TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at     TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_instance_org_name
    ON providers(instance_id, org_id, display_name);
CREATE INDEX IF NOT EXISTS idx_providers_instance_org ON providers(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_providers_instance_protocol ON providers(instance_id, protocol, enabled);
CREATE INDEX IF NOT EXISTS idx_providers_instance_sort ON providers(instance_id, display_order, display_name);

CREATE TABLE IF NOT EXISTS apps (
    instance_id     STRING(MAX) NOT NULL,
    id              STRING(MAX) NOT NULL,
    org_id          STRING(MAX) NOT NULL DEFAULT '1',
    name            STRING(MAX) NOT NULL,
    app_type        STRING(MAX) NOT NULL DEFAULT 'oidc',
    client_id       STRING(MAX) NOT NULL,
    client_secret   STRING(MAX) DEFAULT '',
    redirect_uris   STRING(MAX) DEFAULT ('[]'),
    grant_types     STRING(MAX) DEFAULT ('["authorization_code"]'),
    response_types  STRING(MAX) DEFAULT ('["code"]'),
    state           STRING(MAX) NOT NULL DEFAULT 'active',
    schema_id       STRING(MAX) DEFAULT '',
    metadata        STRING(MAX) DEFAULT ('{}'),
    created_at      TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at      TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_apps_instance_org ON apps(instance_id, org_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_apps_instance_client ON apps(instance_id, client_id);

CREATE TABLE IF NOT EXISTS actions (
    instance_id   STRING(MAX) NOT NULL,
    id            STRING(MAX) NOT NULL,
    org_id        STRING(MAX) NOT NULL DEFAULT '1',
    name          STRING(MAX) NOT NULL,
    hook          STRING(MAX) NOT NULL DEFAULT 'on_event',
    action_type   STRING(MAX) NOT NULL DEFAULT 'expr',
    trigger_expr  STRING(MAX) DEFAULT 'true',
    config        STRING(MAX) NOT NULL DEFAULT ('{}'),
    priority      INT64 NOT NULL DEFAULT 0,
    enabled       BOOL NOT NULL DEFAULT TRUE,
    fail_open     BOOL NOT NULL DEFAULT FALSE,
    timeout_ms    INT64 NOT NULL DEFAULT 5000,
    schema_id     STRING(MAX) DEFAULT '',
    metadata      STRING(MAX) DEFAULT ('{}'),
    created_at    TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at    TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_actions_instance_org ON actions(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_actions_instance_hook ON actions(instance_id, hook, enabled);

CREATE TABLE IF NOT EXISTS login_flows (
    instance_id    STRING(MAX) NOT NULL,
    id             STRING(MAX) NOT NULL,
    org_id         STRING(MAX) DEFAULT '1',
    name           STRING(MAX) NOT NULL,
    strategy       STRING(MAX) NOT NULL DEFAULT 'identifier_first',
    steps          STRING(MAX) NOT NULL DEFAULT ('[]'),
    config         STRING(MAX) NOT NULL DEFAULT ('{}'),
    is_default     BOOL NOT NULL DEFAULT FALSE,
    enabled        BOOL NOT NULL DEFAULT TRUE,
    state          STRING(MAX) NOT NULL DEFAULT 'draft',
    priority       INT64 NOT NULL DEFAULT 0,
    audience       STRING(MAX) NOT NULL DEFAULT ('{}'),
    auth_methods   STRING(MAX) NOT NULL DEFAULT ('{}'),
    schema_id      STRING(MAX) DEFAULT '',
    metadata       STRING(MAX) DEFAULT ('{}'),
    created_at     TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at     TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_login_flows_instance_org ON login_flows(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_login_flows_instance_state ON login_flows(instance_id, state, enabled);
CREATE TABLE IF NOT EXISTS login_flow_assets (
    instance_id    STRING(MAX) NOT NULL,
    id             STRING(MAX) NOT NULL,
    org_id         STRING(MAX) NOT NULL DEFAULT '1',
    login_flow_id  STRING(MAX) NOT NULL,
    slot           STRING(MAX) NOT NULL,
    filename       STRING(MAX) NOT NULL DEFAULT '',
    content_type   STRING(MAX) NOT NULL,
    size_bytes     INT64 NOT NULL DEFAULT 0,
    sha256         STRING(MAX) NOT NULL,
    etag           STRING(MAX) NOT NULL,
    data           BYTES(MAX) NOT NULL,
    metadata       STRING(MAX) DEFAULT ('{}'),
    created_at     TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at     TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, login_flow_id) REFERENCES login_flows(instance_id, id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_login_flow_assets_instance_slot
    ON login_flow_assets(instance_id, login_flow_id, slot);
CREATE INDEX IF NOT EXISTS idx_login_flow_assets_instance_flow
    ON login_flow_assets(instance_id, login_flow_id);

CREATE TABLE IF NOT EXISTS linked_identities (
    instance_id      STRING(MAX) NOT NULL,
    id               STRING(MAX) NOT NULL,
    user_id          STRING(MAX) NOT NULL,
    provider_id      STRING(MAX) NOT NULL,
    external_sub     STRING(MAX) NOT NULL,
    external_email   STRING(MAX) DEFAULT '',
    raw_claims       STRING(MAX) DEFAULT ('{}'),
    linked_at        TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    last_used_at     TIMESTAMP,
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, provider_id) REFERENCES providers(instance_id, id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_linked_identities_instance_provider_subject
    ON linked_identities(instance_id, provider_id, external_sub);
CREATE INDEX IF NOT EXISTS idx_linked_identities_instance_user
    ON linked_identities(instance_id, user_id);

CREATE TABLE IF NOT EXISTS sessions (
    instance_id      STRING(MAX) NOT NULL,
    id               STRING(MAX) NOT NULL,
    user_id          STRING(MAX) NOT NULL,
    org_id           STRING(MAX) NOT NULL DEFAULT '1',
    token_hash       STRING(MAX) NOT NULL DEFAULT '',
    user_agent       STRING(MAX) DEFAULT '',
    ip_address       STRING(MAX) DEFAULT '',
    metadata         STRING(MAX) DEFAULT ('{}'),
    created_at       TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    last_active_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    expires_at       TIMESTAMP,
    revoked_at       TIMESTAMP,
    fingerprint      STRING(MAX) DEFAULT '',
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_sessions_instance_user ON sessions(instance_id, user_id);
CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_sessions_instance_expires
    ON sessions(instance_id, expires_at);
CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_sessions_instance_revoked
    ON sessions(instance_id, revoked_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_instance_token_unique
    ON sessions(instance_id, token_hash);

CREATE TABLE IF NOT EXISTS tokens (
    instance_id        STRING(MAX) NOT NULL,
    id                 STRING(MAX) NOT NULL,
    type               STRING(MAX) NOT NULL,
    token_hash         STRING(MAX) NOT NULL,
    user_id            STRING(MAX),
    session_id         STRING(MAX),
    name               STRING(MAX) DEFAULT '',
    scopes             STRING(MAX) NOT NULL DEFAULT ('[]'),
    audience           STRING(MAX) DEFAULT '',
    application_id     STRING(MAX) DEFAULT '',
    auth_method        STRING(MAX) DEFAULT '',
    auth_time          TIMESTAMP,
    refresh_token_id   STRING(MAX) DEFAULT '',
    expires_at         TIMESTAMP,
    last_used          TIMESTAMP,
    created_at         TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    revoked_at         TIMESTAMP,
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, session_id) REFERENCES sessions(instance_id, id) ON DELETE NO ACTION
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_tokens_instance_token_hash
    ON tokens(instance_id, token_hash);
CREATE INDEX IF NOT EXISTS idx_tokens_instance_user ON tokens(instance_id, user_id);
CREATE INDEX IF NOT EXISTS idx_tokens_instance_type ON tokens(instance_id, type, user_id);
CREATE INDEX IF NOT EXISTS idx_tokens_instance_session ON tokens(instance_id, session_id);
CREATE INDEX IF NOT EXISTS idx_tokens_instance_app ON tokens(instance_id, application_id);
CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_tokens_instance_expires
    ON tokens(instance_id, expires_at);
CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_tokens_instance_revoked
    ON tokens(instance_id, revoked_at);

CREATE TABLE IF NOT EXISTS secrets (
    instance_id         STRING(MAX) NOT NULL,
    id                  STRING(MAX) NOT NULL,
    secret_type         STRING(MAX) NOT NULL,
    algorithm           STRING(MAX) NOT NULL DEFAULT 'RS256',
    encryption_key_id   STRING(MAX) DEFAULT '',
    ciphertext          BYTES(MAX) NOT NULL,
    nonce               BYTES(MAX),
    public_key          BYTES(MAX),
    expires_at          TIMESTAMP,
    created_at          TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_secrets_instance_type ON secrets(instance_id, secret_type);
CREATE INDEX IF NOT EXISTS idx_secrets_instance_enc_key ON secrets(instance_id, encryption_key_id);

CREATE TABLE IF NOT EXISTS auth_states (
    instance_id             STRING(MAX) NOT NULL,
    id                      STRING(MAX) NOT NULL,
    type                    STRING(MAX) NOT NULL,
    state                   STRING(MAX) DEFAULT '',
    user_id                 STRING(MAX) DEFAULT '',
    client_id               STRING(MAX) DEFAULT '',
    redirect_uri            STRING(MAX) DEFAULT '',
    scopes                  STRING(MAX) DEFAULT '',
    nonce                   STRING(MAX) DEFAULT '',
    response_type           STRING(MAX) DEFAULT 'code',
    code_challenge          STRING(MAX) DEFAULT '',
    code_challenge_method   STRING(MAX) DEFAULT '',
    pkce_verifier           STRING(MAX) DEFAULT '',
    provider_id             STRING(MAX) DEFAULT '',
    code                    STRING(MAX) DEFAULT '',
    step                    STRING(MAX) DEFAULT '',
    done                    BOOL NOT NULL DEFAULT FALSE,
    auth_time               TIMESTAMP,
    data                    STRING(MAX) DEFAULT ('{}'),
    expires_at              TIMESTAMP NOT NULL DEFAULT (TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL 10 MINUTE)),
    created_at              TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_auth_states_instance_state
    ON auth_states(instance_id, state);
CREATE INDEX IF NOT EXISTS idx_auth_states_instance_code
    ON auth_states(instance_id, code);
CREATE INDEX IF NOT EXISTS idx_auth_states_instance_type ON auth_states(instance_id, type);
CREATE INDEX IF NOT EXISTS idx_auth_states_instance_expires ON auth_states(instance_id, expires_at);

CREATE TABLE IF NOT EXISTS oidc_auth_requests (
    instance_id             STRING(MAX) NOT NULL,
    id                      STRING(MAX) NOT NULL,
    client_id               STRING(MAX) NOT NULL,
    redirect_uri            STRING(MAX) NOT NULL DEFAULT '',
    scope                   STRING(MAX) NOT NULL DEFAULT '',
    state                   STRING(MAX) NOT NULL DEFAULT '',
    nonce                   STRING(MAX) NOT NULL DEFAULT '',
    response_type           STRING(MAX) NOT NULL DEFAULT 'code',
    code_challenge          STRING(MAX) NOT NULL DEFAULT '',
    code_challenge_method   STRING(MAX) NOT NULL DEFAULT '',
    prompt                  STRING(MAX) NOT NULL DEFAULT ('[]'),
    login_hint              STRING(MAX) NOT NULL DEFAULT '',
    user_id                 STRING(MAX) NOT NULL DEFAULT '',
    code                    STRING(MAX) NOT NULL DEFAULT '',
    done                    BOOL NOT NULL DEFAULT FALSE,
    auth_time               TIMESTAMP,
    max_age                 INT64,
    expires_at              TIMESTAMP NOT NULL DEFAULT (TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL 10 MINUTE)),
    created_at              TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_created
    ON oidc_auth_requests(instance_id, created_at);
CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_code
    ON oidc_auth_requests(instance_id, code);
CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_client
    ON oidc_auth_requests(instance_id, client_id);
CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_expires
    ON oidc_auth_requests(instance_id, expires_at);

CREATE TABLE IF NOT EXISTS oidc_rp_auth_states (
    instance_id       STRING(MAX) NOT NULL,
    id                STRING(MAX) NOT NULL,
    provider_id       STRING(MAX) NOT NULL DEFAULT '',
    state             STRING(MAX) NOT NULL,
    nonce             STRING(MAX) NOT NULL DEFAULT '',
    pkce_verifier     STRING(MAX) NOT NULL DEFAULT '',
    flow_id           STRING(MAX) NOT NULL DEFAULT '',
    redirect_uri      STRING(MAX) NOT NULL DEFAULT '',
    expected_issuer   STRING(MAX) NOT NULL DEFAULT '',
    callback_uri      STRING(MAX) NOT NULL DEFAULT '',
    created_at        TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    expires_at        TIMESTAMP NOT NULL DEFAULT (TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL 10 MINUTE)),
    PRIMARY KEY (instance_id, id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_state
    ON oidc_rp_auth_states(instance_id, state);
CREATE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_provider
    ON oidc_rp_auth_states(instance_id, provider_id);
CREATE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_instance_expires
    ON oidc_rp_auth_states(instance_id, expires_at);

CREATE TABLE IF NOT EXISTS fingerprints (
    instance_id  STRING(MAX) NOT NULL,
    id           STRING(MAX) NOT NULL,
    type         STRING(MAX) NOT NULL,
    raw_data     STRING(MAX) DEFAULT ('{}'),
    created_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_fingerprints_instance_type ON fingerprints(instance_id, type);

CREATE TABLE IF NOT EXISTS events (
    instance_id       STRING(MAX) NOT NULL,
    id                STRING(MAX) NOT NULL,
    event_type        STRING(MAX) NOT NULL,
    category          STRING(MAX) NOT NULL DEFAULT '',
    org_id            STRING(MAX) NOT NULL DEFAULT '0',
    actor_id          STRING(MAX),
    actor_type        STRING(MAX),
    aggregate_id      STRING(MAX),
    aggregate_type    STRING(MAX),
    resource_type     STRING(MAX),
    payload           STRING(MAX) DEFAULT ('{}'),
    metadata          STRING(MAX) DEFAULT ('{}'),
    request_id        STRING(MAX),
    session_id        STRING(MAX),
    flow_id           STRING(MAX),
    fingerprint       STRING(MAX) DEFAULT '',
    client_id         STRING(MAX) DEFAULT '',
    token_id          STRING(MAX) DEFAULT '',
    delegation_type   STRING(MAX) DEFAULT '',
    sdk_name          STRING(MAX) DEFAULT '',
    sdk_version       STRING(MAX) DEFAULT '',
    sequence          INT64,
    created_at        TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    shipped_at        TIMESTAMP,
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_events_instance_created ON events(instance_id, created_at);
CREATE INDEX IF NOT EXISTS idx_events_instance_type_created
    ON events(instance_id, event_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(instance_id, aggregate_id, aggregate_type);
CREATE INDEX IF NOT EXISTS idx_events_request ON events(instance_id, request_id);
CREATE INDEX IF NOT EXISTS idx_events_ship ON events(instance_id, shipped_at);
CREATE INDEX IF NOT EXISTS idx_events_category ON events(instance_id, category, created_at);
CREATE INDEX IF NOT EXISTS idx_events_actor ON events(instance_id, actor_id);
CREATE INDEX IF NOT EXISTS idx_events_flow ON events(instance_id, flow_id);
CREATE INDEX IF NOT EXISTS idx_events_org ON events(instance_id, org_id, created_at);
CREATE INDEX IF NOT EXISTS idx_events_client ON events(instance_id, client_id);
CREATE INDEX IF NOT EXISTS idx_events_delegation ON events(instance_id, delegation_type);

CREATE TABLE IF NOT EXISTS domains (
    domain       STRING(MAX) PRIMARY KEY,
    instance_id  STRING(MAX) NOT NULL,
    org_id       STRING(MAX),
    is_primary   BOOL NOT NULL DEFAULT FALSE,
    purpose      STRING(MAX) NOT NULL DEFAULT 'served',
    state        STRING(MAX) NOT NULL DEFAULT 'active',
    verified     BOOL NOT NULL DEFAULT FALSE,
    verification_token   STRING(MAX) NOT NULL DEFAULT '',
    dns_challenge_host   STRING(MAX) NOT NULL DEFAULT '',
    dns_authorization_id         STRING(MAX) NOT NULL DEFAULT '',
    certificate_dns_record_name  STRING(MAX) NOT NULL DEFAULT '',
    certificate_dns_record_type  STRING(MAX) NOT NULL DEFAULT '',
    certificate_dns_record_value STRING(MAX) NOT NULL DEFAULT '',
    certificate_state    STRING(MAX) NOT NULL DEFAULT '',
    certificate_id       STRING(MAX) NOT NULL DEFAULT '',
    certificate_map_entry        STRING(MAX) NOT NULL DEFAULT '',
    origin_trust_state   STRING(MAX) NOT NULL DEFAULT '',
    provisioning_error           STRING(MAX) NOT NULL DEFAULT '',
    created_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_domains_instance ON domains(instance_id);
CREATE INDEX IF NOT EXISTS idx_domains_instance_org ON domains(instance_id, org_id);
CREATE TABLE IF NOT EXISTS unique_fields (
    instance_id        STRING(MAX) NOT NULL,
    scope_id           STRING(MAX) NOT NULL DEFAULT '',
    field_name         STRING(MAX) NOT NULL,
    normalized_value   STRING(MAX) NOT NULL,
    resource_type      STRING(MAX) NOT NULL DEFAULT '',
    user_id            STRING(MAX) NOT NULL,
    PRIMARY KEY (instance_id, scope_id, field_name, normalized_value),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_unique_fields_instance_resource
    ON unique_fields(instance_id, user_id);
CREATE INDEX IF NOT EXISTS idx_unique_fields_instance_lookup
    ON unique_fields(instance_id, normalized_value, field_name);

CREATE TABLE IF NOT EXISTS settings (
    instance_id  STRING(MAX) NOT NULL,
    id           STRING(MAX) NOT NULL,
    type         STRING(MAX) NOT NULL,
    scope        STRING(MAX) NOT NULL DEFAULT 'instance',
    scope_id     STRING(MAX) NOT NULL DEFAULT '',
    data         STRING(MAX) NOT NULL DEFAULT ('{}'),
    created_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_settings_instance_scope
    ON settings(instance_id, type, scope, scope_id);
CREATE INDEX IF NOT EXISTS idx_settings_instance_type ON settings(instance_id, type, scope);

CREATE TABLE IF NOT EXISTS jobs (
    instance_id         STRING(MAX) NOT NULL,
    name                STRING(MAX) NOT NULL,
    display_name        STRING(MAX) NOT NULL,
    description         STRING(MAX) DEFAULT '',
    cron                STRING(MAX) NOT NULL,
    enabled             BOOL NOT NULL DEFAULT TRUE,
    last_run_at         TIMESTAMP,
    next_run_at         TIMESTAMP,
    last_status         STRING(MAX) NOT NULL DEFAULT 'idle',
    last_error          STRING(MAX) NOT NULL DEFAULT '',
    run_count           INT64 NOT NULL DEFAULT 0,
    config_json         STRING(MAX) DEFAULT ('{}'),
    lease_owner         STRING(MAX) NOT NULL DEFAULT '',
    lease_expires_at    TIMESTAMP,
    updated_at          TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    last_rows_removed   INT64 NOT NULL DEFAULT 0,
    created_at          TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, name)
);
CREATE INDEX IF NOT EXISTS idx_jobs_instance_due_lease
    ON jobs(instance_id, enabled, next_run_at, lease_expires_at);

CREATE TABLE IF NOT EXISTS cache (
    instance_id  STRING(MAX) NOT NULL,
    namespace    STRING(MAX) NOT NULL DEFAULT 'default',
    key          STRING(MAX) NOT NULL,
    data         STRING(MAX) NOT NULL DEFAULT ('{}'),
    expires_at   TIMESTAMP,
    fetched_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, namespace, key)
);
CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_cache_instance_expires
    ON cache(instance_id, expires_at);

CREATE TABLE IF NOT EXISTS consumer_cursors (
    instance_id    STRING(MAX) NOT NULL,
    consumer_name  STRING(MAX) NOT NULL,
    last_event_id  STRING(MAX) NOT NULL DEFAULT '',
    updated_at     TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, consumer_name)
);

CREATE TABLE IF NOT EXISTS retention_policies (
    instance_id    STRING(MAX) NOT NULL,
    id             STRING(MAX) NOT NULL,
    event_pattern  STRING(MAX) NOT NULL,
    oltp_ttl       STRING(MAX) NOT NULL,
    lake_ttl       STRING(MAX) NOT NULL,
    priority       INT64 NOT NULL DEFAULT 0,
    created_at     TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_retention_policies_instance_priority
    ON retention_policies(instance_id, priority DESC);

CREATE TABLE IF NOT EXISTS groups (
    instance_id  STRING(MAX) NOT NULL,
    id           STRING(MAX) NOT NULL,
    org_id       STRING(MAX) NOT NULL DEFAULT '1',
    name         STRING(MAX) NOT NULL,
    description  STRING(MAX) DEFAULT '',
    state        STRING(MAX) NOT NULL DEFAULT 'active',
    schema_id    STRING(MAX) DEFAULT '',
    metadata     STRING(MAX) DEFAULT ('{}'),
    created_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_groups_instance_org_name
    ON groups(instance_id, org_id, name);
CREATE INDEX IF NOT EXISTS idx_groups_instance_org ON groups(instance_id, org_id);

CREATE TABLE IF NOT EXISTS memberships (
    instance_id    STRING(MAX) NOT NULL,
    resource_type  STRING(MAX) NOT NULL,
    resource_id    STRING(MAX) NOT NULL,
    user_id        STRING(MAX) NOT NULL,
    role           STRING(MAX) NOT NULL DEFAULT 'member',
    added_at       TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, resource_type, resource_id, user_id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_memberships_instance_user
    ON memberships(instance_id, user_id, resource_type);
CREATE INDEX IF NOT EXISTS idx_memberships_instance_resource
    ON memberships(instance_id, resource_type, resource_id);

CREATE TABLE IF NOT EXISTS projects (
    instance_id  STRING(MAX) NOT NULL,
    id           STRING(MAX) NOT NULL,
    org_id       STRING(MAX) NOT NULL DEFAULT '1',
    name         STRING(MAX) NOT NULL,
    description  STRING(MAX) DEFAULT '',
    state        STRING(MAX) NOT NULL DEFAULT 'active',
    schema_id    STRING(MAX) DEFAULT '',
    metadata     STRING(MAX) DEFAULT ('{}'),
    created_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at   TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_projects_instance_org_name
    ON projects(instance_id, org_id, name);
CREATE INDEX IF NOT EXISTS idx_projects_instance_org ON projects(instance_id, org_id);

CREATE TABLE IF NOT EXISTS saved_queries (
    instance_id   STRING(MAX) NOT NULL,
    id            STRING(MAX) NOT NULL,
    name          STRING(MAX) NOT NULL,
    description   STRING(MAX) DEFAULT '',
    sql_text      STRING(MAX) NOT NULL,
    created_by    STRING(MAX) DEFAULT '',
    created_at    TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at    TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_saved_queries_instance_name ON saved_queries(instance_id, name);

CREATE TABLE IF NOT EXISTS instance_trust_links (
    child_instance_id  STRING(MAX) NOT NULL,
    issuer             STRING(MAX) NOT NULL,
    audience           STRING(MAX) NOT NULL,
    allowed_scopes     STRING(MAX) NOT NULL DEFAULT ('[]'),
    state              STRING(MAX) NOT NULL DEFAULT 'active',
    created_at         TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at         TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (child_instance_id, issuer, audience),
    FOREIGN KEY (child_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS fga_instance_stores (
    instance_id  STRING(MAX) PRIMARY KEY,
    store_id     STRING(MAX) NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_fga_instance_stores_store_id
    ON fga_instance_stores(store_id);

CREATE TABLE IF NOT EXISTS fga_authorization_models (
    instance_id       STRING(MAX) NOT NULL,
    store_id          STRING(MAX) NOT NULL,
    model_id          STRING(MAX) NOT NULL,
    schema_version    STRING(MAX) NOT NULL,
    core_model_version STRING(MAX) NOT NULL DEFAULT (''),
    compiled_model    STRING(MAX) NOT NULL,
    custom_model      STRING(MAX) NOT NULL DEFAULT ('{}'),
    module_fragments  STRING(MAX) NOT NULL DEFAULT ('[]'),
    is_active         INT64 NOT NULL DEFAULT 0,
    created_at        TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, store_id, model_id)
);
CREATE INDEX IF NOT EXISTS idx_fga_models_active
    ON fga_authorization_models(instance_id, store_id, is_active, created_at DESC);

CREATE TABLE IF NOT EXISTS fga_tuples (
    instance_id    STRING(MAX) NOT NULL,
    store_id       STRING(MAX) NOT NULL,
    object_type    STRING(MAX) NOT NULL,
    object_id      STRING(MAX) NOT NULL,
    relation       STRING(MAX) NOT NULL,
    user_type      STRING(MAX) NOT NULL,
    user_id        STRING(MAX) NOT NULL,
    user_relation  STRING(MAX) NOT NULL DEFAULT '',
    raw_object     STRING(MAX) NOT NULL,
    raw_user       STRING(MAX) NOT NULL,
    inserted_at    TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation)
);
CREATE INDEX IF NOT EXISTS idx_fga_tuples_lookup
    ON fga_tuples(instance_id, store_id, object_type, object_id, relation);
CREATE INDEX IF NOT EXISTS idx_fga_tuples_user
    ON fga_tuples(instance_id, store_id, user_type, user_id, user_relation);

CREATE TABLE IF NOT EXISTS fga_tuple_changes (
    seq                     INT64 GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    instance_id             STRING(MAX) NOT NULL,
    store_id                STRING(MAX) NOT NULL,
    operation               STRING(MAX) NOT NULL,
    object_type             STRING(MAX) NOT NULL,
    object_id               STRING(MAX) NOT NULL,
    relation                STRING(MAX) NOT NULL,
    user_type               STRING(MAX) NOT NULL,
    user_id                 STRING(MAX) NOT NULL,
    user_relation           STRING(MAX) NOT NULL DEFAULT '',
    raw_object              STRING(MAX) NOT NULL,
    raw_user                STRING(MAX) NOT NULL,
    authorization_model_id  STRING(MAX) NOT NULL DEFAULT '',
    created_at              TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP())
);
CREATE INDEX IF NOT EXISTS idx_fga_tuple_changes_lookup
    ON fga_tuple_changes(instance_id, store_id, seq);

-- +goose Down
DROP TABLE IF EXISTS fga_tuple_changes;
DROP TABLE IF EXISTS fga_tuples;
DROP TABLE IF EXISTS fga_authorization_models;
DROP TABLE IF EXISTS fga_instance_stores;
DROP TABLE IF EXISTS instance_trust_links;
DROP TABLE IF EXISTS saved_queries;
DROP TABLE IF EXISTS projects;
DROP TABLE IF EXISTS memberships;
DROP TABLE IF EXISTS groups;
DROP TABLE IF EXISTS retention_policies;
DROP TABLE IF EXISTS consumer_cursors;
DROP TABLE IF EXISTS cache;
DROP TABLE IF EXISTS jobs;
DROP TABLE IF EXISTS settings;
DROP TABLE IF EXISTS unique_fields;
DROP TABLE IF EXISTS domains;
DROP TABLE IF EXISTS events;
DROP TABLE IF EXISTS fingerprints;
DROP TABLE IF EXISTS oidc_rp_auth_states;
DROP TABLE IF EXISTS oidc_auth_requests;
DROP TABLE IF EXISTS auth_states;
DROP TABLE IF EXISTS secrets;
DROP TABLE IF EXISTS tokens;
DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS linked_identities;
DROP TABLE IF EXISTS login_flow_assets;
DROP TABLE IF EXISTS login_flows;
DROP TABLE IF EXISTS actions;
DROP TABLE IF EXISTS apps;
DROP TABLE IF EXISTS providers;
DROP TABLE IF EXISTS credentials;
DROP TABLE IF EXISTS users;
DROP TABLE IF EXISTS orgs;
DROP TABLE IF EXISTS schemas;
DROP TABLE IF EXISTS instances;
