resource "google_dns_managed_zone" "default" {
  name        = var.zone_name
  dns_name    = "${var.base_domain}."
  description = "Zitadel ${var.environment} DNS zone"
  visibility  = "public"
}

resource "google_dns_record_set" "a" {
  managed_zone = google_dns_managed_zone.default.name
  name         = "${var.base_domain}."
  type         = "A"
  ttl          = 300
  rrdatas      = [var.lb_ip_address]
}

resource "google_dns_record_set" "wildcard" {
  managed_zone = google_dns_managed_zone.default.name
  name         = "*.${var.base_domain}."
  type         = "A"
  ttl          = 300
  rrdatas      = [var.lb_ip_address]
}
