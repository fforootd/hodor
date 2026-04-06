-- +goose Up
ALTER TABLE apps
    ADD COLUMN IF NOT EXISTS post_logout_redirect_uris JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE oidc_auth_requests
    ADD COLUMN IF NOT EXISTS session_id TEXT NOT NULL DEFAULT '';

-- +goose Down
