-- +goose Up
CREATE TABLE IF NOT EXISTS fga_authorization_models (
    instance_id      TEXT NOT NULL,
    store_id         TEXT NOT NULL,
    model_id         TEXT NOT NULL,
    schema_version   TEXT NOT NULL,
    compiled_model   TEXT NOT NULL,
    custom_model     TEXT NOT NULL DEFAULT '{}',
    module_fragments TEXT NOT NULL DEFAULT '[]',
    is_active        INTEGER NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, store_id, model_id)
);
CREATE INDEX IF NOT EXISTS idx_fga_models_active
    ON fga_authorization_models(instance_id, store_id, is_active, created_at DESC);

CREATE TABLE IF NOT EXISTS fga_tuples (
    instance_id   TEXT NOT NULL,
    store_id      TEXT NOT NULL,
    object_type   TEXT NOT NULL,
    object_id     TEXT NOT NULL,
    relation      TEXT NOT NULL,
    user_type     TEXT NOT NULL,
    user_id       TEXT NOT NULL,
    user_relation TEXT NOT NULL DEFAULT '',
    raw_object    TEXT NOT NULL,
    raw_user      TEXT NOT NULL,
    inserted_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (instance_id, store_id, object_type, object_id, relation, user_type, user_id, user_relation)
);
CREATE INDEX IF NOT EXISTS idx_fga_tuples_lookup
    ON fga_tuples(instance_id, store_id, object_type, object_id, relation);
CREATE INDEX IF NOT EXISTS idx_fga_tuples_user
    ON fga_tuples(instance_id, store_id, user_type, user_id, user_relation);

CREATE TABLE IF NOT EXISTS fga_tuple_changes (
    seq                    BIGSERIAL PRIMARY KEY,
    instance_id            TEXT NOT NULL,
    store_id               TEXT NOT NULL,
    operation              TEXT NOT NULL,
    object_type            TEXT NOT NULL,
    object_id              TEXT NOT NULL,
    relation               TEXT NOT NULL,
    user_type              TEXT NOT NULL,
    user_id                TEXT NOT NULL,
    user_relation          TEXT NOT NULL DEFAULT '',
    raw_object             TEXT NOT NULL,
    raw_user               TEXT NOT NULL,
    authorization_model_id TEXT NOT NULL DEFAULT '',
    created_at             TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_fga_tuple_changes_lookup
    ON fga_tuple_changes(instance_id, store_id, seq);

INSERT INTO fga_instance_stores (instance_id, store_id)
SELECT DISTINCT instance_id, instance_id
FROM orgs
ON CONFLICT (instance_id) DO NOTHING;

INSERT INTO fga_tuples (
    instance_id,
    store_id,
    object_type,
    object_id,
    relation,
    user_type,
    user_id,
    user_relation,
    raw_object,
    raw_user
)
SELECT
    memberships.instance_id,
    memberships.instance_id,
    memberships.resource_type,
    memberships.resource_id,
    memberships.role,
    'user',
    memberships.user_id,
    '',
    memberships.resource_type || ':' || memberships.resource_id,
    'user:' || memberships.user_id
FROM memberships
ON CONFLICT DO NOTHING;

INSERT INTO fga_tuple_changes (
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
    authorization_model_id
)
SELECT
    memberships.instance_id,
    memberships.instance_id,
    'WRITE',
    memberships.resource_type,
    memberships.resource_id,
    memberships.role,
    'user',
    memberships.user_id,
    '',
    memberships.resource_type || ':' || memberships.resource_id,
    'user:' || memberships.user_id,
    ''
FROM memberships
WHERE EXISTS (
    SELECT 1
    FROM fga_tuples
    WHERE fga_tuples.instance_id = memberships.instance_id
      AND fga_tuples.store_id = memberships.instance_id
      AND fga_tuples.object_type = memberships.resource_type
      AND fga_tuples.object_id = memberships.resource_id
      AND fga_tuples.relation = memberships.role
      AND fga_tuples.user_type = 'user'
      AND fga_tuples.user_id = memberships.user_id
      AND fga_tuples.user_relation = ''
);

-- +goose Down
DROP TABLE IF EXISTS fga_tuple_changes;
DROP TABLE IF EXISTS fga_tuples;
DROP TABLE IF EXISTS fga_authorization_models;
