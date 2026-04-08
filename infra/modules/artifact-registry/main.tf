resource "google_artifact_registry_repository" "zitadel" {
  location      = var.region
  repository_id = var.repository_id
  format        = "DOCKER"
  description   = "Zitadel container images"

  cleanup_policies {
    id     = "keep-recent"
    action = "KEEP"

    most_recent_versions {
      keep_count = 25
    }
  }
}
