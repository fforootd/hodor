resource "google_certificate_manager_certificate_map" "default" {
  name        = "zitadel-cert-map-${var.environment}"
  description = "Certificate map for Zitadel ${var.environment}. Entries managed at runtime by infra.rs."
}
