-- +goose Up
ALTER TABLE apps
    ADD COLUMN post_logout_redirect_uris STRING(MAX) DEFAULT ('[]');

ALTER TABLE oidc_auth_requests
    ADD COLUMN session_id STRING(MAX) DEFAULT ('');

-- +goose Down
