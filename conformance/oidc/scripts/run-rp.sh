#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)"
ARTIFACT_ROOT="${OIDC_CONFORMANCE_ARTIFACTS_DIR:-$ROOT_DIR/artifacts/oidc-conformance}"
ARTIFACT_DIR="${ARTIFACT_ROOT}/rp"

mkdir -p "$ARTIFACT_DIR"

copy_reports() {
  {
    echo "results_json=rp/results.json"
    echo "run_log=rp/run.log"
    echo "html_report=rp/playwright-report/index.html"
  } >"$ARTIFACT_DIR/metadata.txt"

  rm -rf "$ARTIFACT_DIR/playwright-report" "$ARTIFACT_DIR/test-results"

  if [[ -d "$ROOT_DIR/e2e/playwright-report" ]]; then
    cp -R "$ROOT_DIR/e2e/playwright-report" "$ARTIFACT_DIR/" || true
  fi

  if [[ -d "$ROOT_DIR/e2e/test-results" ]]; then
    cp -R "$ROOT_DIR/e2e/test-results" "$ARTIFACT_DIR/" || true
  fi
}

cleanup() {
  local exit_code=$?
  copy_reports
  exit "$exit_code"
}

trap cleanup EXIT INT TERM

reporters="line,html,json"
if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  reporters="github,${reporters}"
fi

PLAYWRIGHT_JSON_OUTPUT_FILE="$ARTIFACT_DIR/results.json" \
  npm run test:rp -w e2e -- --reporter="$reporters" 2>&1 | tee "$ARTIFACT_DIR/run.log"
