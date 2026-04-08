resource "google_compute_global_address" "default" {
  name = "zitadel-ip-${var.environment}"
}

resource "google_compute_region_network_endpoint_group" "serverless" {
  name                  = "zitadel-neg-${var.environment}"
  region                = var.region
  network_endpoint_type = "SERVERLESS"

  cloud_run {
    service = var.cloud_run_service_name
  }
}

resource "google_compute_backend_service" "default" {
  name        = "zitadel-backend-${var.environment}"
  protocol    = "HTTPS"
  port_name   = "http"
  timeout_sec = 30

  enable_cdn = var.cdn_enabled

  dynamic "cdn_policy" {
    for_each = var.cdn_enabled ? [1] : []
    content {
      cache_mode                   = "CACHE_ALL_STATIC"
      default_ttl                  = 3600
      signed_url_cache_max_age_sec = 0
    }
  }

  backend {
    group = google_compute_region_network_endpoint_group.serverless.id
  }

  log_config {
    enable      = true
    sample_rate = 0.1
  }
}

resource "google_compute_url_map" "default" {
  name            = "zitadel-lb-${var.environment}"
  default_service = google_compute_backend_service.default.id
}

resource "google_compute_target_https_proxy" "default" {
  name             = "zitadel-https-${var.environment}"
  url_map          = google_compute_url_map.default.id
  certificate_map  = "//certificatemanager.googleapis.com/${var.certificate_map_id}"
}

resource "google_compute_global_forwarding_rule" "https" {
  name                  = "zitadel-https-fwd-${var.environment}"
  target                = google_compute_target_https_proxy.default.id
  ip_address            = google_compute_global_address.default.address
  port_range            = "443"
  load_balancing_scheme = "EXTERNAL_MANAGED"
}

resource "google_compute_url_map" "http_redirect" {
  name = "zitadel-http-redirect-${var.environment}"

  default_url_redirect {
    https_redirect = true
    strip_query    = false
  }
}

resource "google_compute_target_http_proxy" "redirect" {
  name    = "zitadel-http-redirect-${var.environment}"
  url_map = google_compute_url_map.http_redirect.id
}

resource "google_compute_global_forwarding_rule" "http_redirect" {
  name                  = "zitadel-http-fwd-${var.environment}"
  target                = google_compute_target_http_proxy.redirect.id
  ip_address            = google_compute_global_address.default.address
  port_range            = "80"
  load_balancing_scheme = "EXTERNAL_MANAGED"
}
