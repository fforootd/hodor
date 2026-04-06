output "artifact_registry_repository" {
  description = "Artifact Registry repository used for the Zitadel image."
  value       = google_artifact_registry_repository.images.id
}

output "artifact_registry_repository_url" {
  description = "Artifact Registry Docker URL prefix for pushed Zitadel images."
  value       = "${var.region}-docker.pkg.dev/${var.project_id}/${google_artifact_registry_repository.images.repository_id}"
}

output "container_image" {
  description = "Fully-qualified container image configured for Cloud Run."
  value       = local.container_image
}

output "cloud_run_service_name" {
  description = "Cloud Run service name."
  value       = google_cloud_run_v2_service.app.name
}

output "cloud_run_job_name" {
  description = "Cloud Run migration job name."
  value       = google_cloud_run_v2_job.migrator.name
}

output "spanner_database" {
  description = "Spanner database resource name."
  value       = local.spanner_database
}

output "spanner_instance_id" {
  description = "Spanner instance ID."
  value       = google_spanner_instance.stateful.name
}

output "load_balancer_ip" {
  description = "Global IP address for the external HTTPS load balancer."
  value       = google_compute_global_address.lb.address
}

output "platform_name_servers" {
  description = "Cloud DNS name servers for the managed zone. Delegate your registrar to these values before expecting DNS or certificates to resolve."
  value       = google_dns_managed_zone.platform.name_servers
}

output "certificate_dns_authorization_record" {
  description = "DNS authorization record that Terraform manages inside Cloud DNS for the wildcard certificate."
  value = {
    name = google_certificate_manager_dns_authorization.platform.dns_resource_record[0].name
    type = google_certificate_manager_dns_authorization.platform.dns_resource_record[0].type
    data = google_certificate_manager_dns_authorization.platform.dns_resource_record[0].data
  }
}

output "root_url" {
  description = "Root instance URL after bootstrap."
  value       = "https://${local.root_host}"
}

output "demo_url" {
  description = "Demo child instance URL after bootstrap."
  value       = "https://${local.demo_host}"
}
