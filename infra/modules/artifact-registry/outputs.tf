output "repository_url" {
  description = "Docker repository URL for pushing images"
  value       = "${var.region}-docker.pkg.dev/${google_artifact_registry_repository.zitadel.project}/${google_artifact_registry_repository.zitadel.repository_id}"
}

output "repository_id" {
  description = "Repository ID"
  value       = google_artifact_registry_repository.zitadel.repository_id
}
