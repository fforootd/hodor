-- +goose Up
-- Tighten the MVP auth/session invariants around exact-once durable state.

CREATE UNIQUE INDEX IF NOT EXISTS idx_sessions_instance_token_unique
    ON sessions(instance_id, token_hash)
    WHERE token_hash != '';

DROP INDEX IF EXISTS idx_oidc_auth_requests_code;
CREATE UNIQUE INDEX idx_oidc_auth_requests_code
    ON oidc_auth_requests(instance_id, code)
    WHERE code != '';

-- +goose Down
DROP INDEX IF EXISTS idx_sessions_instance_token_unique;
DROP INDEX IF EXISTS idx_oidc_auth_requests_code;
CREATE INDEX idx_oidc_auth_requests_code
    ON oidc_auth_requests(instance_id, code)
    WHERE code != '';
