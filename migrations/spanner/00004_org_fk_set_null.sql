-- +goose Up
-- Change remaining org FK constraints from CASCADE to SET NULL.
-- Users/sessions already fixed in 00003. This covers the rest.

ALTER TABLE apps ALTER COLUMN org_id STRING(MAX);
ALTER TABLE apps ALTER FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;

ALTER TABLE login_flows ALTER FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;

ALTER TABLE login_flow_assets ALTER COLUMN org_id STRING(MAX);
ALTER TABLE login_flow_assets ALTER FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;

ALTER TABLE providers ALTER COLUMN org_id STRING(MAX);
ALTER TABLE providers ALTER FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;

ALTER TABLE groups ALTER COLUMN org_id STRING(MAX);
ALTER TABLE groups ALTER FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;

ALTER TABLE projects ALTER COLUMN org_id STRING(MAX);
ALTER TABLE projects ALTER FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;

ALTER TABLE actions ALTER COLUMN org_id STRING(MAX);
ALTER TABLE actions ALTER FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;

ALTER TABLE domains ALTER FOREIGN KEY (instance_id, org_id) REFERENCES orgs(instance_id, id) ON DELETE SET NULL;

-- +goose Down
