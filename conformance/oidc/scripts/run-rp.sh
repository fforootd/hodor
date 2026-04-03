#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)"
ARTIFACT_ROOT="${OIDC_CONFORMANCE_ARTIFACTS_DIR:-$ROOT_DIR/artifacts/oidc-conformance}"
ARTIFACT_DIR="${ARTIFACT_ROOT}/rp"

mkdir -p "$ARTIFACT_DIR"

npm run test:rp -w e2e -- --reporter=line

if [[ -d "$ROOT_DIR/e2e/playwright-report" ]]; then
  cp -R "$ROOT_DIR/e2e/playwright-report" "$ARTIFACT_DIR/" || true
fi

if [[ -d "$ROOT_DIR/e2e/test-results" ]]; then
  cp -R "$ROOT_DIR/e2e/test-results" "$ARTIFACT_DIR/" || true
fi
