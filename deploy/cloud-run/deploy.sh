#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

PROJECT_ID="${PROJECT_ID:?set PROJECT_ID}"
SERVICE_NAME="${SERVICE_NAME:-zitadel-test}"
REGION="${REGION:-us-west1}"
SERVICE_ACCOUNT_EMAIL="${SERVICE_ACCOUNT_EMAIL:?set SERVICE_ACCOUNT_EMAIL}"
SOURCE_CONFIG="${SOURCE_CONFIG:-${REPO_ROOT}/fixtures/zitadel.spanner.local.toml}"
ENV_FILE="${ENV_FILE:-${SCRIPT_DIR}/runtime.env.yaml}"
SOURCE_DIR="${SOURCE_DIR:-${REPO_ROOT}}"
IMAGE="${IMAGE:-}"  # Pre-built image reference. When set, skips --source build.
PORT="${PORT:-8080}"
CPU="${CPU:-1}"
MEMORY="${MEMORY:-1Gi}"
MIN_INSTANCES="${MIN_INSTANCES:-1}"
ALLOW_UNAUTHENTICATED="${ALLOW_UNAUTHENTICATED:-true}"

toml_string_value() {
  local file="$1"
  local section="$2"
  local key="$3"
  awk -v section="[${section}]" -v key="${key}" '
    $0 ~ /^\[/ {
      in_section = ($0 == section)
    }
    in_section && $0 ~ "^[[:space:]]*" key "[[:space:]]*=" {
      sub(/^[^=]*=[[:space:]]*/, "", $0)
      gsub(/^[[:space:]]*"/, "", $0)
      gsub(/"[[:space:]]*$/, "", $0)
      print
      exit
    }
  ' "${file}"
}

toml_array_csv() {
  local file="$1"
  local section="$2"
  local key="$3"
  awk -v section="[${section}]" -v key="${key}" '
    $0 ~ /^\[/ {
      in_section = ($0 == section)
    }
    in_section && $0 ~ "^[[:space:]]*" key "[[:space:]]*=" {
      sub(/^[^=]*=[[:space:]]*\[/, "", $0)
      sub(/\][[:space:]]*$/, "", $0)
      gsub(/"/, "", $0)
      gsub(/[[:space:]]+/, "", $0)
      print
      exit
    }
  ' "${file}"
}

generate_env_file() {
  local source_config="$1"
  local env_file="$2"
  local config_port
  local cookie_secrets
  local backend
  local database
  local cloud_enabled

  config_port="$(toml_string_value "${source_config}" "server" "port")"
  cookie_secrets="$(toml_array_csv "${source_config}" "server" "cookie_secrets")"
  backend="$(toml_string_value "${source_config}" "storage.stateful" "backend")"
  database="$(toml_string_value "${source_config}" "storage.stateful" "database")"
  cloud_enabled="$(toml_string_value "${source_config}" "cloud" "enabled")"

  if [[ -z "${cookie_secrets}" || -z "${backend}" || -z "${database}" ]]; then
    echo "could not derive Cloud Run runtime env from ${source_config}" >&2
    echo "expected [server].cookie_secrets and [storage.stateful].{backend,database}" >&2
    exit 1
  fi

  cat >"${env_file}" <<EOF
ZITADEL_SERVER__PORT: "${PORT:-${config_port:-8080}}"
ZITADEL_COOKIE_SECRETS: "${cookie_secrets}"

ZITADEL_STORAGE__STATEFUL__BACKEND: "${backend}"
ZITADEL_STORAGE__STATEFUL__DATABASE: "${database}"
ZITADEL_STORAGE__STATEFUL__MIGRATE: "check"
ZITADEL_STORAGE__STATEFUL__BOOTSTRAP: "skip"

ZITADEL_OBSERVABILITY__CACHE_PATH: "/tmp/zitadel-cache.db"
EOF

  if [[ "${cloud_enabled}" == "true" ]]; then
    cat >>"${env_file}" <<'EOF'

ZITADEL_CLOUD__ENABLED: "true"
EOF
  fi
}

if [[ ! -f "${ENV_FILE}" ]]; then
  if [[ ! -f "${SOURCE_CONFIG}" ]]; then
    echo "missing env file: ${ENV_FILE}" >&2
    echo "missing source config: ${SOURCE_CONFIG}" >&2
    echo "either create ${ENV_FILE} manually or point SOURCE_CONFIG at a fixture TOML" >&2
    exit 1
  fi
  generate_env_file "${SOURCE_CONFIG}" "${ENV_FILE}"
  echo "generated ${ENV_FILE} from ${SOURCE_CONFIG}" >&2
fi

if [[ ! -f "${ENV_FILE}" ]]; then
  echo "missing env file after generation attempt: ${ENV_FILE}" >&2
  exit 1
fi

deploy_cmd=(
  gcloud run deploy "${SERVICE_NAME}"
  --project "${PROJECT_ID}"
  --region "${REGION}"
  --service-account "${SERVICE_ACCOUNT_EMAIL}"
  --port "${PORT}"
  --cpu "${CPU}"
  --memory "${MEMORY}"
  --min-instances "${MIN_INSTANCES}"
  --env-vars-file "${ENV_FILE}"
)

if [[ -n "${IMAGE}" ]]; then
  deploy_cmd+=(--image "${IMAGE}")
else
  deploy_cmd+=(--source "${SOURCE_DIR}")
fi

if [[ "${ALLOW_UNAUTHENTICATED}" == "true" ]]; then
  deploy_cmd+=(--allow-unauthenticated)
else
  deploy_cmd+=(--no-allow-unauthenticated)
fi

"${deploy_cmd[@]}"

SERVICE_URL="$(
  gcloud run services describe "${SERVICE_NAME}" \
    --project "${PROJECT_ID}" \
    --region "${REGION}" \
    --format='value(status.url)'
)"
SERVICE_HOST="${SERVICE_URL#https://}"

gcloud run services update "${SERVICE_NAME}" \
  --project "${PROJECT_ID}" \
  --region "${REGION}" \
  --update-env-vars "ZITADEL_SERVER__PUBLIC_ORIGIN=${SERVICE_URL},ZITADEL_SERVER__EXTERNAL_DOMAIN=${SERVICE_HOST}"

printf 'Deployed %s\n' "${SERVICE_NAME}"
printf 'URL: %s\n' "${SERVICE_URL}"
printf 'Origin vars updated: PUBLIC_ORIGIN=%s EXTERNAL_DOMAIN=%s\n' "${SERVICE_URL}" "${SERVICE_HOST}"
