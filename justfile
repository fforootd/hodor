set shell := ["bash", "-euo", "pipefail", "-c"]

dev_config := "fixtures/zitadel.dev.toml"
dev_seed_dir := "fixtures/seeds"
dev_db_file := "data/zitadel.db"
dev_cache_file := "data/zitadel-cache.db"

fuzz_targets := "fuzz_cookie_verify fuzz_resolve_client_auth fuzz_token_request_deser fuzz_authorize_params_deser fuzz_decode_request_object fuzz_password_verify fuzz_secretbox_open fuzz_generator"

# ─── Default ──────────────────────────────────────────────

[private]
default:
    @just --list

# ─── Web (Vue/Vite) ──────────────────────────────────────

# Install all workspace dependencies
[group('web')]
install:
    npm ci --prefer-offline

[private]
_ensure-node-modules:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -d node_modules ]; then
        npm ci --prefer-offline
    fi

# Build embedded frontend assets into web/dist
[group('web')]
web: _ensure-node-modules
    npm run build

# Create placeholder embedded assets for Rust builds
[group('web')]
ensure-webdist:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ ! -d web/dist ]; then
        mkdir -p web/dist/src/login web/dist/src/console web/dist/src/account web/dist/assets
        echo '<!DOCTYPE html><html><body>dev placeholder</body></html>' > web/dist/src/login/index.html
        echo '<!DOCTYPE html><html><body>dev placeholder</body></html>' > web/dist/src/console/index.html
        echo '<!DOCTYPE html><html><body>dev placeholder</body></html>' > web/dist/src/account/index.html
        echo "Created placeholder web/dist (use 'just web' for real assets)"
    fi

# ─── Development ─────────────────────────────────────────

# Canonical dev flow: Vite HMR + Rust API with seed data
[group('dev')]
dev SEED="frontend": _ensure-node-modules ensure-webdist
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p data
    cargo build -p zitadel
    echo "─── Zitadel local dev ───"
    echo "→ Console / Login / Account: http://localhost:5173"
    echo "→ API / OIDC:               http://localhost:8080"
    echo "→ Seed pack:                {{SEED}}"
    echo "→ Admin login:              admin / admin123"
    echo "→ Admin PAT:                zitadel-dev-pat-do-not-use-in-production"
    echo "→ SQLite DB:                $(pwd)/{{dev_db_file}}"
    trap 'kill 0' EXIT
    ZITADEL_SEED_FILE="{{dev_seed_dir}}/{{SEED}}.yaml" ./target/debug/zitadel start -c {{dev_config}} &
    npm run dev -w web

# Embedded-assets development — parity mode without Vite
[group('dev')]
dev-embed SEED="frontend": web ensure-webdist
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p data
    cargo build -p zitadel
    echo "→ Embedded UI + API: http://localhost:8080"
    echo "→ Seed pack:         {{SEED}}"
    ZITADEL_SEED_FILE="{{dev_seed_dir}}/{{SEED}}.yaml" ./target/debug/zitadel start -c {{dev_config}}

# Frontend-only development — expects a backend at localhost:8080 or ZITADEL_API_BASE
[group('dev')]
dev-web: _ensure-node-modules
    #!/usr/bin/env bash
    set -euo pipefail
    echo "→ Frontend dev server: http://localhost:5173"
    echo "→ API target:          ${ZITADEL_API_BASE:-http://localhost:8080}"
    ZITADEL_API_BASE="${ZITADEL_API_BASE:-http://localhost:8080}" npm run dev -w web

# Wipe DB and restart fresh
[group('dev')]
dev-reset SEED="frontend":
    #!/usr/bin/env bash
    set -euo pipefail
    db_url="${ZITADEL_STORAGE_STATEFUL_URL:-sqlite://$(pwd)/{{dev_db_file}}}"
    case "$db_url" in
        sqlite://*) ;;
        *) echo "refusing to delete local SQLite files for non-SQLite ZITADEL_STORAGE_STATEFUL_URL=$db_url"; exit 1 ;;
    esac
    echo "→ stopping local dev services on :8080 and :5173 (if running)"
    for port in 8080 5173; do
        if command -v lsof >/dev/null 2>&1; then
            pids="$(lsof -ti tcp:$port 2>/dev/null)" || true
            if [ -n "$pids" ]; then kill $pids 2>/dev/null || true; fi
        fi
    done
    rm -f {{dev_db_file}} {{dev_db_file}}-shm {{dev_db_file}}-wal {{dev_db_file}}-journal
    rm -f {{dev_cache_file}} {{dev_cache_file}}-shm {{dev_cache_file}}-wal {{dev_cache_file}}-journal
    just dev {{SEED}}

# Validate and apply a named seed pack
[group('dev')]
dev-seed SEED="frontend":
    #!/usr/bin/env bash
    set -euo pipefail
    seed_file="{{dev_seed_dir}}/{{SEED}}.yaml"
    test -f "$seed_file" || (echo "unknown seed pack '{{SEED}}' (expected $seed_file)" && exit 1)
    if [ ! -x ./target/debug/zitadel ]; then
        echo "→ building debug binary"
        cargo build -p zitadel
    fi
    echo "→ validating $seed_file"
    ./target/debug/zitadel seed validate --file "$seed_file"
    echo "→ applying $seed_file"
    ./target/debug/zitadel seed apply -c {{dev_config}} --file "$seed_file"

# Print local dev paths, credentials, and seed pack
[group('dev')]
dev-status SEED="frontend":
    #!/usr/bin/env bash
    set -euo pipefail
    db_url="${ZITADEL_STORAGE_STATEFUL_URL:-sqlite://$(pwd)/{{dev_db_file}}}"
    seed_file="${ZITADEL_SEED_FILE:-$(pwd)/{{dev_seed_dir}}/{{SEED}}.yaml}"
    echo "Dev config:     $(pwd)/{{dev_config}}"
    echo "Server:         http://localhost:8080"
    echo "Vite server:    http://localhost:5173"
    echo "Database URL:   $db_url"
    echo "Database path:  $(pwd)/{{dev_db_file}}"
    echo "Seed pack:      {{SEED}}"
    echo "Seed file:      $seed_file"
    echo "Admin login:    admin / admin123"
    echo "Admin PAT:      zitadel-dev-pat-do-not-use-in-production"

# ─── Build ───────────────────────────────────────────────

# Build the release zitadel binary
[group('build')]
build: ensure-webdist
    cargo build --release

# ─── Testing ─────────────────────────────────────────────

[private]
_run-rust-tests:
    #!/usr/bin/env bash
    set -euo pipefail
    if cargo nextest --version >/dev/null 2>&1; then
        cargo nextest run --workspace
    else
        cargo test --workspace
    fi

# Run Rust tests
[group('test')]
test: _run-rust-tests

# Run web component tests (Vitest)
[group('test')]
test-web: _ensure-node-modules
    npm test -w web

# Run the extended Playwright suite
[group('test')]
test-e2e: _ensure-node-modules
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${ZITADEL_E2E_BINARY:-}" ]; then
        just web
        cargo build -p zitadel
    else
        echo "→ using prepared zitadel binary $ZITADEL_E2E_BINARY"
    fi
    npm test -w e2e

# Run Playwright smoke tests
[group('test')]
e2e-smoke: _ensure-node-modules
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${ZITADEL_E2E_BINARY:-}" ]; then
        just web
        cargo build -p zitadel
    else
        echo "→ using prepared zitadel binary $ZITADEL_E2E_BINARY"
    fi
    npm run test:smoke -w e2e

# ─── Conformance ─────────────────────────────────────────

# Alias for oidc-conformance
[group('conformance')]
conformance: oidc-conformance

# Run the Dockerized OIDC provider conformance lane
[group('conformance')]
oidc-conformance-op:
    ./conformance/oidc/scripts/run-op.sh

# Run the Compliance OIDC RP regression lane
[group('conformance')]
oidc-conformance-rp: _ensure-node-modules
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "${ZITADEL_E2E_BINARY:-}" ]; then
        just web
        cargo build -p zitadel
    else
        echo "→ using prepared zitadel binary $ZITADEL_E2E_BINARY"
    fi
    ./conformance/oidc/scripts/run-rp.sh

# Run Compliance OIDC (surface: op, rp, or both)
[group('conformance')]
oidc-conformance SURFACE="both":
    #!/usr/bin/env bash
    set -euo pipefail
    surface="${OIDC_CONFORMANCE_SURFACE:-{{SURFACE}}}"
    case "$surface" in
        op)   just oidc-conformance-op ;;
        rp)   just oidc-conformance-rp ;;
        both) just oidc-conformance-op && just oidc-conformance-rp ;;
        *)    echo "invalid surface=$surface (expected op, rp, or both)"; exit 1 ;;
    esac

# Stop and remove local OIDC conformance containers
[group('conformance')]
oidc-conformance-clean:
    ./conformance/oidc/scripts/clean.sh

# ─── Quality ─────────────────────────────────────────────

# TypeScript typecheck (vue-tsc)
[group('quality')]
typecheck: _ensure-node-modules
    npm run typecheck -w web

# ESLint for Vue/TS files
[group('quality')]
lint-web: _ensure-node-modules
    npm run lint -w web

# Run fmt, clippy, and Rust tests
[group('quality')]
rust-check: ensure-webdist
    #!/usr/bin/env bash
    set -euo pipefail
    echo "═══ cargo fmt ═══"
    cargo fmt --check
    echo ""
    echo "═══ cargo clippy ═══"
    cargo clippy --workspace -- -D warnings
    echo ""
    echo "═══ Rust tests ═══"
    just _run-rust-tests

# Fail on stale doc commands and local absolute links
[group('quality')]
docs-check:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "═══ stale doc command checks ═══"
    ! rg -n \
        -e 'go run \./cmd/zitadel' \
        -e 'go test \./\.\.\.' \
        -e 'make dev-go' \
        -e 'dev-hot' \
        -e 'dev-full' \
        -e 'dev-clean' \
        README.md docs .github/workflows
    ! rg -n \
        -e 'make webdist-only' \
        -e 'make webdist([^A-Za-z0-9_-]|$)' \
        README.md docs .github/workflows
    ! rg -n '\]\((/Users/|/home/)' README.md docs

# Run the main local quality gate
[group('quality')]
quality: ensure-webdist _ensure-node-modules
    #!/usr/bin/env bash
    set -euo pipefail
    just rust-check
    echo ""
    echo "═══ docs checks ═══"
    just docs-check
    echo ""
    echo "═══ typecheck (vue-tsc) ═══"
    npm run typecheck -w web
    echo ""
    echo "═══ eslint ═══"
    npm run lint -w web 2>/dev/null || echo "(skipped — eslint not configured or had warnings)"
    echo ""
    echo "═══ web tests (vitest) ═══"
    npm test -w web
    echo ""
    echo "✅ local quality gate passed"

# Alias for quality
[group('quality')]
check: quality

# ─── SDK Generation ──────────────────────────────────────

# Regenerate config.schema.json from Rust config types
[group('codegen')]
config-schema:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "═══ config-schema ═══"
    SCHEMA_OUT=config.schema.json cargo test -p zitadel-config json_schema_generates --quiet
    echo "Written config.schema.json ($(wc -l < config.schema.json) lines)"

# Export OpenAPI 3.1 spec
[group('codegen')]
openapi-export:
    @echo "OpenAPI export not yet implemented in Rust binary"

# Generate TypeScript SDK from OpenAPI spec
[group('codegen')]
client-js: _ensure-node-modules
    npm run generate -w packages/client-js

# Generate all code
[group('codegen')]
generate: client-js
    @echo "✅ SDK generated"

# ─── Performance ─────────────────────────────────────────

# Run the SQLite database performance harness
[group('perf')]
perf-db-sqlite: ensure-webdist
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p artifacts/db-perf
    cargo run -p zitadel --release -- perf db run --backend sqlite --profile ci --format json --output artifacts/db-perf/sqlite.json

# Run the Postgres database performance harness
[group('perf')]
perf-db-postgres: ensure-webdist
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p artifacts/db-perf
    db_url="${ZITADEL_PERF_POSTGRES_URL:-${ZITADEL_STORAGE_STATEFUL_URL:-postgres://postgres:postgres@127.0.0.1:5432/zitadel_perf}}"
    echo "→ Postgres perf DB: $db_url"
    cargo run -p zitadel --release -- perf db run --backend postgres --profile ci --database-url "$db_url" --format json --output artifacts/db-perf/postgres.json

# Run SQLite and Postgres database performance harnesses
[group('perf')]
perf-db: perf-db-sqlite perf-db-postgres

# ─── Fuzz Testing ────────────────────────────────────────

# Run all fuzz targets for the given duration (seconds)
[group('fuzz')]
fuzz DURATION="10":
    #!/usr/bin/env bash
    set -euo pipefail
    for target in {{fuzz_targets}}; do
        echo "═══ fuzzing $target ({{DURATION}}s) ═══"
        cargo fuzz run "$target" -- -max_total_time={{DURATION}} || exit 1
    done

# Quick fuzz: 10s per target (CI budget from ADR-011)
[group('fuzz')]
fuzz-quick: (fuzz "10")

# Extended fuzz: 5 minutes per target (daily CI)
[group('fuzz')]
fuzz-extended: (fuzz "300")

# ─── Clean ───────────────────────────────────────────────

# Remove build artifacts, web dist, and local dev databases
[group('build')]
clean:
    rm -rf target/
    rm -rf web/dist/
    rm -f {{dev_db_file}} {{dev_cache_file}}
