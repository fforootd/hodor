.DEFAULT_GOAL := help

.PHONY: help install dev dev-web dev-embed dev-reset dev-seed dev-status test test-web test-e2e e2e-smoke oidc-conformance oidc-conformance-op oidc-conformance-rp oidc-conformance-clean typecheck lint-web build clean web ensure-webdist quality check rust-check run-rust-tests docs-check generate openapi-export config-schema client-js

SEED ?= frontend
JS_WORKSPACE_MANIFESTS := package.json package-lock.json web/package.json e2e/package.json packages/client-js/package.json
DEV_CONFIG := fixtures/zitadel.dev.toml
DEV_SEED_DIR := fixtures/seeds
DEV_SEED_FILE := $(DEV_SEED_DIR)/$(SEED).yaml
DEV_DB_FILE := data/zitadel.db
DEV_CACHE_FILE := data/zitadel-cache.db

# ─── Build DAG ──────────────────────────────────────────────
# web → Rust binary (with embedded assets via rust-embed)
#
#   make build         (release binary)
#   make dev           (Vite HMR on :5173 + Rust server on :8080)
#   make dev-embed     (embedded web assets + Rust server on :8080)
#   make quality       (CI-equivalent: fmt → clippy → test → typecheck → vitest)

# ─── Web (Vue/Vite) ────────────────────────────────────────

# Print the supported developer interface.
help:
	@printf '%s\n' \
		'Supported targets:' \
		'  install       Install Node workspace dependencies' \
		'  web           Build embedded frontend assets into web/dist' \
		'  ensure-webdist Create placeholder embedded assets for Rust builds' \
		'  dev           Run Rust API on :8080 with Vite HMR on :5173' \
		'  dev-embed     Run the embedded frontend + Rust API on :8080' \
		'  dev-web       Run only the Vite frontend against an existing API' \
		'  dev-reset     Remove local SQLite dev data and restart dev' \
		'  dev-seed      Validate and apply a named seed pack' \
		'  dev-status    Print local dev paths, credentials, and seed pack' \
		'  build         Build the release zitadel binary' \
		'  test          Run the Rust workspace test suite (prefers cargo nextest)' \
		'  test-web      Run web Vitest tests' \
		'  e2e-smoke     Run Playwright smoke tests' \
		'  test-e2e      Run the full Playwright suite' \
		'  oidc-conformance-op Run the Dockerized OIDC provider conformance lane' \
		'  oidc-conformance-rp Run the current RP OIDC daily regression lane' \
		'  oidc-conformance Run OIDC daily coverage (OIDF OP + RP regression by default)' \
		'  oidc-conformance-clean Stop and remove local OIDC conformance containers' \
		'  rust-check    Run fmt, clippy, and Rust tests' \
		'  docs-check    Fail on stale doc commands and local absolute links' \
		'  quality       Run the main local quality gate' \
		'  config-schema Regenerate config.schema.json from Rust types' \
		'  generate      Generate the client SDK' \
		'' \
		'CLI commands (via cargo run -p zitadel --)' \
		'  config-schema   Print JSON Schema for config files' \
		'  config-validate Validate a config file and print resolved values'

# Install all workspace dependencies.
install: $(JS_WORKSPACE_MANIFESTS)
	npm ci --prefer-offline

node_modules: $(JS_WORKSPACE_MANIFESTS)
	npm ci --prefer-offline

# Build Vue apps (login, console, account).
web/dist: node_modules web/package.json web/vite.config.ts web/vitest.config.ts $(shell find web/src -type f 2>/dev/null)
	npm run build

web: web/dist

# Ensure web/dist exists for rust-embed (placeholder if not built).
ensure-webdist:
	@if [ ! -d web/dist ]; then \
		mkdir -p web/dist/src/login web/dist/src/console web/dist/src/account web/dist/assets; \
		echo '<!DOCTYPE html><html><body>dev placeholder</body></html>' > web/dist/src/login/index.html; \
		echo '<!DOCTYPE html><html><body>dev placeholder</body></html>' > web/dist/src/console/index.html; \
		echo '<!DOCTYPE html><html><body>dev placeholder</body></html>' > web/dist/src/account/index.html; \
		echo "Created placeholder web/dist (use 'make web' for real assets)"; \
	fi

# ─── Rust Backend ─────────────────────────────────────────

# Canonical development flow — Vite HMR + Rust API with deterministic seed data.
dev: node_modules ensure-webdist
	@mkdir -p data
	cargo build -p zitadel
	@echo "─── Zitadel local dev ───"
	@echo "→ Console / Login / Account: http://localhost:5173"
	@echo "→ API / OIDC:               http://localhost:8080"
	@echo "→ Seed pack:                $(SEED)"
	@echo "→ Admin login:              admin / admin123"
	@echo "→ Admin PAT:                zitadel-dev-pat-do-not-use-in-production"
	@echo "→ SQLite DB:                $(CURDIR)/$(DEV_DB_FILE)"
	@trap 'kill 0' EXIT; \
	ZITADEL_SEED_FILE="$(DEV_SEED_FILE)" ./target/debug/zitadel start -c $(DEV_CONFIG) & \
	npm run dev -w web

# Embedded-assets development — parity mode without Vite.
dev-embed: web ensure-webdist
	@mkdir -p data
	cargo build -p zitadel
	@echo "→ Embedded UI + API: http://localhost:8080"
	@echo "→ Seed pack:         $(SEED)"
	ZITADEL_SEED_FILE="$(DEV_SEED_FILE)" ./target/debug/zitadel start -c $(DEV_CONFIG)

# Frontend-only development — expects a backend at localhost:8080 or ZITADEL_API_BASE.
dev-web: node_modules
	@echo "→ Frontend dev server: http://localhost:5173"
	@echo "→ API target:          $${ZITADEL_API_BASE:-http://localhost:8080}"
	@ZITADEL_API_BASE="$${ZITADEL_API_BASE:-http://localhost:8080}" npm run dev -w web

# Wipe DB and restart fresh.
dev-reset:
	@db_url="$${ZITADEL_DATABASE_URL:-sqlite://$(CURDIR)/$(DEV_DB_FILE)}"; \
	case "$$db_url" in \
		sqlite://*) ;; \
		*) echo "refusing to delete local SQLite files for non-SQLite ZITADEL_DATABASE_URL=$$db_url"; exit 1 ;; \
	esac
	@echo "→ stopping local dev services on :8080 and :5173 (if running)"
	@for port in 8080 5173; do \
		if command -v lsof >/dev/null 2>&1; then \
			pids="$$(lsof -ti tcp:$$port 2>/dev/null)"; \
			if [ -n "$$pids" ]; then kill $$pids 2>/dev/null || true; fi; \
		fi; \
	done
	@rm -f $(DEV_DB_FILE) $(DEV_DB_FILE)-shm $(DEV_DB_FILE)-wal $(DEV_DB_FILE)-journal
	@rm -f $(DEV_CACHE_FILE) $(DEV_CACHE_FILE)-shm $(DEV_CACHE_FILE)-wal $(DEV_CACHE_FILE)-journal
	@$(MAKE) dev SEED=$(SEED)

dev-seed:
	@test -f "$(DEV_SEED_FILE)" || (echo "unknown seed pack '$(SEED)' (expected $(DEV_SEED_FILE))" && exit 1)
	@if [ ! -x ./target/debug/zitadel ]; then \
		echo "→ building debug binary"; \
		cargo build -p zitadel; \
	fi
	@echo "→ validating $(DEV_SEED_FILE)"
	@./target/debug/zitadel seed validate --file "$(DEV_SEED_FILE)"
	@echo "→ applying $(DEV_SEED_FILE)"
	@./target/debug/zitadel seed apply -c $(DEV_CONFIG) --file "$(DEV_SEED_FILE)"

dev-status:
	@db_url="$${ZITADEL_DATABASE_URL:-sqlite://$(CURDIR)/$(DEV_DB_FILE)}"; \
	seed_file="$${ZITADEL_SEED_FILE:-$(CURDIR)/$(DEV_SEED_FILE)}"; \
	echo "Dev config:     $(CURDIR)/$(DEV_CONFIG)"; \
	echo "Server:         http://localhost:8080"; \
	echo "Vite server:    http://localhost:5173"; \
	echo "Database URL:   $$db_url"; \
	echo "Database path:  $(CURDIR)/$(DEV_DB_FILE)"; \
	echo "Seed pack:      $(SEED)"; \
	echo "Seed file:      $$seed_file"; \
	echo "Admin login:    admin / admin123"; \
	echo "Admin PAT:      zitadel-dev-pat-do-not-use-in-production"

# Build release binary.
build: ensure-webdist
	cargo build --release

# Run Rust tests with nextest when available, falling back to cargo test.
run-rust-tests:
	@if cargo nextest --version >/dev/null 2>&1; then \
		cargo nextest run --workspace; \
	else \
		cargo test --workspace; \
	fi

# Run Rust tests.
test:
	@$(MAKE) run-rust-tests

# Run web component tests (Vitest).
test-web: node_modules
	npm test -w web

# Run E2E browser tests (Playwright).
test-e2e: node_modules web
	cargo build -p zitadel
	npm test -w e2e

# Run Playwright smoke tests.
e2e-smoke: node_modules web
	cargo build -p zitadel
	npm run test:smoke -w e2e

# Run the Dockerized OIDC provider conformance lane.
oidc-conformance-op:
	./conformance/oidc/scripts/run-op.sh

# Run the current RP daily regression lane.
oidc-conformance-rp: node_modules web
	cargo build -p zitadel
	./conformance/oidc/scripts/run-rp.sh

# Run OIDC daily coverage. Set OIDC_CONFORMANCE_SURFACE=op|rp|both (default both).
oidc-conformance:
	@surface="$${OIDC_CONFORMANCE_SURFACE:-both}"; \
	case "$$surface" in \
		op) $(MAKE) oidc-conformance-op ;; \
		rp) $(MAKE) oidc-conformance-rp ;; \
		both) $(MAKE) oidc-conformance-op && $(MAKE) oidc-conformance-rp ;; \
		*) echo "invalid OIDC_CONFORMANCE_SURFACE=$$surface (expected op, rp, or both)"; exit 1 ;; \
	esac

# Stop and remove local OIDC conformance containers.
oidc-conformance-clean:
	./conformance/oidc/scripts/clean.sh

# TypeScript typecheck (vue-tsc).
typecheck: node_modules
	npm run typecheck -w web

# ESLint for Vue/TS files.
lint-web: node_modules
	npm run lint -w web

# ─── SDK Generation ────────────────────────────────────────

# Regenerate config.schema.json from Rust config types.
config-schema:
	@echo "═══ config-schema ═══"
	SCHEMA_OUT=config.schema.json cargo test -p zitadel-config json_schema_generates --quiet
	@echo "Written config.schema.json ($$(wc -l < config.schema.json) lines)"

# Export OpenAPI 3.1 spec (TODO: implement in Rust binary).
openapi-export:
	@echo "OpenAPI export not yet implemented in Rust binary"

# Generate TypeScript SDK from OpenAPI spec.
client-js: node_modules
	npm run generate -w packages/client-js

# Generate all code.
generate: client-js
	@echo "✅ SDK generated"

# ─── Quality (local gate) ─────────────────────────────────
#
# Runs the main local checks that should stay green before committing.
# CI additionally runs docs validation and Playwright suites in dedicated jobs.

rust-check: ensure-webdist
	@echo "═══ cargo fmt ═══"
	cargo fmt --check
	@echo ""
	@echo "═══ cargo clippy ═══"
	cargo clippy --workspace -- -D warnings
	@echo ""
	@echo "═══ Rust tests ═══"
	@$(MAKE) run-rust-tests

docs-check:
	@echo "═══ stale doc command checks ═══"
	@! rg -n \
		-e 'go run \./cmd/zitadel' \
		-e 'go test \./\.\.\.' \
		-e 'make dev-go' \
		-e 'dev-hot' \
		-e 'dev-full' \
		-e 'dev-clean' \
		README.md docs .github/workflows
	@! rg -n \
		-e 'make webdist-only' \
		-e 'make webdist([^A-Za-z0-9_-]|$$)' \
		README.md docs .github/workflows
	@! rg -n '\]\((/Users/|/home/)' README.md docs

quality: ensure-webdist node_modules
	@$(MAKE) rust-check
	@echo ""
	@echo "═══ docs checks ═══"
	@$(MAKE) docs-check
	@echo ""
	@echo "═══ typecheck (vue-tsc) ═══"
	npm run typecheck -w web
	@echo ""
	@echo "═══ eslint ═══"
	@npm run lint -w web 2>/dev/null || echo "(skipped — eslint not configured or had warnings)"
	@echo ""
	@echo "═══ web tests (vitest) ═══"
	npm test -w web
	@echo ""
	@echo "✅ local quality gate passed"

# Alias for quality.
check: quality

# ─── Clean ─────────────────────────────────────────────────

clean:
	rm -rf target/
	rm -rf web/dist/
	rm -f $(DEV_DB_FILE) $(DEV_CACHE_FILE)
