#!/usr/bin/env bash
set -euo pipefail

require() {
  command -v "$1" >/dev/null 2>&1 || {
    echo "missing required command: $1" >&2
    exit 1
  }
}

require curl
require gcloud
require jq

PROJECT_ID="${PROJECT_ID:?set PROJECT_ID}"
PLATFORM_DOMAIN="${PLATFORM_DOMAIN:?set PLATFORM_DOMAIN}"
ADMIN_PAT_SECRET_NAME="${ADMIN_PAT_SECRET_NAME:?set ADMIN_PAT_SECRET_NAME}"
ROOT_SUBDOMAIN="${ROOT_SUBDOMAIN:-root}"
DEMO_SUBDOMAIN="${DEMO_SUBDOMAIN:-demo}"
ROOT_HOST="${ROOT_SUBDOMAIN}.${PLATFORM_DOMAIN}"
DEMO_HOST="${DEMO_SUBDOMAIN}.${PLATFORM_DOMAIN}"

if [[ -n "${ADMIN_PAT:-}" ]]; then
  admin_pat="${ADMIN_PAT}"
else
  admin_pat="$(gcloud secrets versions access latest \
    --project="${PROJECT_ID}" \
    --secret="${ADMIN_PAT_SECRET_NAME}")"
fi

echo "Checking for an existing demo instance on https://${DEMO_HOST}"
instances_json="$(
  curl -fsS \
    --retry 5 \
    --retry-delay 2 \
    -H "Authorization: Bearer ${admin_pat}" \
    "https://${ROOT_HOST}/v1/instances"
)"

if jq -e --arg demo_host "${DEMO_HOST}" '.items[]? | select(.primary_domain == $demo_host)' \
  >/dev/null <<<"${instances_json}"; then
  echo "Demo instance already exists."
  exit 0
fi

payload="$(jq -nc --arg domain "${DEMO_HOST}" '{domain: $domain}')"
create_json="$(
  curl -fsS \
    --retry 5 \
    --retry-delay 2 \
    -X POST \
    -H "Authorization: Bearer ${admin_pat}" \
    -H "Content-Type: application/json" \
    -d "${payload}" \
    "https://${ROOT_HOST}/v1/instances"
)"

echo "Created demo instance:"
jq '{instance_id, primary_domain, kind, placement_mode, state}' <<<"${create_json}"
