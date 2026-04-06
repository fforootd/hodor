-- +goose Up

CREATE TABLE IF NOT EXISTS instances (
    instance_id              TEXT PRIMARY KEY,
    parent_instance_id       TEXT,
    owner_org_id             TEXT,
    kind                     TEXT NOT NULL DEFAULT 'managed' CHECK (kind IN ('root', 'managed', 'federated')),
    state                    TEXT NOT NULL DEFAULT 'active',
    placement_mode           TEXT NOT NULL DEFAULT 'global' CHECK (placement_mode IN ('global', 'regional')),
    region_key               TEXT,
    feature_overrides        JSONB NOT NULL DEFAULT '{}'::jsonb,
    registration_token_hash  TEXT NOT NULL DEFAULT '',
    last_heartbeat_at        TIMESTAMPTZ,
    last_heartbeat_status    TEXT NOT NULL DEFAULT '',
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (
        (parent_instance_id IS NULL AND owner_org_id IS NULL AND kind = 'root')
        OR (parent_instance_id IS NOT NULL AND owner_org_id IS NOT NULL AND kind IN ('managed', 'federated'))
    ),
    FOREIGN KEY (parent_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS schemas (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL,
    schema      JSONB NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    is_default  BOOLEAN NOT NULL DEFAULT FALSE,
    visibility  TEXT NOT NULL DEFAULT 'private',
    message     TEXT DEFAULT '',
    created_by  TEXT DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_schema_type ON schemas(type);
CREATE INDEX IF NOT EXISTS idx_schema_default ON schemas(type, is_default);
CREATE INDEX IF NOT EXISTS idx_schema_version ON schemas(type, version);

CREATE TABLE IF NOT EXISTS orgs (
    instance_id  TEXT NOT NULL,
    id           TEXT NOT NULL,
    name         TEXT NOT NULL,
    state        TEXT NOT NULL DEFAULT 'active',
    schema_id    TEXT DEFAULT '',
    metadata     JSONB DEFAULT '{}'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, name),
    FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_orgs_instance_state ON orgs(instance_id, state);

ALTER TABLE instances
    ADD CONSTRAINT instances_owner_org_fk
    FOREIGN KEY (parent_instance_id, owner_org_id)
    REFERENCES orgs(instance_id, id)
    ON DELETE RESTRICT;

CREATE TABLE IF NOT EXISTS users (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    org_id        TEXT NOT NULL DEFAULT '1',
    identifier    TEXT NOT NULL,
    display_name  TEXT DEFAULT '',
    user_type     TEXT NOT NULL DEFAULT 'human',
    state         TEXT NOT NULL DEFAULT 'active',
    schema_id     TEXT DEFAULT '',
    metadata      JSONB DEFAULT '{}'::jsonb,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, identifier),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_users_instance_org ON users(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_users_instance_state ON users(instance_id, state);
CREATE INDEX IF NOT EXISTS idx_users_instance_type ON users(instance_id, user_type);

CREATE TABLE IF NOT EXISTS credentials (
    instance_id  TEXT NOT NULL,
    id           TEXT NOT NULL,
    user_id      TEXT NOT NULL,
    type         TEXT NOT NULL,
    data         JSONB DEFAULT '{}'::jsonb,
    name         TEXT DEFAULT '',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_credentials_instance_user ON credentials(instance_id, user_id);
CREATE INDEX IF NOT EXISTS idx_credentials_instance_type ON credentials(instance_id, user_id, type);

CREATE TABLE IF NOT EXISTS providers (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    org_id         TEXT NOT NULL DEFAULT '1',
    display_name   TEXT NOT NULL,
    kind           TEXT NOT NULL DEFAULT 'custom',
    protocol       TEXT NOT NULL DEFAULT 'oidc',
    connection     JSONB NOT NULL DEFAULT '{}'::jsonb,
    mapping        JSONB NOT NULL DEFAULT '{}'::jsonb,
    target         JSONB NOT NULL DEFAULT '{}'::jsonb,
    linking        JSONB NOT NULL DEFAULT '{}'::jsonb,
    session        JSONB NOT NULL DEFAULT '{}'::jsonb,
    ui             JSONB NOT NULL DEFAULT '{}'::jsonb,
    enabled        BOOLEAN NOT NULL DEFAULT TRUE,
    display_order  INTEGER NOT NULL DEFAULT 0,
    catalog_ref    JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, display_name),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_providers_instance_org ON providers(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_providers_instance_protocol ON providers(instance_id, protocol, enabled);
CREATE INDEX IF NOT EXISTS idx_providers_instance_sort ON providers(instance_id, display_order, display_name);

CREATE TABLE IF NOT EXISTS apps (
    instance_id     TEXT NOT NULL,
    id              TEXT NOT NULL,
    org_id          TEXT NOT NULL DEFAULT '1',
    name            TEXT NOT NULL,
    app_type        TEXT NOT NULL DEFAULT 'oidc',
    client_id       TEXT NOT NULL,
    client_secret   TEXT DEFAULT '',
    redirect_uris   JSONB DEFAULT '[]'::jsonb,
    grant_types     JSONB DEFAULT '["authorization_code"]'::jsonb,
    response_types  JSONB DEFAULT '["code"]'::jsonb,
    state           TEXT NOT NULL DEFAULT 'active',
    schema_id       TEXT DEFAULT '',
    metadata        JSONB DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, client_id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_apps_instance_org ON apps(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_apps_instance_client ON apps(instance_id, client_id);

CREATE TABLE IF NOT EXISTS actions (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    org_id        TEXT NOT NULL DEFAULT '1',
    name          TEXT NOT NULL,
    hook          TEXT NOT NULL DEFAULT 'on_event',
    action_type   TEXT NOT NULL DEFAULT 'expr',
    trigger_expr  TEXT DEFAULT 'true',
    config        JSONB NOT NULL DEFAULT '{}'::jsonb,
    priority      INTEGER NOT NULL DEFAULT 0,
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    fail_open     BOOLEAN NOT NULL DEFAULT FALSE,
    timeout_ms    INTEGER NOT NULL DEFAULT 5000,
    schema_id     TEXT DEFAULT '',
    metadata      JSONB DEFAULT '{}'::jsonb,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_actions_instance_org ON actions(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_actions_instance_hook ON actions(instance_id, hook, enabled);

CREATE TABLE IF NOT EXISTS login_flows (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    org_id         TEXT DEFAULT '1',
    name           TEXT NOT NULL,
    strategy       TEXT NOT NULL DEFAULT 'identifier_first',
    steps          JSONB NOT NULL DEFAULT '[]'::jsonb,
    config         JSONB NOT NULL DEFAULT '{}'::jsonb,
    is_default     BOOLEAN NOT NULL DEFAULT FALSE,
    enabled        BOOLEAN NOT NULL DEFAULT TRUE,
    state          TEXT NOT NULL DEFAULT 'draft',
    priority       INTEGER NOT NULL DEFAULT 0,
    audience       JSONB NOT NULL DEFAULT '{}'::jsonb,
    auth_methods   JSONB NOT NULL DEFAULT '{}'::jsonb,
    schema_id      TEXT DEFAULT '',
    metadata       JSONB DEFAULT '{}'::jsonb,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_login_flows_instance_org ON login_flows(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_login_flows_instance_state ON login_flows(instance_id, state, enabled);
CREATE UNIQUE INDEX IF NOT EXISTS idx_login_flows_instance_default
    ON login_flows(instance_id)
    WHERE is_default = TRUE;

CREATE TABLE IF NOT EXISTS login_flow_assets (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    org_id         TEXT NOT NULL DEFAULT '1',
    login_flow_id  TEXT NOT NULL,
    slot           TEXT NOT NULL,
    filename       TEXT NOT NULL DEFAULT '',
    content_type   TEXT NOT NULL,
    size_bytes     BIGINT NOT NULL DEFAULT 0,
    sha256         TEXT NOT NULL,
    etag           TEXT NOT NULL,
    data           BYTEA NOT NULL,
    metadata       JSONB DEFAULT '{}'::jsonb,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, login_flow_id, slot),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, login_flow_id) REFERENCES login_flows(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_login_flow_assets_instance_flow
    ON login_flow_assets(instance_id, login_flow_id);

CREATE TABLE IF NOT EXISTS linked_identities (
    instance_id      TEXT NOT NULL,
    id               TEXT NOT NULL,
    user_id          TEXT NOT NULL,
    provider_id      TEXT NOT NULL,
    external_sub     TEXT NOT NULL,
    external_email   TEXT DEFAULT '',
    raw_claims       JSONB DEFAULT '{}'::jsonb,
    linked_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_used_at     TIMESTAMPTZ,
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, provider_id, external_sub),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, provider_id) REFERENCES providers(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_linked_identities_instance_user
    ON linked_identities(instance_id, user_id);

CREATE TABLE IF NOT EXISTS sessions (
    instance_id      TEXT NOT NULL,
    id               TEXT NOT NULL,
    user_id          TEXT NOT NULL,
    org_id           TEXT NOT NULL DEFAULT '1',
    token_hash       TEXT NOT NULL DEFAULT '',
    user_agent       TEXT DEFAULT '',
    ip_address       TEXT DEFAULT '',
    metadata         JSONB DEFAULT '{}'::jsonb,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_active_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at       TIMESTAMPTZ,
    revoked_at       TIMESTAMPTZ,
    fingerprint      TEXT DEFAULT '',
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_sessions_instance_user ON sessions(instance_id, user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_instance_expires
    ON sessions(instance_id, expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_sessions_instance_revoked
    ON sessions(instance_id, revoked_at) WHERE revoked_at IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_instance_token_unique
    ON sessions(instance_id, token_hash) WHERE token_hash != '';

CREATE TABLE IF NOT EXISTS tokens (
    instance_id        TEXT NOT NULL,
    id                 TEXT NOT NULL,
    type               TEXT NOT NULL,
    token_hash         TEXT NOT NULL,
    user_id            TEXT,
    session_id         TEXT,
    name               TEXT DEFAULT '',
    scopes             JSONB NOT NULL DEFAULT '[]'::jsonb,
    audience           TEXT DEFAULT '',
    application_id     TEXT DEFAULT '',
    auth_method        TEXT DEFAULT '',
    auth_time          TIMESTAMPTZ,
    refresh_token_id   TEXT DEFAULT '',
    expires_at         TIMESTAMPTZ,
    last_used          TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    revoked_at         TIMESTAMPTZ,
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, token_hash),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, session_id) REFERENCES sessions(instance_id, id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_tokens_instance_user ON tokens(instance_id, user_id);
CREATE INDEX IF NOT EXISTS idx_tokens_instance_type ON tokens(instance_id, type, user_id);
CREATE INDEX IF NOT EXISTS idx_tokens_instance_session ON tokens(instance_id, session_id);
CREATE INDEX IF NOT EXISTS idx_tokens_instance_app ON tokens(instance_id, application_id) WHERE application_id != '';
CREATE INDEX IF NOT EXISTS idx_tokens_instance_expires
    ON tokens(instance_id, expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_tokens_instance_revoked
    ON tokens(instance_id, revoked_at) WHERE revoked_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS secrets (
    instance_id         TEXT NOT NULL,
    id                  TEXT NOT NULL,
    secret_type         TEXT NOT NULL,
    algorithm           TEXT NOT NULL DEFAULT 'RS256',
    encryption_key_id   TEXT DEFAULT '',
    ciphertext          BYTEA NOT NULL,
    nonce               BYTEA,
    public_key          BYTEA,
    expires_at          TIMESTAMPTZ,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_secrets_instance_type ON secrets(instance_id, secret_type);
CREATE INDEX IF NOT EXISTS idx_secrets_instance_enc_key ON secrets(instance_id, encryption_key_id);

CREATE TABLE IF NOT EXISTS auth_states (
    instance_id             TEXT NOT NULL,
    id                      TEXT NOT NULL,
    type                    TEXT NOT NULL,
    state                   TEXT DEFAULT '',
    user_id                 TEXT DEFAULT '',
    client_id               TEXT DEFAULT '',
    redirect_uri            TEXT DEFAULT '',
    scopes                  TEXT DEFAULT '',
    nonce                   TEXT DEFAULT '',
    response_type           TEXT DEFAULT 'code',
    code_challenge          TEXT DEFAULT '',
    code_challenge_method   TEXT DEFAULT '',
    pkce_verifier           TEXT DEFAULT '',
    provider_id             TEXT DEFAULT '',
    code                    TEXT DEFAULT '',
    step                    TEXT DEFAULT '',
    done                    BOOLEAN NOT NULL DEFAULT FALSE,
    auth_time               TIMESTAMPTZ,
    data                    JSONB DEFAULT '{}'::jsonb,
    expires_at              TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes'),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_auth_states_instance_state
    ON auth_states(instance_id, state) WHERE state != '';
CREATE INDEX IF NOT EXISTS idx_auth_states_instance_code
    ON auth_states(instance_id, code) WHERE code != '';
CREATE INDEX IF NOT EXISTS idx_auth_states_instance_type ON auth_states(instance_id, type);
CREATE INDEX IF NOT EXISTS idx_auth_states_instance_expires ON auth_states(instance_id, expires_at);

CREATE TABLE IF NOT EXISTS oidc_auth_requests (
    instance_id             TEXT NOT NULL,
    id                      TEXT NOT NULL,
    client_id               TEXT NOT NULL,
    redirect_uri            TEXT NOT NULL DEFAULT '',
    scope                   TEXT NOT NULL DEFAULT '',
    state                   TEXT NOT NULL DEFAULT '',
    nonce                   TEXT NOT NULL DEFAULT '',
    response_type           TEXT NOT NULL DEFAULT 'code',
    code_challenge          TEXT NOT NULL DEFAULT '',
    code_challenge_method   TEXT NOT NULL DEFAULT '',
    prompt                  JSONB NOT NULL DEFAULT '[]'::jsonb,
    login_hint              TEXT NOT NULL DEFAULT '',
    user_id                 TEXT NOT NULL DEFAULT '',
    code                    TEXT NOT NULL DEFAULT '',
    done                    BOOLEAN NOT NULL DEFAULT FALSE,
    auth_time               TIMESTAMPTZ,
    max_age                 BIGINT,
    expires_at              TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes'),
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_created
    ON oidc_auth_requests(instance_id, created_at);
CREATE UNIQUE INDEX IF NOT EXISTS idx_oidc_auth_requests_code
    ON oidc_auth_requests(instance_id, code) WHERE code != '';
CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_client
    ON oidc_auth_requests(instance_id, client_id);
CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_expires
    ON oidc_auth_requests(instance_id, expires_at);

CREATE TABLE IF NOT EXISTS oidc_rp_auth_states (
    instance_id       TEXT NOT NULL,
    id                TEXT NOT NULL,
    provider_id       TEXT NOT NULL DEFAULT '',
    state             TEXT NOT NULL,
    nonce             TEXT NOT NULL DEFAULT '',
    pkce_verifier     TEXT NOT NULL DEFAULT '',
    flow_id           TEXT NOT NULL DEFAULT '',
    redirect_uri      TEXT NOT NULL DEFAULT '',
    expected_issuer   TEXT NOT NULL DEFAULT '',
    callback_uri      TEXT NOT NULL DEFAULT '',
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    expires_at        TIMESTAMPTZ NOT NULL DEFAULT (NOW() + INTERVAL '10 minutes'),
    PRIMARY KEY (instance_id, id)
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_state
    ON oidc_rp_auth_states(instance_id, state);
CREATE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_provider
    ON oidc_rp_auth_states(instance_id, provider_id);
CREATE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_instance_expires
    ON oidc_rp_auth_states(instance_id, expires_at);

CREATE TABLE IF NOT EXISTS fingerprints (
    instance_id  TEXT NOT NULL,
    id           TEXT NOT NULL,
    type         TEXT NOT NULL,
    raw_data     JSONB DEFAULT '{}'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_fingerprints_instance_type ON fingerprints(instance_id, type);

CREATE TABLE IF NOT EXISTS events (
    instance_id       TEXT NOT NULL,
    id                TEXT NOT NULL,
    event_type        TEXT NOT NULL,
    category          TEXT NOT NULL DEFAULT '',
    org_id            TEXT NOT NULL DEFAULT '0',
    actor_id          TEXT,
    actor_type        TEXT,
    aggregate_id      TEXT,
    aggregate_type    TEXT,
    resource_type     TEXT,
    payload           JSONB DEFAULT '{}'::jsonb,
    metadata          JSONB DEFAULT '{}'::jsonb,
    request_id        TEXT,
    session_id        TEXT,
    flow_id           TEXT,
    fingerprint       TEXT DEFAULT '',
    client_id         TEXT DEFAULT '',
    token_id          TEXT DEFAULT '',
    delegation_type   TEXT DEFAULT '',
    sdk_name          TEXT DEFAULT '',
    sdk_version       TEXT DEFAULT '',
    sequence          BIGINT,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    shipped_at        TIMESTAMPTZ,
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_events_instance_created ON events(instance_id, created_at);
CREATE INDEX IF NOT EXISTS idx_events_instance_type_created
    ON events(instance_id, event_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(instance_id, aggregate_id, aggregate_type);
CREATE INDEX IF NOT EXISTS idx_events_request ON events(instance_id, request_id) WHERE request_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_ship ON events(instance_id, shipped_at) WHERE shipped_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_events_category ON events(instance_id, category, created_at);
CREATE INDEX IF NOT EXISTS idx_events_actor ON events(instance_id, actor_id) WHERE actor_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_flow ON events(instance_id, flow_id) WHERE flow_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_events_org ON events(instance_id, org_id, created_at);
CREATE INDEX IF NOT EXISTS idx_events_client ON events(instance_id, client_id) WHERE client_id != '';
CREATE INDEX IF NOT EXISTS idx_events_delegation ON events(instance_id, delegation_type) WHERE delegation_type != '';

CREATE TABLE IF NOT EXISTS domains (
    domain       TEXT PRIMARY KEY,
    instance_id  TEXT NOT NULL,
    org_id       TEXT,
    is_primary   BOOLEAN NOT NULL DEFAULT FALSE,
    state        TEXT NOT NULL DEFAULT 'active',
    verified     BOOLEAN NOT NULL DEFAULT FALSE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_domains_instance ON domains(instance_id);
CREATE INDEX IF NOT EXISTS idx_domains_instance_org ON domains(instance_id, org_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_domains_instance_primary
    ON domains(instance_id)
    WHERE org_id IS NULL AND is_primary = TRUE;
CREATE UNIQUE INDEX IF NOT EXISTS idx_domains_org_primary
    ON domains(instance_id, org_id)
    WHERE org_id IS NOT NULL AND is_primary = TRUE;

CREATE TABLE IF NOT EXISTS unique_fields (
    instance_id        TEXT NOT NULL,
    scope_id           TEXT NOT NULL DEFAULT '',
    field_name         TEXT NOT NULL,
    normalized_value   TEXT NOT NULL,
    resource_type      TEXT NOT NULL DEFAULT '',
    user_id            TEXT NOT NULL,
    PRIMARY KEY (instance_id, scope_id, field_name, normalized_value),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_unique_fields_instance_resource
    ON unique_fields(instance_id, user_id);
CREATE INDEX IF NOT EXISTS idx_unique_fields_instance_lookup
    ON unique_fields(instance_id, normalized_value, field_name);

CREATE TABLE IF NOT EXISTS settings (
    instance_id  TEXT NOT NULL,
    id           TEXT NOT NULL,
    type         TEXT NOT NULL,
    scope        TEXT NOT NULL DEFAULT 'instance',
    scope_id     TEXT NOT NULL DEFAULT '',
    data         JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, type, scope, scope_id)
);
CREATE INDEX IF NOT EXISTS idx_settings_instance_type ON settings(instance_id, type, scope);

CREATE TABLE IF NOT EXISTS jobs (
    instance_id         TEXT NOT NULL,
    name                TEXT NOT NULL,
    display_name        TEXT NOT NULL,
    description         TEXT DEFAULT '',
    cron                TEXT NOT NULL,
    enabled             BOOLEAN NOT NULL DEFAULT TRUE,
    last_run_at         TIMESTAMPTZ,
    next_run_at         TIMESTAMPTZ,
    last_status         TEXT NOT NULL DEFAULT 'idle',
    last_error          TEXT NOT NULL DEFAULT '',
    run_count           INTEGER NOT NULL DEFAULT 0,
    config_json         JSONB DEFAULT '{}'::jsonb,
    lease_owner         TEXT NOT NULL DEFAULT '',
    lease_expires_at    TIMESTAMPTZ,
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    last_rows_removed   BIGINT NOT NULL DEFAULT 0,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, name)
);
CREATE INDEX IF NOT EXISTS idx_jobs_instance_due_lease
    ON jobs(instance_id, enabled, next_run_at, lease_expires_at);

CREATE TABLE IF NOT EXISTS cache (
    instance_id  TEXT NOT NULL,
    namespace    TEXT NOT NULL DEFAULT 'default',
    key          TEXT NOT NULL,
    data         JSONB NOT NULL DEFAULT '{}'::jsonb,
    expires_at   TIMESTAMPTZ,
    fetched_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, namespace, key)
);
CREATE INDEX IF NOT EXISTS idx_cache_instance_expires
    ON cache(instance_id, expires_at) WHERE expires_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS consumer_cursors (
    instance_id    TEXT NOT NULL,
    consumer_name  TEXT NOT NULL,
    last_event_id  TEXT NOT NULL DEFAULT '',
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, consumer_name)
);

CREATE TABLE IF NOT EXISTS retention_policies (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    event_pattern  TEXT NOT NULL,
    oltp_ttl       TEXT NOT NULL,
    lake_ttl       TEXT NOT NULL,
    priority       INTEGER NOT NULL DEFAULT 0,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_retention_policies_instance_priority
    ON retention_policies(instance_id, priority DESC);

CREATE TABLE IF NOT EXISTS groups (
    instance_id  TEXT NOT NULL,
    id           TEXT NOT NULL,
    org_id       TEXT NOT NULL DEFAULT '1',
    name         TEXT NOT NULL,
    description  TEXT DEFAULT '',
    state        TEXT NOT NULL DEFAULT 'active',
    schema_id    TEXT DEFAULT '',
    metadata     JSONB DEFAULT '{}'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, name),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_groups_instance_org ON groups(instance_id, org_id);

CREATE TABLE IF NOT EXISTS memberships (
    instance_id    TEXT NOT NULL,
    resource_type  TEXT NOT NULL,
    resource_id    TEXT NOT NULL,
    user_id        TEXT NOT NULL,
    role           TEXT NOT NULL DEFAULT 'member',
    added_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, resource_type, resource_id, user_id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_memberships_instance_user
    ON memberships(instance_id, user_id, resource_type);
CREATE INDEX IF NOT EXISTS idx_memberships_instance_resource
    ON memberships(instance_id, resource_type, resource_id);

CREATE TABLE IF NOT EXISTS projects (
    instance_id  TEXT NOT NULL,
    id           TEXT NOT NULL,
    org_id       TEXT NOT NULL DEFAULT '1',
    name         TEXT NOT NULL,
    description  TEXT DEFAULT '',
    state        TEXT NOT NULL DEFAULT 'active',
    schema_id    TEXT DEFAULT '',
    metadata     JSONB DEFAULT '{}'::jsonb,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, name),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_projects_instance_org ON projects(instance_id, org_id);

CREATE TABLE IF NOT EXISTS saved_queries (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    name          TEXT NOT NULL,
    description   TEXT DEFAULT '',
    sql_text      TEXT NOT NULL,
    created_by    TEXT DEFAULT '',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_saved_queries_instance_name ON saved_queries(instance_id, name);

CREATE TABLE IF NOT EXISTS instance_trust_links (
    child_instance_id  TEXT NOT NULL,
    issuer             TEXT NOT NULL,
    audience           TEXT NOT NULL,
    allowed_scopes     JSONB NOT NULL DEFAULT '[]'::jsonb,
    state              TEXT NOT NULL DEFAULT 'active',
    created_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (child_instance_id, issuer, audience),
    FOREIGN KEY (child_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS fga_instance_stores (
    instance_id  TEXT PRIMARY KEY,
    store_id     TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS fga_authorization_models (
    instance_id       TEXT NOT NULL,
    store_id          TEXT NOT NULL,
    model_id          TEXT NOT NULL,
    schema_version    TEXT NOT NULL,
    core_model_version TEXT NOT NULL DEFAULT '',
    compiled_model    TEXT NOT NULL,
    custom_model      JSONB NOT NULL DEFAULT '{}'::jsonb,
    module_fragments  JSONB NOT NULL DEFAULT '[]'::jsonb,
    is_active         INTEGER NOT NULL DEFAULT 0,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, store_id, model_id)
);
CREATE INDEX IF NOT EXISTS idx_fga_models_active
    ON fga_authorization_models(instance_id, store_id, is_active, created_at DESC);

CREATE TABLE IF NOT EXISTS fga_tuples (
    instance_id    TEXT NOT NULL,
    store_id       TEXT NOT NULL,
    object_type    TEXT NOT NULL,
    object_id      TEXT NOT NULL,
    relation       TEXT NOT NULL,
    user_type      TEXT NOT NULL,
    user_id        TEXT NOT NULL,
    user_relation  TEXT NOT NULL DEFAULT '',
    raw_object     TEXT NOT NULL,
    raw_user       TEXT NOT NULL,
    inserted_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation)
);
CREATE INDEX IF NOT EXISTS idx_fga_tuples_lookup
    ON fga_tuples(instance_id, store_id, object_type, object_id, relation);
CREATE INDEX IF NOT EXISTS idx_fga_tuples_user
    ON fga_tuples(instance_id, store_id, user_type, user_id, user_relation);

CREATE TABLE IF NOT EXISTS fga_tuple_changes (
    seq                     BIGINT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
    instance_id             TEXT NOT NULL,
    store_id                TEXT NOT NULL,
    operation               TEXT NOT NULL,
    object_type             TEXT NOT NULL,
    object_id               TEXT NOT NULL,
    relation                TEXT NOT NULL,
    user_type               TEXT NOT NULL,
    user_id                 TEXT NOT NULL,
    user_relation           TEXT NOT NULL DEFAULT '',
    raw_object              TEXT NOT NULL,
    raw_user                TEXT NOT NULL,
    authorization_model_id  TEXT NOT NULL DEFAULT '',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_fga_tuple_changes_lookup
    ON fga_tuple_changes(instance_id, store_id, seq);

INSERT INTO instances (instance_id, kind, state, placement_mode, feature_overrides)
VALUES ('default', 'root', 'active', 'global', '{}'::jsonb)
ON CONFLICT (instance_id) DO NOTHING;

INSERT INTO retention_policies (instance_id, id, event_pattern, oltp_ttl, lake_ttl, priority)
VALUES
    ('default', 'rp_auth_login_failure', 'auth.login_failure', '30d', '365d', 100),
    ('default', 'rp_auth',               'auth.*',             '14d', '365d', 90),
    ('default', 'rp_session',            'session.*',          '7d',  '90d',  80),
    ('default', 'rp_identity',           'identity.*',         '30d', '0',    70),
    ('default', 'rp_event',              'event.*',            '3d',  '30d',  60),
    ('default', 'rp_default',            '*',                  '14d', '365d', 0)
ON CONFLICT (instance_id, id) DO NOTHING;

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
