-- +goose Up
CREATE TABLE IF NOT EXISTS role_definitions (
    role_key         TEXT PRIMARY KEY,
    relation_name    TEXT NOT NULL,
    scope_kind       TEXT NOT NULL,
    permissions_json JSONB NOT NULL DEFAULT '[]'::jsonb,
    builtin          BOOLEAN NOT NULL DEFAULT TRUE,
    source_version   TEXT NOT NULL DEFAULT '',
    created_at       TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT NOW()
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
    expires_at               TIMESTAMPTZ,
    revoked_at               TIMESTAMPTZ,
    created_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at               TIMESTAMPTZ NOT NULL DEFAULT NOW(),
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

-- +goose Down
DROP TABLE IF EXISTS role_assignments;
DROP TABLE IF EXISTS role_definitions;
