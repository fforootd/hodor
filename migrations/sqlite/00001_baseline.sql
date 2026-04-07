-- +goose Up


CREATE TABLE IF NOT EXISTS instances (
    instance_id              TEXT PRIMARY KEY,
    parent_instance_id       TEXT,
    owner_org_id             TEXT,
    kind                     TEXT NOT NULL DEFAULT 'managed' CHECK (kind IN ('root', 'managed', 'federated')),
    state                    TEXT NOT NULL DEFAULT 'active',
    placement_mode           TEXT NOT NULL DEFAULT 'global' CHECK (placement_mode IN ('global', 'regional')),
    region_key               TEXT,
    feature_overrides        TEXT NOT NULL DEFAULT '{}',
    registration_token_hash  TEXT NOT NULL DEFAULT '',
    last_heartbeat_at        TEXT,
    last_heartbeat_status    TEXT NOT NULL DEFAULT '',
    created_at               TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at               TEXT NOT NULL DEFAULT (datetime('now')),
    CHECK (
        (parent_instance_id IS NULL AND owner_org_id IS NULL AND kind = 'root')
        OR (parent_instance_id IS NOT NULL AND owner_org_id IS NOT NULL AND kind IN ('managed', 'federated'))
    ),
    FOREIGN KEY (parent_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE,
    FOREIGN KEY (parent_instance_id, owner_org_id) REFERENCES orgs(instance_id, id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS schemas (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL,
    schema      TEXT NOT NULL,
    version     INTEGER NOT NULL DEFAULT 1,
    is_default  BOOLEAN NOT NULL DEFAULT 0,
    visibility  TEXT NOT NULL DEFAULT 'private',
    message     TEXT DEFAULT '',
    created_by  TEXT DEFAULT '',
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
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
    metadata     TEXT DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, name),
    FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_orgs_instance_state ON orgs(instance_id, state);

CREATE TABLE IF NOT EXISTS users (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    org_id        TEXT NOT NULL DEFAULT '1',
    identifier    TEXT NOT NULL,
    display_name  TEXT DEFAULT '',
    user_type     TEXT NOT NULL DEFAULT 'human',
    state         TEXT NOT NULL DEFAULT 'active',
    schema_id     TEXT DEFAULT '',
    metadata      TEXT DEFAULT '{}',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
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
    data         TEXT DEFAULT '{}',
    name         TEXT DEFAULT '',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
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
    connection     TEXT NOT NULL DEFAULT '{}',
    mapping        TEXT NOT NULL DEFAULT '{}',
    target         TEXT NOT NULL DEFAULT '{}',
    linking        TEXT NOT NULL DEFAULT '{}',
    session        TEXT NOT NULL DEFAULT '{}',
    ui             TEXT NOT NULL DEFAULT '{}',
    enabled        BOOLEAN NOT NULL DEFAULT 1,
    display_order  INTEGER NOT NULL DEFAULT 0,
    catalog_ref    TEXT NOT NULL DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
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
    redirect_uris   TEXT DEFAULT '[]',
    grant_types     TEXT DEFAULT '["authorization_code"]',
    response_types  TEXT DEFAULT '["code"]',
    state           TEXT NOT NULL DEFAULT 'active',
    schema_id       TEXT DEFAULT '',
    metadata        TEXT DEFAULT '{}',
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
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
    config        TEXT NOT NULL DEFAULT '{}',
    priority      INTEGER NOT NULL DEFAULT 0,
    enabled       BOOLEAN NOT NULL DEFAULT 1,
    fail_open     BOOLEAN NOT NULL DEFAULT 0,
    timeout_ms    INTEGER NOT NULL DEFAULT 5000,
    schema_id     TEXT DEFAULT '',
    metadata      TEXT DEFAULT '{}',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
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
    steps          TEXT NOT NULL DEFAULT '[]',
    config         TEXT NOT NULL DEFAULT '{}',
    is_default     BOOLEAN NOT NULL DEFAULT 0,
    enabled        BOOLEAN NOT NULL DEFAULT 1,
    state          TEXT NOT NULL DEFAULT 'draft',
    priority       INTEGER NOT NULL DEFAULT 0,
    audience       TEXT NOT NULL DEFAULT '{}',
    auth_methods   TEXT NOT NULL DEFAULT '{}',
    schema_id      TEXT DEFAULT '',
    metadata       TEXT DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_login_flows_instance_org ON login_flows(instance_id, org_id);
CREATE INDEX IF NOT EXISTS idx_login_flows_instance_state ON login_flows(instance_id, state, enabled);
CREATE UNIQUE INDEX IF NOT EXISTS idx_login_flows_instance_default
    ON login_flows(instance_id)
    WHERE is_default = 1;

CREATE TABLE IF NOT EXISTS login_flow_assets (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    org_id         TEXT NOT NULL DEFAULT '1',
    login_flow_id  TEXT NOT NULL,
    slot           TEXT NOT NULL,
    filename       TEXT NOT NULL DEFAULT '',
    content_type   TEXT NOT NULL,
    size_bytes     INTEGER NOT NULL DEFAULT 0,
    sha256         TEXT NOT NULL,
    etag           TEXT NOT NULL,
    data           BLOB NOT NULL,
    metadata       TEXT DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
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
    raw_claims       TEXT DEFAULT '{}',
    linked_at        TEXT NOT NULL DEFAULT (datetime('now')),
    last_used_at     TEXT,
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
    metadata         TEXT DEFAULT '{}',
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    last_active_at   TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at       TEXT,
    revoked_at       TEXT,
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
    scopes             TEXT NOT NULL DEFAULT '[]',
    audience           TEXT DEFAULT '',
    application_id     TEXT DEFAULT '',
    auth_method        TEXT DEFAULT '',
    auth_time          TEXT,
    refresh_token_id   TEXT DEFAULT '',
    expires_at         TEXT,
    last_used          TEXT,
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    revoked_at         TEXT,
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
    ciphertext          BLOB NOT NULL,
    nonce               BLOB,
    public_key          BLOB,
    expires_at          TEXT,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
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
    done                    INTEGER NOT NULL DEFAULT 0,
    auth_time               TEXT,
    data                    TEXT DEFAULT '{}',
    expires_at              TEXT NOT NULL DEFAULT (datetime('now', '+10 minutes')),
    created_at              TEXT NOT NULL DEFAULT (datetime('now')),
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
    prompt                  TEXT NOT NULL DEFAULT '[]',
    login_hint              TEXT NOT NULL DEFAULT '',
    user_id                 TEXT NOT NULL DEFAULT '',
    code                    TEXT NOT NULL DEFAULT '',
    done                    INTEGER NOT NULL DEFAULT 0,
    auth_time               TEXT,
    max_age                 INTEGER,
    expires_at              TEXT NOT NULL DEFAULT (datetime('now', '+10 minutes')),
    created_at              TEXT NOT NULL DEFAULT (datetime('now')),
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
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at        TEXT NOT NULL DEFAULT (datetime('now', '+10 minutes')),
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
    raw_data     TEXT NOT NULL,
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
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
    payload           TEXT DEFAULT '{}',
    metadata          TEXT DEFAULT '{}',
    request_id        TEXT,
    session_id        TEXT,
    flow_id           TEXT,
    fingerprint       TEXT DEFAULT '',
    client_id         TEXT DEFAULT '',
    token_id          TEXT DEFAULT '',
    delegation_type   TEXT DEFAULT '',
    sdk_name          TEXT DEFAULT '',
    sdk_version       TEXT DEFAULT '',
    sequence          INTEGER,
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
    shipped_at        TEXT,
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
    is_primary   BOOLEAN NOT NULL DEFAULT 0,
    purpose      TEXT NOT NULL DEFAULT 'served',
    state        TEXT NOT NULL DEFAULT 'active',
    verified     BOOLEAN NOT NULL DEFAULT 0,
    verification_token           TEXT NOT NULL DEFAULT '',
    dns_challenge_host           TEXT NOT NULL DEFAULT '',
    dns_authorization_id         TEXT NOT NULL DEFAULT '',
    certificate_dns_record_name  TEXT NOT NULL DEFAULT '',
    certificate_dns_record_type  TEXT NOT NULL DEFAULT '',
    certificate_dns_record_value TEXT NOT NULL DEFAULT '',
    certificate_state            TEXT NOT NULL DEFAULT '',
    certificate_id               TEXT NOT NULL DEFAULT '',
    certificate_map_entry        TEXT NOT NULL DEFAULT '',
    origin_trust_state           TEXT NOT NULL DEFAULT '',
    provisioning_error           TEXT NOT NULL DEFAULT '',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_domains_instance ON domains(instance_id);
CREATE INDEX IF NOT EXISTS idx_domains_instance_org ON domains(instance_id, org_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_domains_instance_primary
    ON domains(instance_id)
    WHERE org_id IS NULL AND is_primary = 1;
CREATE UNIQUE INDEX IF NOT EXISTS idx_domains_org_primary
    ON domains(instance_id, org_id)
    WHERE org_id IS NOT NULL AND is_primary = 1;

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
    data         TEXT NOT NULL DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
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
    enabled             INTEGER NOT NULL DEFAULT 1,
    last_run_at         TEXT,
    next_run_at         TEXT,
    last_status         TEXT NOT NULL DEFAULT 'idle',
    last_error          TEXT NOT NULL DEFAULT '',
    run_count           INTEGER NOT NULL DEFAULT 0,
    config_json         TEXT DEFAULT '{}',
    lease_owner         TEXT NOT NULL DEFAULT '',
    lease_expires_at    TEXT,
    updated_at          TEXT NOT NULL DEFAULT (datetime('now')),
    last_rows_removed   INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, name)
);
CREATE INDEX IF NOT EXISTS idx_jobs_instance_due_lease
    ON jobs(instance_id, enabled, next_run_at, lease_expires_at);

CREATE TABLE IF NOT EXISTS cache (
    instance_id  TEXT NOT NULL,
    namespace    TEXT NOT NULL DEFAULT 'default',
    key          TEXT NOT NULL,
    data         TEXT NOT NULL,
    expires_at   TEXT,
    fetched_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, namespace, key)
);
CREATE INDEX IF NOT EXISTS idx_cache_instance_expires
    ON cache(instance_id, expires_at) WHERE expires_at IS NOT NULL;

CREATE TABLE IF NOT EXISTS consumer_cursors (
    instance_id    TEXT NOT NULL,
    consumer_name  TEXT NOT NULL,
    last_event_id  TEXT NOT NULL DEFAULT '',
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, consumer_name)
);

CREATE TABLE IF NOT EXISTS retention_policies (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    event_pattern  TEXT NOT NULL,
    oltp_ttl       TEXT NOT NULL,
    lake_ttl       TEXT NOT NULL,
    priority       INTEGER NOT NULL DEFAULT 0,
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
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
    metadata     TEXT DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
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
    added_at       TEXT NOT NULL DEFAULT (datetime('now')),
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
    metadata     TEXT DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
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
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);
CREATE INDEX IF NOT EXISTS idx_saved_queries_instance_name ON saved_queries(instance_id, name);

CREATE TABLE IF NOT EXISTS instance_trust_links (
    child_instance_id  TEXT NOT NULL,
    issuer             TEXT NOT NULL,
    audience           TEXT NOT NULL,
    allowed_scopes     TEXT NOT NULL DEFAULT '[]',
    state              TEXT NOT NULL DEFAULT 'active',
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at         TEXT NOT NULL DEFAULT (datetime('now')),
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
    custom_model      TEXT NOT NULL DEFAULT '{}',
    module_fragments  TEXT NOT NULL DEFAULT '[]',
    is_active         INTEGER NOT NULL DEFAULT 0,
    created_at        TEXT NOT NULL DEFAULT (datetime('now')),
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
    inserted_at    TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation)
);
CREATE INDEX IF NOT EXISTS idx_fga_tuples_lookup
    ON fga_tuples(instance_id, store_id, object_type, object_id, relation);
CREATE INDEX IF NOT EXISTS idx_fga_tuples_user
    ON fga_tuples(instance_id, store_id, user_type, user_id, user_relation);

CREATE TABLE IF NOT EXISTS fga_tuple_changes (
    seq                     INTEGER PRIMARY KEY AUTOINCREMENT,
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
    created_at              TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_fga_tuple_changes_lookup
    ON fga_tuple_changes(instance_id, store_id, seq);

INSERT OR IGNORE INTO instances (instance_id, kind, state, placement_mode, feature_overrides)
VALUES ('default', 'root', 'active', 'global', '{}');

INSERT OR IGNORE INTO retention_policies (instance_id, id, event_pattern, oltp_ttl, lake_ttl, priority) VALUES
    ('default', 'rp_auth_login_failure', 'auth.login_failure', '30d', '365d', 100),
    ('default', 'rp_auth',               'auth.*',             '14d', '365d', 90),
    ('default', 'rp_session',            'session.*',          '7d',  '90d',  80),
    ('default', 'rp_identity',           'identity.*',         '30d', '0',    70),
    ('default', 'rp_event',              'event.*',            '3d',  '30d',  60),
    ('default', 'rp_default',            '*',                  '14d', '365d', 0);



ALTER TABLE apps ADD COLUMN post_logout_redirect_uris TEXT NOT NULL DEFAULT '[]';
ALTER TABLE oidc_auth_requests ADD COLUMN session_id TEXT NOT NULL DEFAULT '';





-- Change all org FK constraints from CASCADE to no constraint (SQLite).
-- Orgs are relationships, not ownership — deleting one should orphan resources, not destroy them.
--
-- SQLite does not support column-specific ON DELETE SET NULL for composite FKs.
-- Using ON DELETE SET NULL on (instance_id, org_id) would null BOTH columns,
-- violating the NOT NULL constraint on instance_id.
-- Solution: drop the org FK entirely on SQLite, the app layer handles integrity.
-- Also applies the 00011 fix for users/sessions that was a no-op due to missing goose marker.

-- SQLite cannot ALTER foreign keys, so we recreate each table.
-- Use explicit column lists in INSERT to guard against column-order drift.

-- ── users ──

CREATE TABLE users_new (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    org_id        TEXT DEFAULT NULL,
    identifier    TEXT NOT NULL,
    display_name  TEXT DEFAULT '',
    user_type     TEXT NOT NULL DEFAULT 'human',
    state         TEXT NOT NULL DEFAULT 'active',
    schema_id     TEXT DEFAULT '',
    metadata      TEXT DEFAULT '{}',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, identifier)
);

INSERT INTO users_new (instance_id, id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at FROM users;
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

CREATE INDEX idx_users_instance_org ON users(instance_id, org_id);
CREATE INDEX idx_users_instance_state ON users(instance_id, state);
CREATE INDEX idx_users_instance_type ON users(instance_id, user_type);
CREATE UNIQUE INDEX idx_users_instance_identifier_no_org
    ON users(instance_id, identifier) WHERE org_id IS NULL;

-- ── sessions ──

CREATE TABLE sessions_new (
    instance_id      TEXT NOT NULL,
    id               TEXT NOT NULL,
    user_id          TEXT NOT NULL,
    org_id           TEXT DEFAULT NULL,
    token_hash       TEXT NOT NULL DEFAULT '',
    user_agent       TEXT DEFAULT '',
    ip_address       TEXT DEFAULT '',
    metadata         TEXT DEFAULT '{}',
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    last_active_at   TEXT NOT NULL DEFAULT (datetime('now')),
    expires_at       TEXT,
    revoked_at       TEXT,
    fingerprint      TEXT DEFAULT '',
    PRIMARY KEY (instance_id, id),
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE
);

INSERT INTO sessions_new (instance_id, id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, last_active_at, expires_at, revoked_at, fingerprint)
    SELECT instance_id, id, user_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, last_active_at, expires_at, revoked_at, fingerprint FROM sessions;
DROP TABLE sessions;
ALTER TABLE sessions_new RENAME TO sessions;

CREATE INDEX idx_sessions_instance_user ON sessions(instance_id, user_id);
CREATE INDEX idx_sessions_instance_expires
    ON sessions(instance_id, expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX idx_sessions_instance_revoked
    ON sessions(instance_id, revoked_at) WHERE revoked_at IS NOT NULL;
CREATE UNIQUE INDEX idx_sessions_instance_token_unique
    ON sessions(instance_id, token_hash) WHERE token_hash != '';

-- ── apps ──

CREATE TABLE apps_new (
    instance_id             TEXT NOT NULL,
    id                      TEXT NOT NULL,
    org_id                  TEXT DEFAULT NULL,
    name                    TEXT NOT NULL,
    app_type                TEXT NOT NULL DEFAULT 'oidc',
    client_id               TEXT NOT NULL,
    client_secret           TEXT DEFAULT '',
    redirect_uris           TEXT DEFAULT '[]',
    grant_types             TEXT DEFAULT '["authorization_code"]',
    response_types          TEXT DEFAULT '["code"]',
    state                   TEXT NOT NULL DEFAULT 'active',
    schema_id               TEXT DEFAULT '',
    metadata                TEXT DEFAULT '{}',
    created_at              TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at              TEXT NOT NULL DEFAULT (datetime('now')),
    post_logout_redirect_uris TEXT NOT NULL DEFAULT '[]',
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, client_id)
);

INSERT INTO apps_new (instance_id, id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state, schema_id, metadata, created_at, updated_at, post_logout_redirect_uris)
    SELECT instance_id, id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state, schema_id, metadata, created_at, updated_at, post_logout_redirect_uris FROM apps;
DROP TABLE apps;
ALTER TABLE apps_new RENAME TO apps;

CREATE INDEX idx_apps_instance_org ON apps(instance_id, org_id);
CREATE INDEX idx_apps_instance_client ON apps(instance_id, client_id);

-- ── providers ──

CREATE TABLE providers_new (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    org_id         TEXT DEFAULT NULL,
    display_name   TEXT NOT NULL,
    kind           TEXT NOT NULL DEFAULT 'custom',
    protocol       TEXT NOT NULL DEFAULT 'oidc',
    connection     TEXT NOT NULL DEFAULT '{}',
    mapping        TEXT NOT NULL DEFAULT '{}',
    target         TEXT NOT NULL DEFAULT '{}',
    linking        TEXT NOT NULL DEFAULT '{}',
    session        TEXT NOT NULL DEFAULT '{}',
    ui             TEXT NOT NULL DEFAULT '{}',
    enabled        BOOLEAN NOT NULL DEFAULT 1,
    display_order  INTEGER NOT NULL DEFAULT 0,
    catalog_ref    TEXT NOT NULL DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, display_name)
);

INSERT INTO providers_new (instance_id, id, org_id, display_name, kind, protocol, connection, mapping, target, linking, session, ui, enabled, display_order, catalog_ref, created_at, updated_at)
    SELECT instance_id, id, org_id, display_name, kind, protocol, connection, mapping, target, linking, session, ui, enabled, display_order, catalog_ref, created_at, updated_at FROM providers;
DROP TABLE providers;
ALTER TABLE providers_new RENAME TO providers;

CREATE INDEX idx_providers_instance_org ON providers(instance_id, org_id);
CREATE INDEX idx_providers_instance_protocol ON providers(instance_id, protocol, enabled);
CREATE INDEX idx_providers_instance_sort ON providers(instance_id, display_order, display_name);
CREATE UNIQUE INDEX idx_providers_instance_name_no_org
    ON providers(instance_id, display_name) WHERE org_id IS NULL;

-- ── login_flows ──

CREATE TABLE login_flows_new (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    org_id         TEXT DEFAULT NULL,
    name           TEXT NOT NULL,
    strategy       TEXT NOT NULL DEFAULT 'identifier_first',
    steps          TEXT NOT NULL DEFAULT '[]',
    config         TEXT NOT NULL DEFAULT '{}',
    is_default     BOOLEAN NOT NULL DEFAULT 0,
    enabled        BOOLEAN NOT NULL DEFAULT 1,
    state          TEXT NOT NULL DEFAULT 'draft',
    priority       INTEGER NOT NULL DEFAULT 0,
    audience       TEXT NOT NULL DEFAULT '{}',
    auth_methods   TEXT NOT NULL DEFAULT '{}',
    schema_id      TEXT DEFAULT '',
    metadata       TEXT DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

INSERT INTO login_flows_new (instance_id, id, org_id, name, strategy, steps, config, is_default, enabled, state, priority, audience, auth_methods, schema_id, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, name, strategy, steps, config, is_default, enabled, state, priority, audience, auth_methods, schema_id, metadata, created_at, updated_at FROM login_flows;
DROP TABLE login_flows;
ALTER TABLE login_flows_new RENAME TO login_flows;

CREATE INDEX idx_login_flows_instance_org ON login_flows(instance_id, org_id);
CREATE INDEX idx_login_flows_instance_state ON login_flows(instance_id, state, enabled);
CREATE UNIQUE INDEX idx_login_flows_instance_default
    ON login_flows(instance_id)
    WHERE is_default = 1;

-- ── login_flow_assets ──

CREATE TABLE login_flow_assets_new (
    instance_id    TEXT NOT NULL,
    id             TEXT NOT NULL,
    org_id         TEXT DEFAULT NULL,
    login_flow_id  TEXT NOT NULL,
    slot           TEXT NOT NULL,
    filename       TEXT NOT NULL DEFAULT '',
    content_type   TEXT NOT NULL,
    size_bytes     INTEGER NOT NULL DEFAULT 0,
    sha256         TEXT NOT NULL,
    etag           TEXT NOT NULL,
    data           BLOB NOT NULL,
    metadata       TEXT DEFAULT '{}',
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at     TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, login_flow_id, slot),
    FOREIGN KEY (instance_id, login_flow_id) REFERENCES login_flows(instance_id, id) ON DELETE CASCADE
);

INSERT INTO login_flow_assets_new (instance_id, id, org_id, login_flow_id, slot, filename, content_type, size_bytes, sha256, etag, data, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, login_flow_id, slot, filename, content_type, size_bytes, sha256, etag, data, metadata, created_at, updated_at FROM login_flow_assets;
DROP TABLE login_flow_assets;
ALTER TABLE login_flow_assets_new RENAME TO login_flow_assets;

CREATE INDEX idx_login_flow_assets_instance_flow
    ON login_flow_assets(instance_id, login_flow_id);

-- ── groups ──

CREATE TABLE groups_new (
    instance_id  TEXT NOT NULL,
    id           TEXT NOT NULL,
    org_id       TEXT DEFAULT NULL,
    name         TEXT NOT NULL,
    description  TEXT DEFAULT '',
    state        TEXT NOT NULL DEFAULT 'active',
    schema_id    TEXT DEFAULT '',
    metadata     TEXT DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, name)
);

INSERT INTO groups_new (instance_id, id, org_id, name, description, state, schema_id, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, name, description, state, schema_id, metadata, created_at, updated_at FROM groups;
DROP TABLE groups;
ALTER TABLE groups_new RENAME TO groups;

CREATE INDEX idx_groups_instance_org ON groups(instance_id, org_id);
CREATE UNIQUE INDEX idx_groups_instance_name_no_org
    ON groups(instance_id, name) WHERE org_id IS NULL;

-- ── projects ──

CREATE TABLE projects_new (
    instance_id  TEXT NOT NULL,
    id           TEXT NOT NULL,
    org_id       TEXT DEFAULT NULL,
    name         TEXT NOT NULL,
    description  TEXT DEFAULT '',
    state        TEXT NOT NULL DEFAULT 'active',
    schema_id    TEXT DEFAULT '',
    metadata     TEXT DEFAULT '{}',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id),
    UNIQUE (instance_id, org_id, name)
);

INSERT INTO projects_new (instance_id, id, org_id, name, description, state, schema_id, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, name, description, state, schema_id, metadata, created_at, updated_at FROM projects;
DROP TABLE projects;
ALTER TABLE projects_new RENAME TO projects;

CREATE INDEX idx_projects_instance_org ON projects(instance_id, org_id);
CREATE UNIQUE INDEX idx_projects_instance_name_no_org
    ON projects(instance_id, name) WHERE org_id IS NULL;

-- ── actions ──

CREATE TABLE actions_new (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    org_id        TEXT DEFAULT NULL,
    name          TEXT NOT NULL,
    hook          TEXT NOT NULL DEFAULT 'on_event',
    action_type   TEXT NOT NULL DEFAULT 'expr',
    trigger_expr  TEXT DEFAULT 'true',
    config        TEXT NOT NULL DEFAULT '{}',
    priority      INTEGER NOT NULL DEFAULT 0,
    enabled       BOOLEAN NOT NULL DEFAULT 1,
    fail_open     BOOLEAN NOT NULL DEFAULT 0,
    timeout_ms    INTEGER NOT NULL DEFAULT 5000,
    schema_id     TEXT DEFAULT '',
    metadata      TEXT DEFAULT '{}',
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (instance_id, id)
);

INSERT INTO actions_new (instance_id, id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, timeout_ms, schema_id, metadata, created_at, updated_at)
    SELECT instance_id, id, org_id, name, hook, action_type, trigger_expr, config, priority, enabled, fail_open, timeout_ms, schema_id, metadata, created_at, updated_at FROM actions;
DROP TABLE actions;
ALTER TABLE actions_new RENAME TO actions;

CREATE INDEX idx_actions_instance_org ON actions(instance_id, org_id);
CREATE INDEX idx_actions_instance_hook ON actions(instance_id, hook, enabled);

-- ── domains ──

CREATE TABLE domains_new (
    domain       TEXT PRIMARY KEY,
    instance_id  TEXT NOT NULL,
    org_id       TEXT,
    is_primary   BOOLEAN NOT NULL DEFAULT 0,
    purpose      TEXT NOT NULL DEFAULT 'served',
    state        TEXT NOT NULL DEFAULT 'active',
    verified     BOOLEAN NOT NULL DEFAULT 0,
    verification_token           TEXT NOT NULL DEFAULT '',
    dns_challenge_host           TEXT NOT NULL DEFAULT '',
    dns_authorization_id         TEXT NOT NULL DEFAULT '',
    certificate_dns_record_name  TEXT NOT NULL DEFAULT '',
    certificate_dns_record_type  TEXT NOT NULL DEFAULT '',
    certificate_dns_record_value TEXT NOT NULL DEFAULT '',
    certificate_state            TEXT NOT NULL DEFAULT '',
    certificate_id               TEXT NOT NULL DEFAULT '',
    certificate_map_entry        TEXT NOT NULL DEFAULT '',
    origin_trust_state           TEXT NOT NULL DEFAULT '',
    provisioning_error           TEXT NOT NULL DEFAULT '',
    created_at   TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at   TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE
);

INSERT INTO domains_new (
    domain, instance_id, org_id, is_primary, purpose, state, verified,
    verification_token, dns_challenge_host, dns_authorization_id,
    certificate_dns_record_name, certificate_dns_record_type, certificate_dns_record_value,
    certificate_state, certificate_id, certificate_map_entry,
    origin_trust_state, provisioning_error, created_at, updated_at
)
    SELECT
        domain, instance_id, org_id, is_primary,
        COALESCE(purpose, 'served'),
        state, verified,
        COALESCE(verification_token, ''),
        COALESCE(dns_challenge_host, ''),
        COALESCE(dns_authorization_id, ''),
        COALESCE(certificate_dns_record_name, ''),
        COALESCE(certificate_dns_record_type, ''),
        COALESCE(certificate_dns_record_value, ''),
        COALESCE(certificate_state, ''),
        COALESCE(certificate_id, ''),
        COALESCE(certificate_map_entry, ''),
        COALESCE(origin_trust_state, ''),
        COALESCE(provisioning_error, ''),
        created_at, updated_at
    FROM domains;
DROP TABLE domains;
ALTER TABLE domains_new RENAME TO domains;

CREATE INDEX idx_domains_instance ON domains(instance_id);
CREATE INDEX idx_domains_instance_org ON domains(instance_id, org_id);
CREATE UNIQUE INDEX idx_domains_instance_primary
    ON domains(instance_id)
    WHERE org_id IS NULL AND is_primary = 1;
CREATE UNIQUE INDEX idx_domains_org_primary
    ON domains(instance_id, org_id)
    WHERE org_id IS NOT NULL AND is_primary = 1;

-- NOTE: SQLite cannot do column-specific ON DELETE SET NULL for composite FKs,
-- and trigger BEGIN..END blocks break the migration statement splitter.
-- The DeleteOrg use case handles nulling org_id in application code instead.



CREATE TABLE IF NOT EXISTS role_definitions (
    role_key         TEXT PRIMARY KEY,
    relation_name    TEXT NOT NULL,
    scope_kind       TEXT NOT NULL,
    permissions_json TEXT NOT NULL DEFAULT '[]',
    builtin          INTEGER NOT NULL DEFAULT 1,
    source_version   TEXT NOT NULL DEFAULT '',
    created_at       TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS role_assignments (
    assignment_id            TEXT PRIMARY KEY,
    enforcement_instance_id  TEXT NOT NULL,
    scope_kind               TEXT NOT NULL,
    scope_id                 TEXT NOT NULL,
    principal_ref            TEXT NOT NULL,
    role_key                 TEXT NOT NULL,
    source_kind              TEXT NOT NULL DEFAULT 'manual',
    origin_instance_id       TEXT,
    approved_by              TEXT,
    reason                   TEXT,
    expires_at               TEXT,
    revoked_at               TEXT,
    created_at               TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at               TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (enforcement_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE,
    FOREIGN KEY (origin_instance_id) REFERENCES instances(instance_id) ON DELETE SET NULL,
    FOREIGN KEY (role_key) REFERENCES role_definitions(role_key)
);

CREATE INDEX IF NOT EXISTS idx_role_assignments_instance_scope
    ON role_assignments(enforcement_instance_id, scope_kind, scope_id);
CREATE INDEX IF NOT EXISTS idx_role_assignments_principal
    ON role_assignments(principal_ref, revoked_at, expires_at);
CREATE INDEX IF NOT EXISTS idx_role_assignments_role_source
    ON role_assignments(role_key, source_kind);



DROP INDEX IF EXISTS idx_fga_models_active;
DROP INDEX IF EXISTS idx_fga_tuples_lookup;
DROP INDEX IF EXISTS idx_fga_tuples_user;
DROP INDEX IF EXISTS idx_fga_tuple_changes_lookup;

CREATE TABLE fga_stores_new (
    scope_id  TEXT PRIMARY KEY,
    store_id  TEXT NOT NULL UNIQUE
);

CREATE TABLE fga_authorization_models_new (
    scope_id           TEXT NOT NULL,
    store_id           TEXT NOT NULL,
    model_id           TEXT NOT NULL,
    schema_version     TEXT NOT NULL,
    core_model_version TEXT NOT NULL DEFAULT '',
    compiled_model     TEXT NOT NULL,
    custom_model       TEXT NOT NULL DEFAULT '{}',
    module_fragments   TEXT NOT NULL DEFAULT '[]',
    is_active          INTEGER NOT NULL DEFAULT 0,
    created_at         TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (scope_id, store_id, model_id)
);

CREATE TABLE fga_tuples_new (
    scope_id       TEXT NOT NULL,
    store_id       TEXT NOT NULL,
    object_type    TEXT NOT NULL,
    object_id      TEXT NOT NULL,
    relation       TEXT NOT NULL,
    user_type      TEXT NOT NULL,
    user_id        TEXT NOT NULL,
    user_relation  TEXT NOT NULL DEFAULT '',
    raw_object     TEXT NOT NULL,
    raw_user       TEXT NOT NULL,
    inserted_at    TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (scope_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation)
);

CREATE TABLE fga_tuple_changes_new (
    seq                     INTEGER PRIMARY KEY AUTOINCREMENT,
    scope_id                TEXT NOT NULL,
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
    created_at              TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO fga_stores_new (scope_id, store_id)
SELECT instance_id, store_id FROM fga_instance_stores;

INSERT INTO fga_authorization_models_new (
    scope_id,
    store_id,
    model_id,
    schema_version,
    core_model_version,
    compiled_model,
    custom_model,
    module_fragments,
    is_active,
    created_at
)
SELECT
    instance_id,
    store_id,
    model_id,
    schema_version,
    core_model_version,
    compiled_model,
    custom_model,
    module_fragments,
    is_active,
    created_at
FROM fga_authorization_models;

INSERT INTO fga_tuples_new (
    scope_id,
    store_id,
    object_type,
    object_id,
    relation,
    user_type,
    user_id,
    user_relation,
    raw_object,
    raw_user,
    inserted_at
)
SELECT
    instance_id,
    store_id,
    object_type,
    object_id,
    relation,
    user_type,
    user_id,
    user_relation,
    raw_object,
    raw_user,
    inserted_at
FROM fga_tuples;

INSERT INTO fga_tuple_changes_new (
    seq,
    scope_id,
    store_id,
    operation,
    object_type,
    object_id,
    relation,
    user_type,
    user_id,
    user_relation,
    raw_object,
    raw_user,
    authorization_model_id,
    created_at
)
SELECT
    seq,
    instance_id,
    store_id,
    operation,
    object_type,
    object_id,
    relation,
    user_type,
    user_id,
    user_relation,
    raw_object,
    raw_user,
    authorization_model_id,
    created_at
FROM fga_tuple_changes;

DROP TABLE fga_tuple_changes;
DROP TABLE fga_tuples;
DROP TABLE fga_authorization_models;
DROP TABLE fga_instance_stores;

ALTER TABLE fga_stores_new RENAME TO fga_stores;
ALTER TABLE fga_authorization_models_new RENAME TO fga_authorization_models;
ALTER TABLE fga_tuples_new RENAME TO fga_tuples;
ALTER TABLE fga_tuple_changes_new RENAME TO fga_tuple_changes;

CREATE INDEX idx_fga_models_active
    ON fga_authorization_models(scope_id, store_id, is_active, created_at DESC);
CREATE INDEX idx_fga_tuples_lookup
    ON fga_tuples(scope_id, store_id, object_type, object_id, relation);
CREATE INDEX idx_fga_tuples_user
    ON fga_tuples(scope_id, store_id, user_type, user_id, user_relation);
CREATE INDEX idx_fga_tuple_changes_lookup
    ON fga_tuple_changes(scope_id, store_id, seq);

DELETE FROM sqlite_sequence WHERE name IN ('fga_tuple_changes', 'fga_tuple_changes_new');
INSERT INTO sqlite_sequence(name, seq)
SELECT 'fga_tuple_changes', COALESCE(MAX(seq), 0) FROM fga_tuple_changes;

DELETE FROM instances WHERE instance_id = '_platform';




CREATE TABLE IF NOT EXISTS effects (
    instance_id   TEXT NOT NULL,
    id            TEXT NOT NULL,
    event_id      TEXT NOT NULL DEFAULT '',
    source_key    TEXT NOT NULL,
    effect_type   TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending',
    config        TEXT NOT NULL DEFAULT '{}',
    payload       TEXT NOT NULL DEFAULT '{}',
    attempt       INTEGER NOT NULL DEFAULT 0,
    max_attempts  INTEGER NOT NULL DEFAULT 5,
    next_retry_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_error    TEXT NOT NULL DEFAULT '',
    lease_owner   TEXT NOT NULL DEFAULT '',
    lease_expires_at TEXT,
    created_at    TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at  TEXT,
    PRIMARY KEY (instance_id, id)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_effects_source_key
    ON effects(instance_id, source_key);

CREATE INDEX IF NOT EXISTS idx_effects_due
    ON effects(instance_id, status, next_retry_at, lease_expires_at);

CREATE INDEX IF NOT EXISTS idx_effects_event
    ON effects(instance_id, event_id);

CREATE INDEX IF NOT EXISTS idx_effects_cleanup
    ON effects(instance_id, status, completed_at);



-- +goose Down
