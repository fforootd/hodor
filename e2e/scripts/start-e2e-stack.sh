#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
BASE_URL="${BASE_URL:-http://127.0.0.1:18080}"
MOCK_URL="${MOCK_OIDC_URL:-http://127.0.0.1:19998}"

wait_for_url() {
  local target="$1"
  local label="$2"

  for _ in $(seq 1 240); do
    if curl --silent --fail "$target" >/dev/null 2>&1; then
      echo "[e2e] ${label} ready: ${target}"
      return 0
    fi
    sleep 0.5
  done

  echo "[e2e] ${label} failed to become ready: ${target}" >&2
  return 1
}

cleanup() {
  local exit_code=$?
  if [[ -n "${MOCK_PID:-}" ]] && kill -0 "$MOCK_PID" >/dev/null 2>&1; then
    kill "$MOCK_PID" >/dev/null 2>&1 || true
  fi
  if [[ -n "${ZITADEL_PID:-}" ]] && kill -0 "$ZITADEL_PID" >/dev/null 2>&1; then
    kill "$ZITADEL_PID" >/dev/null 2>&1 || true
  fi
  wait "${MOCK_PID:-}" 2>/dev/null || true
  wait "${ZITADEL_PID:-}" 2>/dev/null || true
  exit "$exit_code"
}

trap cleanup EXIT INT TERM

"$ROOT_DIR/e2e/scripts/start-mock-oidc.sh" &
MOCK_PID=$!

"$ROOT_DIR/e2e/scripts/start-zitadel-e2e.sh" &
ZITADEL_PID=$!

wait_for_url "${MOCK_URL}/healthz" "mock oidc"
wait_for_url "${BASE_URL}/healthz" "zitadel"

echo "[e2e] stack ready"

while kill -0 "$MOCK_PID" >/dev/null 2>&1 && kill -0 "$ZITADEL_PID" >/dev/null 2>&1; do
  sleep 1
done

if ! kill -0 "$MOCK_PID" >/dev/null 2>&1; then
  echo "[e2e] mock oidc exited unexpectedly" >&2
fi
if ! kill -0 "$ZITADEL_PID" >/dev/null 2>&1; then
  echo "[e2e] zitadel exited unexpectedly" >&2
fi

exit 1
