-- +goose Up
CREATE TABLE IF NOT EXISTS instances (
    instance_id     TEXT PRIMARY KEY,
    customer_id     TEXT NOT NULL DEFAULT '',
    state           TEXT NOT NULL DEFAULT 'active',
    primary_domain  TEXT NOT NULL DEFAULT '',
    placement_mode  TEXT NOT NULL DEFAULT 'global',
    region_key      TEXT,
    backend_key     TEXT NOT NULL DEFAULT 'default',
    created_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS instance_domains (
    domain       TEXT PRIMARY KEY,
    instance_id  TEXT NOT NULL REFERENCES instances(instance_id) ON DELETE CASCADE,
    is_primary   BOOLEAN NOT NULL DEFAULT FALSE,
    state        TEXT NOT NULL DEFAULT 'active',
    updated_at   TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_instances_customer ON instances(customer_id);
CREATE INDEX IF NOT EXISTS idx_instances_backend ON instances(backend_key);
CREATE INDEX IF NOT EXISTS idx_instance_domains_instance ON instance_domains(instance_id);

INSERT INTO instances (instance_id, customer_id, state, placement_mode, backend_key)
VALUES ('default', '', 'active', 'global', 'default')
ON CONFLICT (instance_id) DO NOTHING;

-- +goose Down
DROP TABLE IF EXISTS instance_domains;
DROP TABLE IF EXISTS instances;
