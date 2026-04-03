-- +goose Up
ALTER TABLE oidc_auth_requests ADD COLUMN max_age BIGINT;
