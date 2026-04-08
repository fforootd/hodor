# ─────────────────────────────────────────────────────────────────────────────
# Zitadel GCP Platform Infrastructure
#
# Managed by Google Cloud Infrastructure Manager (auto-apply on merge).
# Use environment-specific tfvars: tofu apply -var-file=environments/dev.tfvars
# ─────────────────────────────────────────────────────────────────────────────

module "project" {
  source     = "./modules/project"
  project_id = var.project_id
}

module "artifact_registry" {
  source = "./modules/artifact-registry"
  region = var.region

  depends_on = [module.project]
}

module "spanner" {
  source = "./modules/spanner"

  instance_name    = var.spanner_instance_name
  database_name    = var.spanner_database_name
  spanner_config   = var.spanner_config
  processing_units = var.spanner_processing_units
  environment      = var.environment

  depends_on = [module.project]
}

module "iam" {
  source = "./modules/iam"

  project_id            = var.project_id
  spanner_instance_name = module.spanner.instance_name
  spanner_database_name = module.spanner.database_name
}

module "workload_identity" {
  source = "./modules/workload-identity"

  github_repo         = var.github_repo
  github_deploy_sa_id = module.iam.github_deploy_sa_email

  depends_on = [module.project]
}

module "certificate_map" {
  source      = "./modules/certificate-map"
  environment = var.environment

  depends_on = [module.project]
}

module "cloud_run" {
  source = "./modules/cloud-run"

  region            = var.region
  environment       = var.environment
  run_sa_email      = module.iam.run_sa_email
  migrator_sa_email = module.iam.migrator_sa_email
  cpu               = var.cloud_run_cpu
  memory            = var.cloud_run_memory
  min_instances     = var.cloud_run_min_instances
  max_instances     = var.cloud_run_max_instances

  depends_on = [module.project]
}

module "load_balancer" {
  source = "./modules/load-balancer"

  region                 = var.region
  environment            = var.environment
  cloud_run_service_name = module.cloud_run.service_name
  certificate_map_id     = module.certificate_map.map_id
  cdn_enabled            = var.cdn_enabled
}

module "dns" {
  source = "./modules/dns"

  zone_name     = var.dns_zone_name
  base_domain   = var.base_domain
  environment   = var.environment
  lb_ip_address = module.load_balancer.ip_address
}
