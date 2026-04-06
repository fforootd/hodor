-- +goose Up
-- Make org_id optional on users and sessions.
-- Deleting an org sets org_id to NULL instead of cascade-deleting rows.

ALTER TABLE users ALTER COLUMN org_id STRING(MAX);
ALTER TABLE users ALTER FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;

ALTER TABLE sessions ALTER COLUMN org_id STRING(MAX);
ALTER TABLE sessions ALTER FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;
