data "google_project" "current" {
  project_id = var.project_id
}

locals {
  required_apis = toset([
    "artifactregistry.googleapis.com",
    "certificatemanager.googleapis.com",
    "compute.googleapis.com",
    "dns.googleapis.com",
    "iam.googleapis.com",
    "run.googleapis.com",
    "secretmanager.googleapis.com",
    "spanner.googleapis.com",
  ])

  root_host = "${var.root_subdomain}.${var.platform_domain}"
  demo_host = "${var.demo_subdomain}.${var.platform_domain}"

  container_image = trimspace(var.container_image) != "" ? trimspace(var.container_image) : format(
    "%s-docker.pkg.dev/%s/%s/%s:%s",
    var.region,
    var.project_id,
    google_artifact_registry_repository.images.repository_id,
    var.image_name,
    var.image_tag,
  )

  spanner_database = format(
    "projects/%s/instances/%s/databases/%s",
    var.project_id,
    google_spanner_instance.stateful.name,
    google_spanner_database.stateful.name,
  )

  common_env = merge(
    {
      ZITADEL_PORT                      = "8080"
      ZITADEL_EXTERNAL_DOMAIN           = local.root_host
      ZITADEL_STORAGE_STATEFUL_BACKEND  = "spanner"
      ZITADEL_STORAGE_STATEFUL_DATABASE = local.spanner_database
      ZITADEL_CLOUD_ENABLED             = "true"
      ZITADEL_SEED_FILE                 = "/etc/zitadel/seed.yaml"
      ZITADEL_LOG_FORMAT                = "json"
      ZITADEL_LOG_LEVEL                 = var.log_level
      ZITADEL_ADMIN_EMAIL               = var.admin_email
    },
    trimspace(var.encryption_active_key_id) != "" ? {
      ZITADEL_ENCRYPTION_ACTIVE_KEY_ID = var.encryption_active_key_id
    } : {},
  )

  app_env = merge(local.common_env, {
    ZITADEL_STORAGE_STATEFUL_MIGRATE   = "check"
    ZITADEL_STORAGE_STATEFUL_BOOTSTRAP = "skip"
  })

  job_env = merge(local.common_env, {
    ZITADEL_STORAGE_STATEFUL_MIGRATE   = "auto"
    ZITADEL_STORAGE_STATEFUL_BOOTSTRAP = "auto"
  })

  app_secret_envs = concat(
    [
      {
        name    = "ZITADEL_COOKIE_SECRETS"
        secret  = google_secret_manager_secret.cookie_secrets.secret_id
        version = google_secret_manager_secret_version.cookie_secrets.version
      },
      {
        name    = "ZITADEL_MANAGEMENT_SECRET"
        secret  = google_secret_manager_secret.management_secret.secret_id
        version = google_secret_manager_secret_version.management_secret.version
      },
    ],
    trimspace(var.encryption_keys) != "" ? [
      {
        name    = "ZITADEL_ENCRYPTION_KEYS"
        secret  = google_secret_manager_secret.encryption_keys[0].secret_id
        version = google_secret_manager_secret_version.encryption_keys[0].version
      },
    ] : [],
  )

  job_secret_envs = concat(
    local.app_secret_envs,
    [
      {
        name    = "ZITADEL_ADMIN_PASSWORD"
        secret  = google_secret_manager_secret.admin_password.secret_id
        version = google_secret_manager_secret_version.admin_password.version
      },
      {
        name    = "ZITADEL_ADMIN_PAT"
        secret  = google_secret_manager_secret.admin_pat.secret_id
        version = google_secret_manager_secret_version.admin_pat.version
      },
    ],
  )

  certificate_map_uri = "//certificatemanager.googleapis.com/${google_certificate_manager_certificate_map.platform.id}"
}

resource "google_project_service" "required" {
  for_each = local.required_apis

  project            = var.project_id
  service            = each.value
  disable_on_destroy = false
}

resource "google_artifact_registry_repository" "images" {
  provider = google-beta

  project       = var.project_id
  location      = var.region
  repository_id = var.artifact_registry_repository_id
  description   = "Zitadel Cloud Run images"
  format        = "DOCKER"
  labels        = var.labels

  depends_on = [google_project_service.required]
}

resource "google_artifact_registry_repository_iam_member" "serverless_robot_reader" {
  provider = google-beta

  project    = var.project_id
  location   = var.region
  repository = google_artifact_registry_repository.images.name
  role       = "roles/artifactregistry.reader"
  member     = "serviceAccount:service-${data.google_project.current.number}@serverless-robot-prod.iam.gserviceaccount.com"
}

resource "google_service_account" "runtime" {
  project      = var.project_id
  account_id   = var.runtime_service_account_id
  display_name = "Zitadel Cloud Run runtime"

  depends_on = [google_project_service.required]
}

resource "google_service_account" "migrator" {
  project      = var.project_id
  account_id   = var.migrator_service_account_id
  display_name = "Zitadel Cloud Run migrator"

  depends_on = [google_project_service.required]
}

resource "google_project_iam_member" "runtime_secret_accessor" {
  project = var.project_id
  role    = "roles/secretmanager.secretAccessor"
  member  = "serviceAccount:${google_service_account.runtime.email}"
}

resource "google_project_iam_member" "migrator_secret_accessor" {
  project = var.project_id
  role    = "roles/secretmanager.secretAccessor"
  member  = "serviceAccount:${google_service_account.migrator.email}"
}

resource "google_project_iam_member" "runtime_spanner_user" {
  project = var.project_id
  role    = "roles/spanner.databaseUser"
  member  = "serviceAccount:${google_service_account.runtime.email}"
}

resource "google_project_iam_member" "migrator_spanner_admin" {
  project = var.project_id
  role    = "roles/spanner.databaseAdmin"
  member  = "serviceAccount:${google_service_account.migrator.email}"
}

resource "google_project_iam_member" "runtime_log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.runtime.email}"
}

resource "google_project_iam_member" "migrator_log_writer" {
  project = var.project_id
  role    = "roles/logging.logWriter"
  member  = "serviceAccount:${google_service_account.migrator.email}"
}

resource "google_secret_manager_secret" "cookie_secrets" {
  project   = var.project_id
  secret_id = var.cookie_secret_name

  replication {
    auto {}
  }

  depends_on = [google_project_service.required]
}

resource "google_secret_manager_secret_version" "cookie_secrets" {
  secret      = google_secret_manager_secret.cookie_secrets.id
  secret_data = var.cookie_secrets
}

resource "google_secret_manager_secret" "management_secret" {
  project   = var.project_id
  secret_id = var.management_secret_name

  replication {
    auto {}
  }

  depends_on = [google_project_service.required]
}

resource "google_secret_manager_secret_version" "management_secret" {
  secret      = google_secret_manager_secret.management_secret.id
  secret_data = var.management_secret
}

resource "google_secret_manager_secret" "admin_password" {
  project   = var.project_id
  secret_id = var.admin_password_secret_name

  replication {
    auto {}
  }

  depends_on = [google_project_service.required]
}

resource "google_secret_manager_secret_version" "admin_password" {
  secret      = google_secret_manager_secret.admin_password.id
  secret_data = var.admin_password
}

resource "google_secret_manager_secret" "admin_pat" {
  project   = var.project_id
  secret_id = var.admin_pat_secret_name

  replication {
    auto {}
  }

  depends_on = [google_project_service.required]
}

resource "google_secret_manager_secret_version" "admin_pat" {
  secret      = google_secret_manager_secret.admin_pat.id
  secret_data = var.admin_pat
}

resource "google_secret_manager_secret" "encryption_keys" {
  count = trimspace(var.encryption_keys) != "" ? 1 : 0

  project   = var.project_id
  secret_id = var.encryption_keys_secret_name

  replication {
    auto {}
  }

  depends_on = [google_project_service.required]
}

resource "google_secret_manager_secret_version" "encryption_keys" {
  count = trimspace(var.encryption_keys) != "" ? 1 : 0

  secret      = google_secret_manager_secret.encryption_keys[0].id
  secret_data = var.encryption_keys
}

resource "google_spanner_instance" "stateful" {
  project          = var.project_id
  name             = var.spanner_instance_id
  config           = var.spanner_instance_config
  display_name     = "${var.name_prefix} stateful"
  processing_units = var.spanner_processing_units
  labels           = var.labels

  depends_on = [google_project_service.required]
}

resource "google_spanner_database" "stateful" {
  project  = var.project_id
  instance = google_spanner_instance.stateful.name
  name     = var.spanner_database_name

  depends_on = [google_spanner_instance.stateful]
}

resource "google_cloud_run_v2_service" "app" {
  provider = google-beta

  project              = var.project_id
  name                 = "${var.name_prefix}-app"
  location             = var.region
  ingress              = "INGRESS_TRAFFIC_INTERNAL_LOAD_BALANCER"
  default_uri_disabled = true
  deletion_protection  = false
  labels               = var.labels

  template {
    service_account                  = google_service_account.runtime.email
    timeout                          = var.service_timeout
    max_instance_request_concurrency = var.container_concurrency

    scaling {
      min_instance_count = var.service_min_instances
      max_instance_count = var.service_max_instances
    }

    containers {
      image = local.container_image

      ports {
        container_port = 8080
      }

      resources {
        limits = {
          cpu    = var.service_cpu
          memory = var.service_memory
        }
      }

      dynamic "env" {
        for_each = local.app_env
        content {
          name  = env.key
          value = env.value
        }
      }

      dynamic "env" {
        for_each = local.app_secret_envs
        content {
          name = env.value.name
          value_source {
            secret_key_ref {
              secret  = env.value.secret
              version = env.value.version
            }
          }
        }
      }
    }
  }

  traffic {
    percent = 100
    type    = "TRAFFIC_TARGET_ALLOCATION_TYPE_LATEST"
  }

  depends_on = [
    google_artifact_registry_repository_iam_member.serverless_robot_reader,
    google_project_iam_member.runtime_log_writer,
    google_project_iam_member.runtime_secret_accessor,
    google_project_iam_member.runtime_spanner_user,
  ]
}

resource "google_cloud_run_v2_service_iam_member" "public_invoker" {
  provider = google-beta

  project  = var.project_id
  location = var.region
  name     = google_cloud_run_v2_service.app.name
  role     = "roles/run.invoker"
  member   = "allUsers"
}

resource "google_cloud_run_v2_job" "migrator" {
  provider = google-beta

  project             = var.project_id
  name                = "${var.name_prefix}-db-migrate"
  location            = var.region
  deletion_protection = false
  labels              = var.labels

  template {
    template {
      service_account = google_service_account.migrator.email
      timeout         = var.job_timeout
      max_retries     = 0
      task_count      = 1
      parallelism     = 1

      containers {
        image = local.container_image
        args  = ["db", "migrate", "--bootstrap"]

        resources {
          limits = {
            cpu    = var.job_cpu
            memory = var.job_memory
          }
        }

        dynamic "env" {
          for_each = local.job_env
          content {
            name  = env.key
            value = env.value
          }
        }

        dynamic "env" {
          for_each = local.job_secret_envs
          content {
            name = env.value.name
            value_source {
              secret_key_ref {
                secret  = env.value.secret
                version = env.value.version
              }
            }
          }
        }
      }
    }
  }

  depends_on = [
    google_artifact_registry_repository_iam_member.serverless_robot_reader,
    google_project_iam_member.migrator_log_writer,
    google_project_iam_member.migrator_secret_accessor,
    google_project_iam_member.migrator_spanner_admin,
  ]
}

resource "google_compute_region_network_endpoint_group" "app" {
  provider = google-beta

  project               = var.project_id
  name                  = "${var.name_prefix}-serverless-neg"
  region                = var.region
  network_endpoint_type = "SERVERLESS"

  cloud_run {
    service = google_cloud_run_v2_service.app.name
  }
}

resource "google_compute_backend_service" "app" {
  provider = google-beta

  project               = var.project_id
  name                  = "${var.name_prefix}-backend"
  protocol              = "HTTP"
  load_balancing_scheme = "EXTERNAL_MANAGED"
  timeout_sec           = 30

  backend {
    group = google_compute_region_network_endpoint_group.app.id
  }

  log_config {
    enable      = true
    sample_rate = 1.0
  }
}

resource "google_compute_url_map" "app" {
  project         = var.project_id
  name            = "${var.name_prefix}-url-map"
  default_service = google_compute_backend_service.app.id
}

resource "google_compute_url_map" "http_redirect" {
  project = var.project_id
  name    = "${var.name_prefix}-http-redirect"

  default_url_redirect {
    https_redirect         = true
    redirect_response_code = "MOVED_PERMANENTLY_DEFAULT"
    strip_query            = false
  }
}

resource "google_compute_target_http_proxy" "http" {
  project = var.project_id
  name    = "${var.name_prefix}-http-proxy"
  url_map = google_compute_url_map.http_redirect.id
}

resource "google_dns_managed_zone" "platform" {
  project     = var.project_id
  name        = var.dns_managed_zone_name
  dns_name    = "${var.platform_domain}."
  description = "Managed zone for ${var.platform_domain}"
  labels      = var.labels

  depends_on = [google_project_service.required]
}

resource "google_certificate_manager_dns_authorization" "platform" {
  provider = google-beta

  project = var.project_id
  name    = "${var.name_prefix}-platform"
  domain  = var.platform_domain

  depends_on = [google_project_service.required]
}

resource "google_dns_record_set" "certificate_authorization" {
  project      = var.project_id
  managed_zone = google_dns_managed_zone.platform.name
  name         = google_certificate_manager_dns_authorization.platform.dns_resource_record[0].name
  type         = google_certificate_manager_dns_authorization.platform.dns_resource_record[0].type
  ttl          = 300
  rrdatas      = [google_certificate_manager_dns_authorization.platform.dns_resource_record[0].data]
}

resource "google_certificate_manager_certificate" "wildcard" {
  provider = google-beta

  project = var.project_id
  name    = "${var.name_prefix}-wildcard"

  managed {
    domains            = ["*.${var.platform_domain}"]
    dns_authorizations = [google_certificate_manager_dns_authorization.platform.id]
  }

  depends_on = [google_dns_record_set.certificate_authorization]
}

resource "google_certificate_manager_certificate_map" "platform" {
  provider = google-beta

  project = var.project_id
  name    = "${var.name_prefix}-platform"
}

resource "google_certificate_manager_certificate_map_entry" "wildcard" {
  provider = google-beta

  project      = var.project_id
  name         = "${var.name_prefix}-wildcard"
  map          = google_certificate_manager_certificate_map.platform.name
  hostname     = "*.${var.platform_domain}"
  certificates = [google_certificate_manager_certificate.wildcard.id]
}

resource "google_compute_target_https_proxy" "https" {
  provider = google-beta

  project         = var.project_id
  name            = "${var.name_prefix}-https-proxy"
  url_map         = google_compute_url_map.app.id
  certificate_map = local.certificate_map_uri

  depends_on = [google_certificate_manager_certificate_map_entry.wildcard]
}

resource "google_compute_global_address" "lb" {
  project = var.project_id
  name    = "${var.name_prefix}-lb-ip"
}

resource "google_compute_global_forwarding_rule" "http" {
  project               = var.project_id
  name                  = "${var.name_prefix}-http"
  ip_address            = google_compute_global_address.lb.address
  port_range            = "80"
  target                = google_compute_target_http_proxy.http.id
  load_balancing_scheme = "EXTERNAL_MANAGED"
}

resource "google_compute_global_forwarding_rule" "https" {
  project               = var.project_id
  name                  = "${var.name_prefix}-https"
  ip_address            = google_compute_global_address.lb.address
  port_range            = "443"
  target                = google_compute_target_https_proxy.https.id
  load_balancing_scheme = "EXTERNAL_MANAGED"
}

resource "google_dns_record_set" "platform_wildcard_a" {
  project      = var.project_id
  managed_zone = google_dns_managed_zone.platform.name
  name         = "*.${var.platform_domain}."
  type         = "A"
  ttl          = 300
  rrdatas      = [google_compute_global_address.lb.address]
}
