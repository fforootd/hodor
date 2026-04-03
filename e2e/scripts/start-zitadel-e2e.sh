#!/usr/bin/env sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
DB_PREFIX="$ROOT_DIR/data/zitadel-e2e.db"
CACHE_PREFIX="$ROOT_DIR/data/zitadel-e2e-cache.db"

mkdir -p "$ROOT_DIR/data"
rm -f \
  "$DB_PREFIX" "$DB_PREFIX-shm" "$DB_PREFIX-wal" "$DB_PREFIX-journal" \
  "$CACHE_PREFIX" "$CACHE_PREFIX-shm" "$CACHE_PREFIX-wal" "$CACHE_PREFIX-journal"

# Always build so direct Playwright runs do not use a stale binary.
cargo build -p zitadel 2>&1

exec "$ROOT_DIR/target/debug/zitadel" start -c "$ROOT_DIR/fixtures/zitadel.e2e.toml"
