#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)"
PROJECT_NAME="${OIDC_CONFORMANCE_PROJECT:-oidc-conformance}"
CACHE_DIR="${OIDC_CONFORMANCE_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/hodor/oidc-conformance}"
SUITE_REF="${OIDC_CONFORMANCE_SUITE_REF:-release-v5.1.40}"
SUITE_DIR="${CACHE_DIR}/conformance-suite-${SUITE_REF}"

if [[ -f "$SUITE_DIR/docker-compose-localtest.yml" ]]; then
  docker compose -p "$PROJECT_NAME" -f "$SUITE_DIR/docker-compose-localtest.yml" down -v --remove-orphans || true
fi

docker compose -p "$PROJECT_NAME" -f "$ROOT_DIR/conformance/oidc/docker-compose.zitadel.yml" down -v --remove-orphans || true
