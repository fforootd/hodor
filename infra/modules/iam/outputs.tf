output "run_sa_email" {
  description = "Runtime service account email"
  value       = google_service_account.run.email
}

output "migrator_sa_email" {
  description = "Migrator service account email"
  value       = google_service_account.migrator.email
}

output "github_deploy_sa_email" {
  description = "GitHub Actions deploy service account email"
  value       = google_service_account.github_deploy.email
}
