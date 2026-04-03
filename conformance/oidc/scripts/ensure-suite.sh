#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../../.." && pwd)"
CACHE_DIR="${OIDC_CONFORMANCE_CACHE_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/hodor/oidc-conformance}"
SUITE_REF="${OIDC_CONFORMANCE_SUITE_REF:-release-v5.1.40}"
SUITE_DIR="${CACHE_DIR}/conformance-suite-${SUITE_REF}"
SUITE_URL="${OIDC_CONFORMANCE_SUITE_URL:-https://gitlab.com/openid/conformance-suite}"

mkdir -p "$CACHE_DIR"

if [[ ! -d "$SUITE_DIR/.git" ]]; then
  git clone --depth 1 --branch "$SUITE_REF" "$SUITE_URL" "$SUITE_DIR"
fi

printf '%s\n' "$SUITE_DIR"
