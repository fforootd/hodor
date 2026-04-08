output "spanner_database" {
  description = "Full Spanner database resource name"
  value       = module.spanner.database_id
}

output "cloud_run_service_name" {
  description = "Cloud Run service name"
  value       = module.cloud_run.service_name
}

output "cloud_run_migrate_job_name" {
  description = "Cloud Run migration job name"
  value       = module.cloud_run.migrate_job_name
}

output "load_balancer_ip" {
  description = "Global static IP address for the load balancer"
  value       = module.load_balancer.ip_address
}

output "dns_name_servers" {
  description = "Name servers for the DNS zone (delegate your domain to these)"
  value       = module.dns.name_servers
}

output "artifact_registry_url" {
  description = "Artifact Registry Docker repository URL"
  value       = module.artifact_registry.repository_url
}

output "wif_provider" {
  description = "Workload Identity Federation provider resource name (for GitHub Actions)"
  value       = module.workload_identity.provider_name
}

output "github_deploy_sa_email" {
  description = "Service account email for GitHub Actions deployment"
  value       = module.iam.github_deploy_sa_email
}

output "certificate_map_name" {
  description = "Certificate Manager map name (runtime infra.rs populates entries)"
  value       = module.certificate_map.map_name
}

output "url_map_name" {
  description = "GLB URL map name (runtime infra.rs adds host rules)"
  value       = module.load_balancer.url_map_name
}

output "backend_service_name" {
  description = "GLB backend service name (referenced by cloud.gcp config)"
  value       = module.load_balancer.backend_service_name
}
