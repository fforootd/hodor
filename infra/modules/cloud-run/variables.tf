variable "region" {
  type = string
}

variable "environment" {
  type = string
}

variable "run_sa_email" {
  type = string
}

variable "migrator_sa_email" {
  type = string
}

variable "cpu" {
  type    = string
  default = "1"
}

variable "memory" {
  type    = string
  default = "512Mi"
}

variable "min_instances" {
  type    = number
  default = 0
}

variable "max_instances" {
  type    = number
  default = 10
}
