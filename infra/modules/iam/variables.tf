variable "project_id" {
  description = "GCP project ID"
  type        = string
}

variable "spanner_instance_name" {
  description = "Spanner instance name for IAM bindings"
  type        = string
}

variable "spanner_database_name" {
  description = "Spanner database name for IAM bindings"
  type        = string
}
