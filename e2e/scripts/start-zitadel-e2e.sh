#!/usr/bin/env sh
set -eu

ROOT_DIR="$(CDPATH= cd -- "$(dirname -- "$0")/../.." && pwd)"
DB_PREFIX="$ROOT_DIR/data/zitadel-e2e.db"
CACHE_PREFIX="$ROOT_DIR/data/zitadel-e2e-cache.db"

mkdir -p "$ROOT_DIR/data"
rm -f \
  "$DB_PREFIX" "$DB_PREFIX-shm" "$DB_PREFIX-wal" "$DB_PREFIX-journal" \
  "$CACHE_PREFIX" "$CACHE_PREFIX-shm" "$CACHE_PREFIX-wal" "$CACHE_PREFIX-journal"

exec go run ./cmd/zitadel start -c fixtures/zitadel.e2e.toml
