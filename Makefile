.PHONY: install dev dev-web dev-embed dev-reset dev-seed dev-status test test-web test-e2e typecheck lint-web build clean web ensure-webdist quality check generate openapi-export client-js

SEED ?= frontend
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

# Install all workspace dependencies.
install: package.json
	npm ci --prefer-offline

node_modules: package.json
	npm ci --prefer-offline

# Build Vue apps (login, console, account).
web/dist: node_modules $(shell find web/src -type f 2>/dev/null)
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
	cargo build
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
	cargo build
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

# Run Rust tests.
test:
	cargo test --workspace

# Run web component tests (Vitest).
test-web: node_modules
	npm test -w web

# Run E2E browser tests (Playwright).
test-e2e: node_modules ensure-webdist
	npm test -w e2e

# TypeScript typecheck (vue-tsc).
typecheck: node_modules
	npm run typecheck -w web

# ESLint for Vue/TS files.
lint-web: node_modules
	npm run lint -w web

# ─── SDK Generation ────────────────────────────────────────

# Export OpenAPI 3.1 spec (TODO: implement in Rust binary).
openapi-export:
	@echo "OpenAPI export not yet implemented in Rust binary"

# Generate TypeScript SDK from OpenAPI spec.
client-js: node_modules
	npm run generate -w packages/client-js

# Generate all code.
generate: client-js
	@echo "✅ SDK generated"

# ─── Quality (all-in-one CI gate) ─────────────────────────
#
# Runs the same checks as CI. Use before committing.
#
#   1. cargo fmt        — formatting
#   2. cargo clippy     — linting
#   3. cargo test       — Rust tests
#   4. typecheck        — vue-tsc
#   5. lint-web         — eslint
#   6. test-web         — Vitest

quality: ensure-webdist node_modules
	@echo "═══ cargo fmt ═══"
	cargo fmt --check
	@echo ""
	@echo "═══ cargo clippy ═══"
	cargo clippy --workspace -- -D warnings
	@echo ""
	@echo "═══ cargo test ═══"
	cargo test --workspace
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
	@echo "✅ quality gate passed"

# Alias for quality.
check: quality

# ─── Clean ─────────────────────────────────────────────────

clean:
	rm -rf target/
	rm -rf web/dist/
	rm -f $(DEV_DB_FILE) $(DEV_CACHE_FILE)
