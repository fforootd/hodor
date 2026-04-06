-- +goose Up
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

-- +goose Down
SELECT 1;
