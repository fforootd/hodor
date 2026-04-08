resource "google_cloud_run_v2_service" "zitadel" {
  name     = "zitadel-${var.environment}"
  location = var.region
  ingress  = "INGRESS_TRAFFIC_INTERNAL_LOAD_BALANCER"

  template {
    service_account = var.run_sa_email

    scaling {
      min_instance_count = var.min_instances
      max_instance_count = var.max_instances
    }

    containers {
      image   = "us-docker.pkg.dev/cloudrun/container/hello"
      command = ["zitadel"]
      args    = ["start", "-c", "/etc/zitadel/config/${var.environment}.toml"]

      ports {
        container_port = 8080
      }

      resources {
        limits = {
          cpu    = var.cpu
          memory = var.memory
        }
      }

      startup_probe {
        http_get {
          path = "/healthz"
          port = 8080
        }
        initial_delay_seconds = 5
        period_seconds        = 5
        failure_threshold     = 12
      }

      liveness_probe {
        http_get {
          path = "/healthz"
          port = 8080
        }
        period_seconds    = 30
        failure_threshold = 3
      }
    }
  }

  lifecycle {
    ignore_changes = [
      template[0].containers[0].image,
      template[0].containers[0].env,
    ]
  }
}

resource "google_cloud_run_v2_service_iam_member" "allow_lb" {
  name     = google_cloud_run_v2_service.zitadel.name
  location = var.region
  role     = "roles/run.invoker"
  member   = "allUsers"
}

resource "google_cloud_run_v2_job" "migrate" {
  name     = "zitadel-migrate-${var.environment}"
  location = var.region

  template {
    task_count = 1

    template {
      service_account  = var.migrator_sa_email
      max_retries      = 0
      timeout          = "600s"

      containers {
        image   = "us-docker.pkg.dev/cloudrun/container/hello"
        command = ["zitadel"]
        args    = ["db", "migrate", "-c", "/etc/zitadel/config/${var.environment}.toml", "--bootstrap"]

        env {
          name  = "ZITADEL_STORAGE__STATEFUL__MIGRATE"
          value = "auto"
        }

        env {
          name  = "ZITADEL_STORAGE__STATEFUL__BOOTSTRAP"
          value = "auto"
        }

        resources {
          limits = {
            cpu    = "1"
            memory = "512Mi"
          }
        }
      }
    }
  }

  lifecycle {
    ignore_changes = [
      template[0].template[0].containers[0].image,
      template[0].template[0].containers[0].env,
    ]
  }
}
