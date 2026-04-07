-- +goose Up

CREATE TABLE IF NOT EXISTS actions (
    action_type STRING(MAX) NOT NULL DEFAULT ('expr'),
    config STRING(MAX) NOT NULL DEFAULT ('{}'),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    enabled BOOL NOT NULL DEFAULT (TRUE),
    fail_open BOOL NOT NULL DEFAULT (FALSE),
    hook STRING(MAX) NOT NULL DEFAULT ('on_event'),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    metadata STRING(MAX) DEFAULT ('{}'),
    name STRING(MAX) NOT NULL,
    org_id STRING(MAX),
    priority INT64 NOT NULL DEFAULT (0),
    schema_id STRING(MAX) DEFAULT (''),
    timeout_ms INT64 NOT NULL DEFAULT (5000),
    trigger_expr STRING(MAX) DEFAULT ('true'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS apps (
    app_type STRING(MAX) NOT NULL DEFAULT ('oidc'),
    client_id STRING(MAX) NOT NULL,
    client_secret STRING(MAX) DEFAULT (''),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    grant_types STRING(MAX) DEFAULT ('["authorization_code"]'),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    metadata STRING(MAX) DEFAULT ('{}'),
    name STRING(MAX) NOT NULL,
    org_id STRING(MAX),
    post_logout_redirect_uris STRING(MAX) NOT NULL DEFAULT ('[]'),
    redirect_uris STRING(MAX) DEFAULT ('[]'),
    response_types STRING(MAX) DEFAULT ('["code"]'),
    schema_id STRING(MAX) DEFAULT (''),
    state STRING(MAX) NOT NULL DEFAULT ('active'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS auth_states (
    auth_time TIMESTAMP,
    client_id STRING(MAX) DEFAULT (''),
    code STRING(MAX) DEFAULT (''),
    code_challenge STRING(MAX) DEFAULT (''),
    code_challenge_method STRING(MAX) DEFAULT (''),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    data STRING(MAX) DEFAULT ('{}'),
    done INT64 NOT NULL DEFAULT (0),
    expires_at TIMESTAMP NOT NULL DEFAULT (TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL 10 MINUTE)),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    nonce STRING(MAX) DEFAULT (''),
    pkce_verifier STRING(MAX) DEFAULT (''),
    provider_id STRING(MAX) DEFAULT (''),
    redirect_uri STRING(MAX) DEFAULT (''),
    response_type STRING(MAX) DEFAULT ('code'),
    scopes STRING(MAX) DEFAULT (''),
    state STRING(MAX) DEFAULT (''),
    step STRING(MAX) DEFAULT (''),
    type STRING(MAX) NOT NULL,
    user_id STRING(MAX) DEFAULT (''),
    spx_2db8f54e942ce572_m BOOL AS (IF(state != '', TRUE, NULL)) STORED,
    spx_8f77f43702bb7948_m BOOL AS (IF(code != '', TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS cache (
    data STRING(MAX) NOT NULL,
    expires_at TIMESTAMP,
    fetched_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    instance_id STRING(MAX) NOT NULL,
    key STRING(MAX) NOT NULL,
    namespace STRING(MAX) NOT NULL DEFAULT ('default'),
    spx_48a38ab3fc5d5532_m BOOL AS (IF(expires_at IS NOT NULL, TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, namespace, key)
);

CREATE TABLE IF NOT EXISTS consumer_cursors (
    consumer_name STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    last_event_id STRING(MAX) NOT NULL DEFAULT (''),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, consumer_name)
);

CREATE TABLE IF NOT EXISTS credentials (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    data STRING(MAX) DEFAULT ('{}'),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    name STRING(MAX) DEFAULT (''),
    type STRING(MAX) NOT NULL,
    user_id STRING(MAX) NOT NULL,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS domains (
    certificate_dns_record_name STRING(MAX) NOT NULL DEFAULT (''),
    certificate_dns_record_type STRING(MAX) NOT NULL DEFAULT (''),
    certificate_dns_record_value STRING(MAX) NOT NULL DEFAULT (''),
    certificate_id STRING(MAX) NOT NULL DEFAULT (''),
    certificate_map_entry STRING(MAX) NOT NULL DEFAULT (''),
    certificate_state STRING(MAX) NOT NULL DEFAULT (''),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    dns_authorization_id STRING(MAX) NOT NULL DEFAULT (''),
    dns_challenge_host STRING(MAX) NOT NULL DEFAULT (''),
    domain STRING(MAX),
    instance_id STRING(MAX) NOT NULL,
    is_primary BOOL NOT NULL DEFAULT (FALSE),
    org_id STRING(MAX),
    origin_trust_state STRING(MAX) NOT NULL DEFAULT (''),
    provisioning_error STRING(MAX) NOT NULL DEFAULT (''),
    purpose STRING(MAX) NOT NULL DEFAULT ('served'),
    state STRING(MAX) NOT NULL DEFAULT ('active'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    verification_token STRING(MAX) NOT NULL DEFAULT (''),
    verified BOOL NOT NULL DEFAULT (FALSE),
    spx_4d952f654d511ed6_m BOOL AS (IF(org_id IS NOT NULL AND is_primary = TRUE, TRUE, NULL)) STORED,
    spx_a4a0c49196eac339_m BOOL AS (IF(org_id IS NULL AND is_primary = TRUE, TRUE, NULL)) STORED,
    PRIMARY KEY (domain)
);

CREATE TABLE IF NOT EXISTS effects (
    attempt INT64 NOT NULL DEFAULT (0),
    completed_at TIMESTAMP,
    config STRING(MAX) NOT NULL DEFAULT ('{}'),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    effect_type STRING(MAX) NOT NULL,
    event_id STRING(MAX) NOT NULL DEFAULT (''),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    last_error STRING(MAX) NOT NULL DEFAULT (''),
    lease_expires_at TIMESTAMP,
    lease_owner STRING(MAX) NOT NULL DEFAULT (''),
    max_attempts INT64 NOT NULL DEFAULT (5),
    next_retry_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    payload STRING(MAX) NOT NULL DEFAULT ('{}'),
    source_key STRING(MAX) NOT NULL,
    status STRING(MAX) NOT NULL DEFAULT ('pending'),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS events (
    actor_id STRING(MAX),
    actor_type STRING(MAX),
    aggregate_id STRING(MAX),
    aggregate_type STRING(MAX),
    category STRING(MAX) NOT NULL DEFAULT (''),
    client_id STRING(MAX) DEFAULT (''),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    delegation_type STRING(MAX) DEFAULT (''),
    event_type STRING(MAX) NOT NULL,
    fingerprint STRING(MAX) DEFAULT (''),
    flow_id STRING(MAX),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    metadata STRING(MAX) DEFAULT ('{}'),
    org_id STRING(MAX) NOT NULL DEFAULT ('0'),
    payload STRING(MAX) DEFAULT ('{}'),
    request_id STRING(MAX),
    resource_type STRING(MAX),
    sdk_name STRING(MAX) DEFAULT (''),
    sdk_version STRING(MAX) DEFAULT (''),
    sequence INT64,
    session_id STRING(MAX),
    shipped_at TIMESTAMP,
    token_id STRING(MAX) DEFAULT (''),
    spx_008e7a4af975252b_m BOOL AS (IF(shipped_at IS NULL, TRUE, NULL)) STORED,
    spx_36df21b0222c6b5b_m BOOL AS (IF(delegation_type != '', TRUE, NULL)) STORED,
    spx_7284230623d631ba_m BOOL AS (IF(client_id != '', TRUE, NULL)) STORED,
    spx_87db3dfee055a026_m BOOL AS (IF(request_id IS NOT NULL, TRUE, NULL)) STORED,
    spx_8b9bddb838e0747e_m BOOL AS (IF(actor_id IS NOT NULL, TRUE, NULL)) STORED,
    spx_d166a9f8bccbf4b1_m BOOL AS (IF(flow_id IS NOT NULL, TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS fga_authorization_models (
    compiled_model STRING(MAX) NOT NULL,
    core_model_version STRING(MAX) NOT NULL DEFAULT (''),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    custom_model STRING(MAX) NOT NULL DEFAULT ('{}'),
    is_active INT64 NOT NULL DEFAULT (0),
    model_id STRING(MAX) NOT NULL,
    module_fragments STRING(MAX) NOT NULL DEFAULT ('[]'),
    schema_version STRING(MAX) NOT NULL,
    scope_id STRING(MAX) NOT NULL,
    store_id STRING(MAX) NOT NULL,
    PRIMARY KEY (scope_id, store_id, model_id)
);

CREATE TABLE IF NOT EXISTS fga_stores (
    scope_id STRING(MAX),
    store_id STRING(MAX) NOT NULL,
    PRIMARY KEY (scope_id)
);

CREATE TABLE IF NOT EXISTS fga_tuple_changes (
    authorization_model_id STRING(MAX) NOT NULL DEFAULT (''),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    object_id STRING(MAX) NOT NULL,
    object_type STRING(MAX) NOT NULL,
    operation STRING(MAX) NOT NULL,
    raw_object STRING(MAX) NOT NULL,
    raw_user STRING(MAX) NOT NULL,
    relation STRING(MAX) NOT NULL,
    scope_id STRING(MAX) NOT NULL,
    seq INT64 GENERATED BY DEFAULT AS IDENTITY (BIT_REVERSED_POSITIVE) PRIMARY KEY,
    store_id STRING(MAX) NOT NULL,
    user_id STRING(MAX) NOT NULL,
    user_relation STRING(MAX) NOT NULL DEFAULT (''),
    user_type STRING(MAX) NOT NULL
);

CREATE TABLE IF NOT EXISTS fga_tuples (
    inserted_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    object_id STRING(MAX) NOT NULL,
    object_type STRING(MAX) NOT NULL,
    raw_object STRING(MAX) NOT NULL,
    raw_user STRING(MAX) NOT NULL,
    relation STRING(MAX) NOT NULL,
    scope_id STRING(MAX) NOT NULL,
    store_id STRING(MAX) NOT NULL,
    user_id STRING(MAX) NOT NULL,
    user_relation STRING(MAX) NOT NULL DEFAULT (''),
    user_type STRING(MAX) NOT NULL,
    PRIMARY KEY (scope_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation)
);

CREATE TABLE IF NOT EXISTS fingerprints (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    raw_data STRING(MAX) NOT NULL,
    type STRING(MAX) NOT NULL,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS `groups` (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    description STRING(MAX) DEFAULT (''),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    metadata STRING(MAX) DEFAULT ('{}'),
    name STRING(MAX) NOT NULL,
    org_id STRING(MAX),
    schema_id STRING(MAX) DEFAULT (''),
    state STRING(MAX) NOT NULL DEFAULT ('active'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    spx_6ec06a62084df108_m BOOL AS (IF(org_id IS NULL, TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS instance_trust_links (
    allowed_scopes STRING(MAX) NOT NULL DEFAULT ('[]'),
    audience STRING(MAX) NOT NULL,
    child_instance_id STRING(MAX) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    issuer STRING(MAX) NOT NULL,
    state STRING(MAX) NOT NULL DEFAULT ('active'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (child_instance_id, issuer, audience)
);

CREATE TABLE IF NOT EXISTS instances (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    feature_overrides STRING(MAX) NOT NULL DEFAULT ('{}'),
    instance_id STRING(MAX),
    kind STRING(MAX) NOT NULL DEFAULT ('managed'),
    last_heartbeat_at TIMESTAMP,
    last_heartbeat_status STRING(MAX) NOT NULL DEFAULT (''),
    owner_org_id STRING(MAX),
    parent_instance_id STRING(MAX),
    placement_mode STRING(MAX) NOT NULL DEFAULT ('global'),
    region_key STRING(MAX),
    registration_token_hash STRING(MAX) NOT NULL DEFAULT (''),
    state STRING(MAX) NOT NULL DEFAULT ('active'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id),
    CHECK ((parent_instance_id IS NULL AND owner_org_id IS NULL AND kind = 'root') OR (parent_instance_id IS NOT NULL AND owner_org_id IS NOT NULL AND kind IN ('managed', 'federated'))),
    CHECK (kind IN ('root', 'managed', 'federated')),
    CHECK (placement_mode IN ('global', 'regional'))
);

CREATE TABLE IF NOT EXISTS jobs (
    config_json STRING(MAX) DEFAULT ('{}'),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    cron STRING(MAX) NOT NULL,
    description STRING(MAX) DEFAULT (''),
    display_name STRING(MAX) NOT NULL,
    enabled INT64 NOT NULL DEFAULT (1),
    instance_id STRING(MAX) NOT NULL,
    last_error STRING(MAX) NOT NULL DEFAULT (''),
    last_rows_removed INT64 NOT NULL DEFAULT (0),
    last_run_at TIMESTAMP,
    last_status STRING(MAX) NOT NULL DEFAULT ('idle'),
    lease_expires_at TIMESTAMP,
    lease_owner STRING(MAX) NOT NULL DEFAULT (''),
    name STRING(MAX) NOT NULL,
    next_run_at TIMESTAMP,
    run_count INT64 NOT NULL DEFAULT (0),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, name)
);

CREATE TABLE IF NOT EXISTS linked_identities (
    external_email STRING(MAX) DEFAULT (''),
    external_sub STRING(MAX) NOT NULL,
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    last_used_at TIMESTAMP,
    linked_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    provider_id STRING(MAX) NOT NULL,
    raw_claims STRING(MAX) DEFAULT ('{}'),
    user_id STRING(MAX) NOT NULL,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS login_flow_assets (
    content_type STRING(MAX) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    data STRING(MAX) NOT NULL,
    etag STRING(MAX) NOT NULL,
    filename STRING(MAX) NOT NULL DEFAULT (''),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    login_flow_id STRING(MAX) NOT NULL,
    metadata STRING(MAX) DEFAULT ('{}'),
    org_id STRING(MAX),
    sha256 STRING(MAX) NOT NULL,
    size_bytes INT64 NOT NULL DEFAULT (0),
    slot STRING(MAX) NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS login_flows (
    audience STRING(MAX) NOT NULL DEFAULT ('{}'),
    auth_methods STRING(MAX) NOT NULL DEFAULT ('{}'),
    config STRING(MAX) NOT NULL DEFAULT ('{}'),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    enabled BOOL NOT NULL DEFAULT (TRUE),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    is_default BOOL NOT NULL DEFAULT (FALSE),
    metadata STRING(MAX) DEFAULT ('{}'),
    name STRING(MAX) NOT NULL,
    org_id STRING(MAX),
    priority INT64 NOT NULL DEFAULT (0),
    schema_id STRING(MAX) DEFAULT (''),
    state STRING(MAX) NOT NULL DEFAULT ('draft'),
    steps STRING(MAX) NOT NULL DEFAULT ('[]'),
    strategy STRING(MAX) NOT NULL DEFAULT ('identifier_first'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    spx_7b74940375556728_m BOOL AS (IF(is_default = TRUE, TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS memberships (
    added_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    instance_id STRING(MAX) NOT NULL,
    resource_id STRING(MAX) NOT NULL,
    resource_type STRING(MAX) NOT NULL,
    role STRING(MAX) NOT NULL DEFAULT ('member'),
    user_id STRING(MAX) NOT NULL,
    PRIMARY KEY (instance_id, resource_type, resource_id, user_id)
);

CREATE TABLE IF NOT EXISTS oidc_auth_requests (
    auth_time TIMESTAMP,
    client_id STRING(MAX) NOT NULL,
    code STRING(MAX) NOT NULL DEFAULT (''),
    code_challenge STRING(MAX) NOT NULL DEFAULT (''),
    code_challenge_method STRING(MAX) NOT NULL DEFAULT (''),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    done INT64 NOT NULL DEFAULT (0),
    expires_at TIMESTAMP NOT NULL DEFAULT (TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL 10 MINUTE)),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    login_hint STRING(MAX) NOT NULL DEFAULT (''),
    max_age INT64,
    nonce STRING(MAX) NOT NULL DEFAULT (''),
    prompt STRING(MAX) NOT NULL DEFAULT ('[]'),
    redirect_uri STRING(MAX) NOT NULL DEFAULT (''),
    response_type STRING(MAX) NOT NULL DEFAULT ('code'),
    scope STRING(MAX) NOT NULL DEFAULT (''),
    session_id STRING(MAX) NOT NULL DEFAULT (''),
    state STRING(MAX) NOT NULL DEFAULT (''),
    user_id STRING(MAX) NOT NULL DEFAULT (''),
    spx_21d48265031a895a_m BOOL AS (IF(code != '', TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS oidc_rp_auth_states (
    callback_uri STRING(MAX) NOT NULL DEFAULT (''),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    expected_issuer STRING(MAX) NOT NULL DEFAULT (''),
    expires_at TIMESTAMP NOT NULL DEFAULT (TIMESTAMP_ADD(CURRENT_TIMESTAMP(), INTERVAL 10 MINUTE)),
    flow_id STRING(MAX) NOT NULL DEFAULT (''),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    nonce STRING(MAX) NOT NULL DEFAULT (''),
    pkce_verifier STRING(MAX) NOT NULL DEFAULT (''),
    provider_id STRING(MAX) NOT NULL DEFAULT (''),
    redirect_uri STRING(MAX) NOT NULL DEFAULT (''),
    state STRING(MAX) NOT NULL,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS orgs (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    metadata STRING(MAX) DEFAULT ('{}'),
    name STRING(MAX) NOT NULL,
    schema_id STRING(MAX) DEFAULT (''),
    state STRING(MAX) NOT NULL DEFAULT ('active'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS projects (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    description STRING(MAX) DEFAULT (''),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    metadata STRING(MAX) DEFAULT ('{}'),
    name STRING(MAX) NOT NULL,
    org_id STRING(MAX),
    schema_id STRING(MAX) DEFAULT (''),
    state STRING(MAX) NOT NULL DEFAULT ('active'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    spx_3c82cf40a95bc1ce_m BOOL AS (IF(org_id IS NULL, TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS providers (
    catalog_ref STRING(MAX) NOT NULL DEFAULT ('{}'),
    connection STRING(MAX) NOT NULL DEFAULT ('{}'),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    display_name STRING(MAX) NOT NULL,
    display_order INT64 NOT NULL DEFAULT (0),
    enabled BOOL NOT NULL DEFAULT (TRUE),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    kind STRING(MAX) NOT NULL DEFAULT ('custom'),
    linking STRING(MAX) NOT NULL DEFAULT ('{}'),
    mapping STRING(MAX) NOT NULL DEFAULT ('{}'),
    org_id STRING(MAX),
    protocol STRING(MAX) NOT NULL DEFAULT ('oidc'),
    session STRING(MAX) NOT NULL DEFAULT ('{}'),
    target STRING(MAX) NOT NULL DEFAULT ('{}'),
    ui STRING(MAX) NOT NULL DEFAULT ('{}'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    spx_2721e3196171e498_m BOOL AS (IF(org_id IS NULL, TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS retention_policies (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    event_pattern STRING(MAX) NOT NULL,
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    lake_ttl STRING(MAX) NOT NULL,
    oltp_ttl STRING(MAX) NOT NULL,
    priority INT64 NOT NULL DEFAULT (0),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS role_assignments (
    approved_by STRING(MAX),
    assignment_id STRING(MAX),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    enforcement_instance_id STRING(MAX) NOT NULL,
    expires_at TIMESTAMP,
    origin_instance_id STRING(MAX),
    principal_ref STRING(MAX) NOT NULL,
    reason STRING(MAX),
    revoked_at TIMESTAMP,
    role_key STRING(MAX) NOT NULL,
    scope_id STRING(MAX) NOT NULL,
    scope_kind STRING(MAX) NOT NULL,
    source_kind STRING(MAX) NOT NULL DEFAULT ('manual'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (assignment_id)
);

CREATE TABLE IF NOT EXISTS role_definitions (
    builtin BOOL NOT NULL DEFAULT (TRUE),
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    permissions_json STRING(MAX) NOT NULL DEFAULT ('[]'),
    relation_name STRING(MAX) NOT NULL,
    role_key STRING(MAX),
    scope_kind STRING(MAX) NOT NULL,
    source_version STRING(MAX) NOT NULL DEFAULT (''),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (role_key)
);

CREATE TABLE IF NOT EXISTS saved_queries (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    created_by STRING(MAX) DEFAULT (''),
    description STRING(MAX) DEFAULT (''),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    name STRING(MAX) NOT NULL,
    sql_text STRING(MAX) NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS schemas (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    created_by STRING(MAX) DEFAULT (''),
    id STRING(MAX),
    is_default BOOL NOT NULL DEFAULT (FALSE),
    message STRING(MAX) DEFAULT (''),
    schema STRING(MAX) NOT NULL,
    type STRING(MAX) NOT NULL,
    version INT64 NOT NULL DEFAULT (1),
    visibility STRING(MAX) NOT NULL DEFAULT ('private'),
    PRIMARY KEY (id)
);

CREATE TABLE IF NOT EXISTS secrets (
    algorithm STRING(MAX) NOT NULL DEFAULT ('RS256'),
    ciphertext BYTES(MAX) NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    encryption_key_id STRING(MAX) DEFAULT (''),
    expires_at TIMESTAMP,
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    nonce BYTES(MAX),
    public_key BYTES(MAX),
    secret_type STRING(MAX) NOT NULL,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS sessions (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    expires_at TIMESTAMP,
    fingerprint STRING(MAX) DEFAULT (''),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    ip_address STRING(MAX) DEFAULT (''),
    last_active_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    metadata STRING(MAX) DEFAULT ('{}'),
    org_id STRING(MAX),
    revoked_at TIMESTAMP,
    token_hash STRING(MAX) NOT NULL DEFAULT (''),
    user_agent STRING(MAX) DEFAULT (''),
    user_id STRING(MAX) NOT NULL,
    spx_25a174e7612ab805_m BOOL AS (IF(expires_at IS NOT NULL, TRUE, NULL)) STORED,
    spx_ebaaca7f82ec20ff_m BOOL AS (IF(revoked_at IS NOT NULL, TRUE, NULL)) STORED,
    spx_f44732e7743e5266_m BOOL AS (IF(token_hash != '', TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS settings (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    data STRING(MAX) NOT NULL DEFAULT ('{}'),
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    scope STRING(MAX) NOT NULL DEFAULT ('instance'),
    scope_id STRING(MAX) NOT NULL DEFAULT (''),
    type STRING(MAX) NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS tokens (
    application_id STRING(MAX) DEFAULT (''),
    audience STRING(MAX) DEFAULT (''),
    auth_method STRING(MAX) DEFAULT (''),
    auth_time TIMESTAMP,
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    expires_at TIMESTAMP,
    id STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    last_used TIMESTAMP,
    name STRING(MAX) DEFAULT (''),
    refresh_token_id STRING(MAX) DEFAULT (''),
    revoked_at TIMESTAMP,
    scopes STRING(MAX) NOT NULL DEFAULT ('[]'),
    session_id STRING(MAX),
    token_hash STRING(MAX) NOT NULL,
    type STRING(MAX) NOT NULL,
    user_id STRING(MAX),
    spx_00dfb2ef603f2554_m BOOL AS (IF(expires_at IS NOT NULL, TRUE, NULL)) STORED,
    spx_3b0e5789b3a74766_m BOOL AS (IF(revoked_at IS NOT NULL, TRUE, NULL)) STORED,
    spx_9f4d778a49b8edbb_m BOOL AS (IF(application_id != '', TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, id)
);

CREATE TABLE IF NOT EXISTS unique_fields (
    field_name STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    normalized_value STRING(MAX) NOT NULL,
    resource_type STRING(MAX) NOT NULL DEFAULT (''),
    scope_id STRING(MAX) NOT NULL DEFAULT (''),
    user_id STRING(MAX) NOT NULL,
    PRIMARY KEY (instance_id, scope_id, field_name, normalized_value)
);

CREATE TABLE IF NOT EXISTS users (
    created_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    display_name STRING(MAX) DEFAULT (''),
    id STRING(MAX) NOT NULL,
    identifier STRING(MAX) NOT NULL,
    instance_id STRING(MAX) NOT NULL,
    metadata STRING(MAX) DEFAULT ('{}'),
    org_id STRING(MAX),
    schema_id STRING(MAX) DEFAULT (''),
    state STRING(MAX) NOT NULL DEFAULT ('active'),
    updated_at TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    user_type STRING(MAX) NOT NULL DEFAULT ('human'),
    spx_1118f7aa842bd33a_m BOOL AS (IF(org_id IS NULL, TRUE, NULL)) STORED,
    PRIMARY KEY (instance_id, id)
);

CREATE INDEX IF NOT EXISTS idx_actions_instance_hook ON actions(instance_id, hook, enabled);

CREATE INDEX IF NOT EXISTS idx_actions_instance_org ON actions(instance_id, org_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_apps_7cc14b94fcd29467 ON apps(instance_id, client_id);

CREATE INDEX IF NOT EXISTS idx_apps_instance_client ON apps(instance_id, client_id);

CREATE INDEX IF NOT EXISTS idx_apps_instance_org ON apps(instance_id, org_id);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_auth_states_instance_code ON auth_states(instance_id, code, spx_8f77f43702bb7948_m);

CREATE INDEX IF NOT EXISTS idx_auth_states_instance_expires ON auth_states(instance_id, expires_at);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_auth_states_instance_state ON auth_states(instance_id, state, spx_2db8f54e942ce572_m);

CREATE INDEX IF NOT EXISTS idx_auth_states_instance_type ON auth_states(instance_id, type);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_cache_instance_expires ON cache(instance_id, expires_at, spx_48a38ab3fc5d5532_m);

ALTER TABLE credentials ADD CONSTRAINT fk_credentials_cc01f17609fb580d FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_credentials_instance_type ON credentials(instance_id, user_id, type);

CREATE INDEX IF NOT EXISTS idx_credentials_instance_user ON credentials(instance_id, user_id);

ALTER TABLE domains ADD CONSTRAINT fk_domains_09620677da6a0e7a FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_domains_instance ON domains(instance_id);

CREATE INDEX IF NOT EXISTS idx_domains_instance_org ON domains(instance_id, org_id);

CREATE UNIQUE NULL_FILTERED INDEX IF NOT EXISTS idx_domains_instance_primary ON domains(instance_id, spx_a4a0c49196eac339_m);

CREATE UNIQUE NULL_FILTERED INDEX IF NOT EXISTS idx_domains_org_primary ON domains(instance_id, org_id, spx_4d952f654d511ed6_m);

CREATE INDEX IF NOT EXISTS idx_effects_cleanup ON effects(instance_id, status, completed_at);

CREATE INDEX IF NOT EXISTS idx_effects_due ON effects(instance_id, status, next_retry_at, lease_expires_at);

CREATE INDEX IF NOT EXISTS idx_effects_event ON effects(instance_id, event_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_effects_source_key ON effects(instance_id, source_key);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_events_actor ON events(instance_id, actor_id, spx_8b9bddb838e0747e_m);

CREATE INDEX IF NOT EXISTS idx_events_aggregate ON events(instance_id, aggregate_id, aggregate_type);

CREATE INDEX IF NOT EXISTS idx_events_category ON events(instance_id, category, created_at);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_events_client ON events(instance_id, client_id, spx_7284230623d631ba_m);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_events_delegation ON events(instance_id, delegation_type, spx_36df21b0222c6b5b_m);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_events_flow ON events(instance_id, flow_id, spx_d166a9f8bccbf4b1_m);

CREATE INDEX IF NOT EXISTS idx_events_instance_created ON events(instance_id, created_at);

CREATE INDEX IF NOT EXISTS idx_events_instance_type_created ON events(instance_id, event_type, created_at);

CREATE INDEX IF NOT EXISTS idx_events_org ON events(instance_id, org_id, created_at);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_events_request ON events(instance_id, request_id, spx_87db3dfee055a026_m);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_events_ship ON events(instance_id, spx_008e7a4af975252b_m);

CREATE INDEX IF NOT EXISTS idx_fga_models_active ON fga_authorization_models(scope_id, store_id, is_active, created_at);

CREATE UNIQUE INDEX IF NOT EXISTS uk_fga_stores_a51aeb6f7be48385 ON fga_stores(store_id);

CREATE INDEX IF NOT EXISTS idx_fga_tuple_changes_lookup ON fga_tuple_changes(scope_id, store_id, seq);

CREATE INDEX IF NOT EXISTS idx_fga_tuples_lookup ON fga_tuples(scope_id, store_id, object_type, object_id, relation);

CREATE INDEX IF NOT EXISTS idx_fga_tuples_user ON fga_tuples(scope_id, store_id, user_type, user_id, user_relation);

CREATE INDEX IF NOT EXISTS idx_fingerprints_instance_type ON fingerprints(instance_id, type);

CREATE UNIQUE INDEX IF NOT EXISTS uk_groups_4f9e372ac213e76f ON `groups`(instance_id, org_id, name);

CREATE UNIQUE NULL_FILTERED INDEX IF NOT EXISTS idx_groups_instance_name_no_org ON `groups`(instance_id, name, spx_6ec06a62084df108_m);

CREATE INDEX IF NOT EXISTS idx_groups_instance_org ON `groups`(instance_id, org_id);

ALTER TABLE instance_trust_links ADD CONSTRAINT fk_instance_trust_links_be722791f7faa9d8 FOREIGN KEY (child_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE;

ALTER TABLE instances ADD CONSTRAINT fk_instances_8c1ab80b176c67ba FOREIGN KEY (parent_instance_id) REFERENCES instances(instance_id) ON DELETE NO ACTION;

ALTER TABLE instances ADD CONSTRAINT fk_instances_a92be6fd032759ff FOREIGN KEY (parent_instance_id, owner_org_id) REFERENCES orgs(instance_id, id) ON DELETE NO ACTION;

CREATE INDEX IF NOT EXISTS idx_jobs_instance_due_lease ON jobs(instance_id, enabled, next_run_at, lease_expires_at);

ALTER TABLE linked_identities ADD CONSTRAINT fk_linked_identities_87093a1523d6ad4b FOREIGN KEY (instance_id, provider_id) REFERENCES providers(instance_id, id) ON DELETE CASCADE;

ALTER TABLE linked_identities ADD CONSTRAINT fk_linked_identities_4b30981635f17ad0 FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE;

CREATE UNIQUE INDEX IF NOT EXISTS uk_linked_identities_9dc9bb1751f5e05b ON linked_identities(instance_id, provider_id, external_sub);

CREATE INDEX IF NOT EXISTS idx_linked_identities_instance_user ON linked_identities(instance_id, user_id);

ALTER TABLE login_flow_assets ADD CONSTRAINT fk_login_flow_assets_adbc5644186703cd FOREIGN KEY (instance_id, login_flow_id) REFERENCES login_flows(instance_id, id) ON DELETE CASCADE;

CREATE UNIQUE INDEX IF NOT EXISTS uk_login_flow_assets_fe47ca5fd93c1632 ON login_flow_assets(instance_id, login_flow_id, slot);

CREATE INDEX IF NOT EXISTS idx_login_flow_assets_instance_flow ON login_flow_assets(instance_id, login_flow_id);

CREATE UNIQUE NULL_FILTERED INDEX IF NOT EXISTS idx_login_flows_instance_default ON login_flows(instance_id, spx_7b74940375556728_m);

CREATE INDEX IF NOT EXISTS idx_login_flows_instance_org ON login_flows(instance_id, org_id);

CREATE INDEX IF NOT EXISTS idx_login_flows_instance_state ON login_flows(instance_id, state, enabled);

ALTER TABLE memberships ADD CONSTRAINT fk_memberships_5ac9f85f54e73848 FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_memberships_instance_resource ON memberships(instance_id, resource_type, resource_id);

CREATE INDEX IF NOT EXISTS idx_memberships_instance_user ON memberships(instance_id, user_id, resource_type);

CREATE UNIQUE NULL_FILTERED INDEX IF NOT EXISTS idx_oidc_auth_requests_code ON oidc_auth_requests(instance_id, code, spx_21d48265031a895a_m);

CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_client ON oidc_auth_requests(instance_id, client_id);

CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_created ON oidc_auth_requests(instance_id, created_at);

CREATE INDEX IF NOT EXISTS idx_oidc_auth_requests_instance_expires ON oidc_auth_requests(instance_id, expires_at);

CREATE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_instance_expires ON oidc_rp_auth_states(instance_id, expires_at);

CREATE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_provider ON oidc_rp_auth_states(instance_id, provider_id);

CREATE UNIQUE INDEX IF NOT EXISTS idx_oidc_rp_auth_states_state ON oidc_rp_auth_states(instance_id, state);

ALTER TABLE orgs ADD CONSTRAINT fk_orgs_a1942fb5957d4982 FOREIGN KEY (instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE;

CREATE UNIQUE INDEX IF NOT EXISTS uk_orgs_1f9e20efebadb052 ON orgs(instance_id, name);

CREATE INDEX IF NOT EXISTS idx_orgs_instance_state ON orgs(instance_id, state);

CREATE UNIQUE INDEX IF NOT EXISTS uk_projects_1da32ee6b9e06ee5 ON projects(instance_id, org_id, name);

CREATE UNIQUE NULL_FILTERED INDEX IF NOT EXISTS idx_projects_instance_name_no_org ON projects(instance_id, name, spx_3c82cf40a95bc1ce_m);

CREATE INDEX IF NOT EXISTS idx_projects_instance_org ON projects(instance_id, org_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_providers_f22536697181dc2e ON providers(instance_id, org_id, display_name);

CREATE UNIQUE NULL_FILTERED INDEX IF NOT EXISTS idx_providers_instance_name_no_org ON providers(instance_id, display_name, spx_2721e3196171e498_m);

CREATE INDEX IF NOT EXISTS idx_providers_instance_org ON providers(instance_id, org_id);

CREATE INDEX IF NOT EXISTS idx_providers_instance_protocol ON providers(instance_id, protocol, enabled);

CREATE INDEX IF NOT EXISTS idx_providers_instance_sort ON providers(instance_id, display_order, display_name);

CREATE INDEX IF NOT EXISTS idx_retention_policies_instance_priority ON retention_policies(instance_id, priority);

ALTER TABLE role_assignments ADD CONSTRAINT fk_role_assignments_b0c80632d096d50d FOREIGN KEY (enforcement_instance_id) REFERENCES instances(instance_id) ON DELETE CASCADE;

ALTER TABLE role_assignments ADD CONSTRAINT fk_role_assignments_7cb20404653323ae FOREIGN KEY (origin_instance_id) REFERENCES instances(instance_id) ON DELETE NO ACTION;

ALTER TABLE role_assignments ADD CONSTRAINT fk_role_assignments_5c485b12355c26cd FOREIGN KEY (role_key) REFERENCES role_definitions(role_key) ON DELETE NO ACTION;

CREATE INDEX IF NOT EXISTS idx_role_assignments_instance_scope ON role_assignments(enforcement_instance_id, scope_kind, scope_id);

CREATE INDEX IF NOT EXISTS idx_role_assignments_principal ON role_assignments(principal_ref, revoked_at, expires_at);

CREATE INDEX IF NOT EXISTS idx_role_assignments_role_source ON role_assignments(role_key, source_kind);

CREATE INDEX IF NOT EXISTS idx_saved_queries_instance_name ON saved_queries(instance_id, name);

CREATE INDEX IF NOT EXISTS idx_schema_default ON schemas(type, is_default);

CREATE INDEX IF NOT EXISTS idx_schema_type ON schemas(type);

CREATE INDEX IF NOT EXISTS idx_schema_version ON schemas(type, version);

CREATE INDEX IF NOT EXISTS idx_secrets_instance_enc_key ON secrets(instance_id, encryption_key_id);

CREATE INDEX IF NOT EXISTS idx_secrets_instance_type ON secrets(instance_id, secret_type);

ALTER TABLE sessions ADD CONSTRAINT fk_sessions_ca7ce9e77ef4176a FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE;

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_sessions_instance_expires ON sessions(instance_id, expires_at, spx_25a174e7612ab805_m);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_sessions_instance_revoked ON sessions(instance_id, revoked_at, spx_ebaaca7f82ec20ff_m);

CREATE UNIQUE NULL_FILTERED INDEX IF NOT EXISTS idx_sessions_instance_token_unique ON sessions(instance_id, token_hash, spx_f44732e7743e5266_m);

CREATE INDEX IF NOT EXISTS idx_sessions_instance_user ON sessions(instance_id, user_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_settings_b80d13a3fd9118ff ON settings(instance_id, type, scope, scope_id);

CREATE INDEX IF NOT EXISTS idx_settings_instance_type ON settings(instance_id, type, scope);

ALTER TABLE tokens ADD CONSTRAINT fk_tokens_8befad759e0f2b4b FOREIGN KEY (instance_id, session_id) REFERENCES sessions(instance_id, id) ON DELETE NO ACTION;

ALTER TABLE tokens ADD CONSTRAINT fk_tokens_8552810c8e4aefe0 FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE;

CREATE UNIQUE INDEX IF NOT EXISTS uk_tokens_4c7bc010dabe07b4 ON tokens(instance_id, token_hash);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_tokens_instance_app ON tokens(instance_id, application_id, spx_9f4d778a49b8edbb_m);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_tokens_instance_expires ON tokens(instance_id, expires_at, spx_00dfb2ef603f2554_m);

CREATE NULL_FILTERED INDEX IF NOT EXISTS idx_tokens_instance_revoked ON tokens(instance_id, revoked_at, spx_3b0e5789b3a74766_m);

CREATE INDEX IF NOT EXISTS idx_tokens_instance_session ON tokens(instance_id, session_id);

CREATE INDEX IF NOT EXISTS idx_tokens_instance_type ON tokens(instance_id, type, user_id);

CREATE INDEX IF NOT EXISTS idx_tokens_instance_user ON tokens(instance_id, user_id);

ALTER TABLE unique_fields ADD CONSTRAINT fk_unique_fields_13b8060683422d8c FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_unique_fields_instance_lookup ON unique_fields(instance_id, normalized_value, field_name);

CREATE INDEX IF NOT EXISTS idx_unique_fields_instance_resource ON unique_fields(instance_id, user_id);

CREATE UNIQUE INDEX IF NOT EXISTS uk_users_8fa36ca7c7768f49 ON users(instance_id, org_id, identifier);

CREATE UNIQUE NULL_FILTERED INDEX IF NOT EXISTS idx_users_instance_identifier_no_org ON users(instance_id, identifier, spx_1118f7aa842bd33a_m);

CREATE INDEX IF NOT EXISTS idx_users_instance_org ON users(instance_id, org_id);

CREATE INDEX IF NOT EXISTS idx_users_instance_state ON users(instance_id, state);

CREATE INDEX IF NOT EXISTS idx_users_instance_type ON users(instance_id, user_type);
