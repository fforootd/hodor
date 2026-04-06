variable "project_id" {
  description = "Google Cloud project ID."
  type        = string
}

variable "region" {
  description = "Primary region for Cloud Run, Artifact Registry, and Spanner."
  type        = string
}

variable "platform_domain" {
  description = "Base platform domain. Root and demo instances will live under subdomains of this zone."
  type        = string
}

variable "name_prefix" {
  description = "Prefix applied to provisioned resource names."
  type        = string
  default     = "zitadel"
}

variable "labels" {
  description = "Optional labels applied to supported resources."
  type        = map(string)
  default     = {}
}

variable "artifact_registry_repository_id" {
  description = "Artifact Registry repository ID for the Zitadel image."
  type        = string
  default     = "zitadel"
}

variable "container_image" {
  description = "Fully-qualified container image override. Leave empty to build from the Artifact Registry repo + image name/tag vars."
  type        = string
  default     = ""
}

variable "image_name" {
  description = "Image name inside Artifact Registry when container_image is empty."
  type        = string
  default     = "zitadel"
}

variable "image_tag" {
  description = "Image tag inside Artifact Registry when container_image is empty."
  type        = string
  default     = "latest"
}

variable "runtime_service_account_id" {
  description = "Service account ID for the Cloud Run service."
  type        = string
  default     = "zitadel-runtime"
}

variable "migrator_service_account_id" {
  description = "Service account ID for the Cloud Run migration job."
  type        = string
  default     = "zitadel-migrator"
}

variable "service_cpu" {
  description = "CPU limit for the Cloud Run service container."
  type        = string
  default     = "1"
}

variable "service_memory" {
  description = "Memory limit for the Cloud Run service container."
  type        = string
  default     = "1Gi"
}

variable "service_timeout" {
  description = "Request timeout for the Cloud Run service."
  type        = string
  default     = "300s"
}

variable "service_min_instances" {
  description = "Minimum number of Cloud Run service instances kept warm."
  type        = number
  default     = 0
}

variable "service_max_instances" {
  description = "Maximum number of Cloud Run service instances."
  type        = number
  default     = 10
}

variable "container_concurrency" {
  description = "Maximum concurrent requests per Cloud Run instance."
  type        = number
  default     = 80
}

variable "job_cpu" {
  description = "CPU limit for the Cloud Run migration job."
  type        = string
  default     = "1"
}

variable "job_memory" {
  description = "Memory limit for the Cloud Run migration job."
  type        = string
  default     = "1Gi"
}

variable "job_timeout" {
  description = "Execution timeout for the Cloud Run migration job."
  type        = string
  default     = "1800s"
}

variable "spanner_instance_id" {
  description = "Spanner instance ID."
  type        = string
  default     = "zitadel"
}

variable "spanner_database_name" {
  description = "Spanner database name."
  type        = string
  default     = "zitadel"
}

variable "spanner_instance_config" {
  description = "Spanner instance config. Use a regional config such as regional-us-central1 for lower cost in v1."
  type        = string
}

variable "spanner_processing_units" {
  description = "Spanner processing units for the instance."
  type        = number
  default     = 100
}

variable "dns_managed_zone_name" {
  description = "Cloud DNS managed zone name for platform_domain."
  type        = string
  default     = "zitadel-platform"
}

variable "root_subdomain" {
  description = "Subdomain used for the root instance."
  type        = string
  default     = "root"
}

variable "demo_subdomain" {
  description = "Subdomain used for the first demo child instance."
  type        = string
  default     = "demo"
}

variable "log_level" {
  description = "Zitadel log level."
  type        = string
  default     = "info"
}

variable "admin_email" {
  description = "Bootstrap admin email used by fixtures/prod-seed.yaml."
  type        = string
  default     = "admin@zitadel.cloud"
}

variable "cookie_secret_name" {
  description = "Secret Manager secret name for ZITADEL_COOKIE_SECRETS."
  type        = string
  default     = "zitadel-cookie-secrets"
}

variable "management_secret_name" {
  description = "Secret Manager secret name for ZITADEL_MANAGEMENT_SECRET."
  type        = string
  default     = "zitadel-management-secret"
}

variable "admin_password_secret_name" {
  description = "Secret Manager secret name for ZITADEL_ADMIN_PASSWORD."
  type        = string
  default     = "zitadel-admin-password"
}

variable "admin_pat_secret_name" {
  description = "Secret Manager secret name for ZITADEL_ADMIN_PAT."
  type        = string
  default     = "zitadel-admin-pat"
}

variable "encryption_keys_secret_name" {
  description = "Optional Secret Manager secret name for ZITADEL_ENCRYPTION_KEYS."
  type        = string
  default     = "zitadel-encryption-keys"
}

variable "cookie_secrets" {
  description = "Comma-separated cookie secrets. The first secret signs and all secrets verify."
  type        = string
  sensitive   = true
}

variable "management_secret" {
  description = "Management secret used for POW challenge signing."
  type        = string
  sensitive   = true
}

variable "admin_password" {
  description = "Bootstrap admin password consumed by fixtures/prod-seed.yaml."
  type        = string
  sensitive   = true
}

variable "admin_pat" {
  description = "Bootstrap admin PAT consumed by fixtures/prod-seed.yaml."
  type        = string
  sensitive   = true
}

variable "encryption_active_key_id" {
  description = "Optional active encryption key ID."
  type        = string
  default     = ""
}

variable "encryption_keys" {
  description = "Optional JSON array for ZITADEL_ENCRYPTION_KEYS. Example: [{\"id\":\"k1\",\"secret\":\"base64-or-random\"}]"
  type        = string
  default     = ""
  sensitive   = true
}
