-- +goose Up
ALTER TABLE apps ADD COLUMN post_logout_redirect_uris TEXT NOT NULL DEFAULT '[]';
ALTER TABLE oidc_auth_requests ADD COLUMN session_id TEXT NOT NULL DEFAULT '';

-- +goose Down
