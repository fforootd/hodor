variable "github_repo" {
  description = "GitHub repository in owner/repo format"
  type        = string
}

variable "github_deploy_sa_id" {
  description = "Full resource name of the GitHub deploy service account"
  type        = string
}
