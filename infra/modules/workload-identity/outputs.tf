output "pool_name" {
  description = "Workload Identity Pool full resource name"
  value       = google_iam_workload_identity_pool.github.name
}

output "provider_name" {
  description = "Workload Identity Pool Provider full resource name (for GitHub Actions auth)"
  value       = google_iam_workload_identity_pool_provider.github.name
}
