#!/usr/bin/env bash
set -euo pipefail

# Generate a Caddyfile from domains.json.
# Each domain gets its own site block with on-demand TLS and an X-Instance-Id header.
#
# Usage:
#   ./generate-caddyfile.sh                          # writes config/Caddyfile
#   ./generate-caddyfile.sh --reload                 # writes + reloads Caddy via Kamal
#   ZITADEL_BACKEND=zitadel:8080 ./generate-caddyfile.sh

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DOMAINS_FILE="${SCRIPT_DIR}/domains.json"
OUTPUT_FILE="${SCRIPT_DIR}/config/Caddyfile"
BACKEND="${ZITADEL_BACKEND:-zitadel:8080}"

if [ ! -f "${DOMAINS_FILE}" ]; then
  echo "Error: ${DOMAINS_FILE} not found" >&2
  exit 1
fi

if ! command -v jq &>/dev/null; then
  echo "Error: jq is required. Install with: apt install jq / brew install jq" >&2
  exit 1
fi

cat > "${OUTPUT_FILE}" <<GLOBAL
# Auto-generated from domains.json — do not edit manually.
# Regenerate with: ./generate-caddyfile.sh
{
	on_demand_tls {
		ask http://localhost:5555/check
	}
}

GLOBAL

# Generate a site block for each domain.
jq -r 'to_entries[] | "\(.key) \(.value)"' "${DOMAINS_FILE}" | while read -r domain instance_id; do
  cat >> "${OUTPUT_FILE}" <<SITE

${domain} {
	tls {
		on_demand
	}
	reverse_proxy ${BACKEND} {
		header_up X-Instance-Id ${instance_id}
		header_up X-Forwarded-Proto https
		header_up X-Real-IP {remote_host}
	}
}
SITE
done

# Add a catch-all health check block.
cat >> "${OUTPUT_FILE}" <<'HEALTH'

# Health check endpoint (HTTP only, no TLS).
:80 {
	handle /healthz {
		respond "ok" 200
	}

	# ACME HTTP-01 challenges are handled automatically by Caddy.
	# All other HTTP traffic redirects to HTTPS.
	handle {
		redir https://{host}{uri} permanent
	}
}

# Ask endpoint for on-demand TLS validation.
# Returns 200 if the domain is in our mapping, 404 otherwise.
# Caddy calls this before provisioning a cert for an unknown domain.
:5555 {
	handle /check {
		# This is a placeholder — in the POC, every domain in the Caddyfile
		# already has an explicit site block, so on_demand_tls will match.
		# For production, replace with a DB-backed lookup.
		respond "ok" 200
	}
}
HEALTH

echo "Generated ${OUTPUT_FILE} with $(jq 'length' "${DOMAINS_FILE}") domains."

if [ "${1:-}" = "--reload" ]; then
  echo "Reloading Caddy via Kamal..."
  kamal accessory exec caddy "caddy reload --config /etc/caddy/Caddyfile"
fi
