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
REGION="${REGION:?set REGION}"
PLATFORM_DOMAIN="${PLATFORM_DOMAIN:?set PLATFORM_DOMAIN}"
SPANNER_INSTANCE_ID="${SPANNER_INSTANCE_ID:?set SPANNER_INSTANCE_ID}"
SPANNER_DATABASE_NAME="${SPANNER_DATABASE_NAME:?set SPANNER_DATABASE_NAME}"
MIGRATOR_JOB_NAME="${MIGRATOR_JOB_NAME:?set MIGRATOR_JOB_NAME}"
ADMIN_PAT_SECRET_NAME="${ADMIN_PAT_SECRET_NAME:?set ADMIN_PAT_SECRET_NAME}"
ROOT_SUBDOMAIN="${ROOT_SUBDOMAIN:-root}"
DEMO_SUBDOMAIN="${DEMO_SUBDOMAIN:-demo}"
ROOT_HOST="${ROOT_SUBDOMAIN}.${PLATFORM_DOMAIN}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Executing Cloud Run migration job ${MIGRATOR_JOB_NAME}"
gcloud run jobs execute "${MIGRATOR_JOB_NAME}" \
  --project="${PROJECT_ID}" \
  --region="${REGION}" \
  --wait

echo "Ensuring ${ROOT_HOST} points at the default instance in Spanner"
root_lookup_sql="SELECT domain FROM domains WHERE domain = '${ROOT_HOST}' LIMIT 1"
root_lookup_json="$(
  gcloud spanner databases execute-sql "${SPANNER_DATABASE_NAME}" \
    --project="${PROJECT_ID}" \
    --instance="${SPANNER_INSTANCE_ID}" \
    --sql="${root_lookup_sql}" \
    --format=json
)"

if jq -e 'length > 0' >/dev/null <<<"${root_lookup_json}"; then
  echo "Root domain mapping already exists."
else
  insert_sql="INSERT INTO domains (domain, instance_id, is_primary, state, verified) VALUES ('${ROOT_HOST}', 'default', TRUE, 'active', TRUE)"
  gcloud spanner databases execute-sql "${SPANNER_DATABASE_NAME}" \
    --project="${PROJECT_ID}" \
    --instance="${SPANNER_INSTANCE_ID}" \
    --sql="${insert_sql}" \
    >/dev/null
  echo "Inserted root domain mapping."
fi

echo "Waiting for https://${ROOT_HOST}/.well-known/openid-configuration to report the root issuer"
for attempt in $(seq 1 60); do
  if discovery_json="$(curl -fsS "https://${ROOT_HOST}/.well-known/openid-configuration" 2>/dev/null)"; then
    if jq -e --arg issuer "https://${ROOT_HOST}" '.issuer == $issuer' >/dev/null <<<"${discovery_json}"; then
      echo "Root instance is reachable."
      break
    fi
  fi

  if [[ "${attempt}" == "60" ]]; then
    echo "Timed out waiting for the root instance to become reachable." >&2
    exit 1
  fi

  sleep 10
done

PROJECT_ID="${PROJECT_ID}" \
PLATFORM_DOMAIN="${PLATFORM_DOMAIN}" \
ADMIN_PAT_SECRET_NAME="${ADMIN_PAT_SECRET_NAME}" \
ROOT_SUBDOMAIN="${ROOT_SUBDOMAIN}" \
DEMO_SUBDOMAIN="${DEMO_SUBDOMAIN}" \
"${script_dir}/create-demo-instance.sh"

echo "Bootstrap complete."
