#!/usr/bin/env sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
DB_PREFIX="$ROOT_DIR/data/zitadel-e2e.db"
CACHE_PREFIX="$ROOT_DIR/data/zitadel-e2e-cache.db"
ZITADEL_BIN="${ZITADEL_BROWSER_TESTS_BINARY:-${ZITADEL_E2E_BINARY:-$ROOT_DIR/target/debug/zitadel}}"

mkdir -p "$ROOT_DIR/data"
rm -f \
  "$DB_PREFIX" "$DB_PREFIX-shm" "$DB_PREFIX-wal" "$DB_PREFIX-journal" \
  "$CACHE_PREFIX" "$CACHE_PREFIX-shm" "$CACHE_PREFIX-wal" "$CACHE_PREFIX-journal"

if [ -n "${ZITADEL_BROWSER_TESTS_BINARY:-${ZITADEL_E2E_BINARY:-}}" ]; then
  if [ ! -x "$ZITADEL_BIN" ]; then
    echo "[browser-tests] prepared Zitadel binary is not executable: $ZITADEL_BIN" >&2
    exit 1
  fi
else
  # Direct browser-test runs need fresh embedded web assets and a fresh binary.
  npm run build -w web >/dev/null
  cargo build -p zitadel 2>&1
fi

SEED_ARG=""
if [ -n "${ZITADEL_E2E_SEED:-}" ]; then
  SEED_ARG="--seed $ZITADEL_E2E_SEED"
fi

exec "$ZITADEL_BIN" start -c "$ROOT_DIR/fixtures/zitadel.e2e.toml" $SEED_ARG
