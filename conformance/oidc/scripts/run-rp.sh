#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)"
ARTIFACT_ROOT="${OIDC_JOURNEYS_ARTIFACTS_DIR:-${OIDC_CONFORMANCE_ARTIFACTS_DIR:-$ROOT_DIR/artifacts/journeys/oidc}}"
ARTIFACT_DIR="${ARTIFACT_ROOT}/rp"

mkdir -p "$ARTIFACT_DIR"
echo "warning: conformance/oidc/scripts/run-rp.sh is deprecated; use journeys-oidc-rp instead" >&2

copy_reports() {
  {
    echo "results_json=rp/results.json"
    echo "run_log=rp/run.log"
    echo "html_report=rp/playwright-report/index.html"
  } >"$ARTIFACT_DIR/metadata.txt"

  rm -rf "$ARTIFACT_DIR/playwright-report" "$ARTIFACT_DIR/test-results"

  if [[ -d "$ROOT_DIR/browser-tests/playwright-report" ]]; then
    cp -R "$ROOT_DIR/browser-tests/playwright-report" "$ARTIFACT_DIR/" || true
  fi

  if [[ -d "$ROOT_DIR/browser-tests/test-results" ]]; then
    cp -R "$ROOT_DIR/browser-tests/test-results" "$ARTIFACT_DIR/" || true
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
  npm run test:journeys:oidc:rp -w browser-tests -- --reporter="$reporters" 2>&1 | tee "$ARTIFACT_DIR/run.log"
