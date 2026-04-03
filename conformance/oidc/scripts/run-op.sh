#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)"
CACHE_DIR="${OIDC_CONFORMANCE_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/hodor/oidc-conformance}"
SUITE_DIR="$("$ROOT_DIR/conformance/oidc/scripts/ensure-suite.sh")"
SUITE_COMPOSE="${SUITE_DIR}/docker-compose-localtest.yml"
ZITADEL_COMPOSE="${ROOT_DIR}/conformance/oidc/docker-compose.zitadel.yml"
PLAN_FILE="${ROOT_DIR}/conformance/oidc/plans/op.txt"
CONFIG_FILE="${ROOT_DIR}/conformance/oidc/config/op-static-client.json"
SUITE_NGINX_TEMPLATE="${ROOT_DIR}/conformance/oidc/suite-nginx.conf"
PROJECT_NAME="${OIDC_CONFORMANCE_PROJECT:-oidc-conformance}"
MAVEN_CACHE="${OIDC_CONFORMANCE_MAVEN_CACHE:-$CACHE_DIR/m2}"
ARTIFACT_ROOT="${OIDC_CONFORMANCE_ARTIFACTS_DIR:-$ROOT_DIR/artifacts/oidc-conformance}"
ARTIFACT_DIR="${ARTIFACT_ROOT}/op"
KEEP_STACK="${OIDC_CONFORMANCE_KEEP_STACK:-0}"

mkdir -p "$MAVEN_CACHE" "$ARTIFACT_DIR"
cp "$SUITE_NGINX_TEMPLATE" "$SUITE_DIR/nginx/nginx.conf"

case "$ARTIFACT_DIR" in
  "$ROOT_DIR"/*) ;;
  *)
    echo "OIDC_CONFORMANCE_ARTIFACTS_DIR must live under $ROOT_DIR" >&2
    exit 1
    ;;
esac

CONTAINER_ARTIFACT_DIR="/work/repo/${ARTIFACT_DIR#$ROOT_DIR/}"
CONTAINER_CONFIG_FILE="/work/repo/${CONFIG_FILE#$ROOT_DIR/}"

wait_for_url() {
  local target="$1"
  local label="$2"
  local insecure="${3:-0}"
  local curl_args=(--silent --fail)

  if [[ "$insecure" == "1" ]]; then
    curl_args+=(--insecure)
  fi

  for _ in $(seq 1 240); do
    if curl "${curl_args[@]}" "$target" >/dev/null 2>&1; then
      echo "[oidc-conformance] ${label} ready: ${target}"
      return 0
    fi
    sleep 1
  done

  echo "[oidc-conformance] ${label} failed to become ready: ${target}" >&2
  return 1
}

save_logs() {
  {
    echo "suite_ref=${OIDC_CONFORMANCE_SUITE_REF:-release-v5.1.40}"
    echo "project=${PROJECT_NAME}"
    echo "plan_file=${PLAN_FILE#$ROOT_DIR/}"
    echo "config_file=${CONFIG_FILE#$ROOT_DIR/}"
  } >"$ARTIFACT_DIR/metadata.txt"

  docker compose -p "$PROJECT_NAME" -f "$SUITE_COMPOSE" logs --no-color \
    >"$ARTIFACT_DIR/suite.log" 2>&1 || true
  docker compose -p "$PROJECT_NAME" -f "$ZITADEL_COMPOSE" logs --no-color \
    >"$ARTIFACT_DIR/zitadel.log" 2>&1 || true
}

cleanup() {
  local exit_code=$?
  save_logs
  if [[ "$KEEP_STACK" != "1" ]]; then
    docker compose -p "$PROJECT_NAME" -f "$SUITE_COMPOSE" down -v --remove-orphans || true
    docker compose -p "$PROJECT_NAME" -f "$ZITADEL_COMPOSE" down -v --remove-orphans || true
  fi
  exit "$exit_code"
}

trap cleanup EXIT INT TERM

(
  cd "$SUITE_DIR"
  MAVEN_CACHE="$MAVEN_CACHE" docker compose -f builder-compose.yml run --rm builder
)

docker compose -p "$PROJECT_NAME" -f "$SUITE_COMPOSE" build nginx >/dev/null
docker compose -p "$PROJECT_NAME" -f "$SUITE_COMPOSE" up -d mongodb oidcc-provider server nginx
docker compose -p "$PROJECT_NAME" -f "$ZITADEL_COMPOSE" up -d --build zitadel

wait_for_url "http://127.0.0.1:18081/healthz" "zitadel"
wait_for_url "https://127.0.0.1:8443/api/runner/available" "conformance suite" 1

mapfile -t plan_lines < <(grep -Ev '^\s*(#|$)' "$PLAN_FILE")
if [[ "${#plan_lines[@]}" -eq 0 ]]; then
  echo "No OIDC provider plans configured in $PLAN_FILE" >&2
  exit 1
fi

args=(scripts/run-test-plan.py --no-parallel --export-dir "$CONTAINER_ARTIFACT_DIR")
for plan in "${plan_lines[@]}"; do
  args+=("$plan" "$CONTAINER_CONFIG_FILE")
done

docker compose -p "$PROJECT_NAME" -f "$SUITE_COMPOSE" run --rm \
  --entrypoint python3 \
  --volume "$ROOT_DIR:/work/repo" \
  test \
  "${args[@]}"
