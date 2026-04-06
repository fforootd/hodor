-- +goose Up
CREATE TABLE IF NOT EXISTS role_definitions (
    role_key         STRING(MAX) NOT NULL,
    relation_name    STRING(MAX) NOT NULL,
    scope_kind       STRING(MAX) NOT NULL,
    permissions_json STRING(MAX) NOT NULL DEFAULT ('[]'),
    builtin          BOOL NOT NULL DEFAULT (TRUE),
    source_version   STRING(MAX) NOT NULL DEFAULT (''),
    created_at       TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at       TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (role_key)
);

CREATE TABLE IF NOT EXISTS role_assignments (
    assignment_id            STRING(MAX) NOT NULL,
    enforcement_instance_id  STRING(MAX) NOT NULL,
    scope_kind               STRING(MAX) NOT NULL,
    scope_id                 STRING(MAX) NOT NULL,
    principal_ref            STRING(MAX) NOT NULL,
    role_key                 STRING(MAX) NOT NULL,
    source_kind              STRING(MAX) NOT NULL DEFAULT ('manual'),
    origin_instance_id       STRING(MAX),
    approved_by              STRING(MAX),
    reason                   STRING(MAX),
    expires_at               TIMESTAMP,
    revoked_at               TIMESTAMP,
    created_at               TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    updated_at               TIMESTAMP NOT NULL DEFAULT (CURRENT_TIMESTAMP()),
    PRIMARY KEY (assignment_id)
);

CREATE INDEX IF NOT EXISTS idx_role_assignments_instance_scope
    ON role_assignments(enforcement_instance_id, scope_kind, scope_id);
CREATE INDEX IF NOT EXISTS idx_role_assignments_principal
    ON role_assignments(principal_ref, revoked_at, expires_at);
CREATE INDEX IF NOT EXISTS idx_role_assignments_role_source
    ON role_assignments(role_key, source_kind);

-- +goose Down
DROP TABLE IF EXISTS role_assignments;
DROP TABLE IF EXISTS role_definitions;
