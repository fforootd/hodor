-- Make org_id optional on users and sessions.
-- Deleting an org now sets org_id to NULL instead of cascade-deleting rows.

-- SQLite cannot ALTER foreign key constraints, so we recreate the tables.

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
    UNIQUE (instance_id, org_id, identifier),
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL
);

INSERT INTO users_new SELECT * FROM users;
DROP TABLE users;
ALTER TABLE users_new RENAME TO users;

CREATE INDEX idx_users_instance_org ON users(instance_id, org_id);
CREATE INDEX idx_users_instance_state ON users(instance_id, state);
CREATE INDEX idx_users_instance_type ON users(instance_id, user_type);
-- Ensure identifier uniqueness for org-less users.
CREATE UNIQUE INDEX idx_users_instance_identifier_no_org
    ON users(instance_id, identifier) WHERE org_id IS NULL;

-- ���─ sessions ──

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
    FOREIGN KEY (instance_id, user_id) REFERENCES users(instance_id, id) ON DELETE CASCADE,
    FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL
);

INSERT INTO sessions_new SELECT * FROM sessions;
DROP TABLE sessions;
ALTER TABLE sessions_new RENAME TO sessions;

CREATE INDEX idx_sessions_instance_user ON sessions(instance_id, user_id);
CREATE INDEX idx_sessions_instance_expires
    ON sessions(instance_id, expires_at) WHERE expires_at IS NOT NULL;
CREATE INDEX idx_sessions_instance_revoked
    ON sessions(instance_id, revoked_at) WHERE revoked_at IS NOT NULL;
CREATE UNIQUE INDEX idx_sessions_instance_token_unique
    ON sessions(instance_id, token_hash) WHERE token_hash != '';
