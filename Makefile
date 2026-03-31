.PHONY: install dev dev-go dev-web dev-embed dev-hot dev-full dev-clean dev-reset dev-seed dev-status test fuzz lint generate build clean web web-install webdist webdist-only ensure-webdist ci-test fmt fmt-check vet release-snapshot quality check openapi-export client-js test-web test-e2e typecheck lint-web test-integration

SEED ?= frontend
DEV_CONFIG := fixtures/zitadel.dev.toml
DEV_SEED_DIR := fixtures/seeds
DEV_SEED_FILE := $(DEV_SEED_DIR)/$(SEED).yaml
DEV_DB_FILE := data/zitadel.db
DEV_CACHE_FILE := data/zitadel-cache.db
DEV_GO_CMD := ZITADEL_SEED_FILE="$(DEV_SEED_FILE)" go run ./cmd/zitadel start -c $(DEV_CONFIG)

# ─── Build DAG ──────────────────────────────────────────────
# web → webdist → Go binary
#
#   make build         (full pipeline: web → webdist → goreleaser)
#   make dev           (integrated: Vite HMR on :5173 + Go server on :8080)
#   make dev-embed     (embedded web assets + Go server on :8080)
#   make quality       (CI-equivalent: build → fmt → vet → lint → typecheck → tests)

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

# Copy built web assets into Go embed directory.
internal/server/webdist: web/dist
	rm -rf internal/server/webdist
	cp -r web/dist internal/server/webdist

webdist: internal/server/webdist

# Copy only (assumes web/dist already exists, used by CI).
webdist-only:
	rm -rf internal/server/webdist
	cp -r web/dist internal/server/webdist

# Create minimal placeholder webdist so go build/test works without -tags devweb.
ensure-webdist:
	@if [ ! -d internal/server/webdist ]; then \
		mkdir -p internal/server/webdist/src/login internal/server/webdist/src/console internal/server/webdist/src/account internal/server/webdist/assets; \
		echo '<!DOCTYPE html><html><body>dev placeholder</body></html>' > internal/server/webdist/src/login/index.html; \
		echo '<!DOCTYPE html><html><body>dev placeholder</body></html>' > internal/server/webdist/src/console/index.html; \
		echo '<!DOCTYPE html><html><body>dev placeholder</body></html>' > internal/server/webdist/src/account/index.html; \
		echo "Created placeholder webdist (use 'make webdist' for real assets)"; \
	fi

# ─── Go ────────────────────────────────────────────────────

# Canonical development flow — Vite HMR + Go API with deterministic seed data.
dev: node_modules webdist
	@mkdir -p data
	@echo "─── Zitadel local dev ───"
	@echo "→ Console / Login / Account: http://localhost:5173"
	@echo "→ API / OIDC:               http://localhost:8080"
	@echo "→ Seed pack:                $(SEED)"
	@echo "→ Admin login:              admin / admin123"
	@echo "→ Admin PAT:                zitadel-dev-pat-do-not-use-in-production"
	@echo "→ SQLite DB:                $(CURDIR)/$(DEV_DB_FILE)"
	@echo "→ SQLite cache:             $(CURDIR)/$(DEV_CACHE_FILE)"
	@trap 'kill 0' EXIT; \
	$(DEV_GO_CMD) & \
	npm run dev -w web

# Backend-only development — reuses embedded assets when already built.
dev-go:
	@if [ ! -d internal/server/webdist ]; then \
		echo "embedded web assets are missing — run 'make webdist' or 'make dev' once first"; \
		exit 1; \
	fi
	@mkdir -p data
	@echo "→ Backend only: http://localhost:8080"
	@echo "→ Seed pack:    $(SEED)"
	@$(DEV_GO_CMD)

# Frontend-only development — expects a backend at localhost:8080 or ZITADEL_API_BASE.
dev-web: node_modules
	@echo "→ Frontend dev server: http://localhost:5173"
	@echo "→ API target:          $${ZITADEL_API_BASE:-http://localhost:8080}"
	@ZITADEL_API_BASE="$${ZITADEL_API_BASE:-http://localhost:8080}" npm run dev -w web

# Embedded-assets development — parity mode without Vite.
dev-embed: webdist
	@mkdir -p data
	@echo "→ Embedded UI + API: http://localhost:8080"
	@echo "→ Seed pack:         $(SEED)"
	@$(DEV_GO_CMD)

# Deprecated aliases kept for contributor muscle memory.
dev-hot:
	@echo "make dev-hot is deprecated; using 'make dev'"
	@$(MAKE) dev SEED=$(SEED)

dev-full:
	@echo "make dev-full is deprecated; using 'make dev-embed'"
	@$(MAKE) dev-embed SEED=$(SEED)

dev-clean:
	@echo "make dev-clean is deprecated; using 'make dev-reset'"
	@$(MAKE) dev-reset SEED=$(SEED)

dev-reset:
	@db_url="$${ZITADEL_DATABASE_URL:-sqlite://$(CURDIR)/$(DEV_DB_FILE)}"; \
	case "$$db_url" in \
		sqlite://:memory:*) \
			echo "refusing to reset an in-memory SQLite database"; \
			exit 1 ;; \
		sqlite://*) ;; \
		*) \
			echo "refusing to reset local data because database URL is not a local SQLite file: $$db_url"; \
			exit 1 ;; \
	esac; \
	echo "→ stopping local dev services on :8080 and :5173 (if running)"; \
	for port in 8080 5173; do \
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
	@go run ./cmd/zitadel seed validate --file "$(DEV_SEED_FILE)"
	@echo "→ applying $(DEV_SEED_FILE)"
	@go run ./cmd/zitadel seed apply -c $(DEV_CONFIG) --file "$(DEV_SEED_FILE)"

dev-status:
	@db_url="$${ZITADEL_DATABASE_URL:-sqlite://$(CURDIR)/$(DEV_DB_FILE)}"; \
	seed_file="$${ZITADEL_SEED_FILE:-$(CURDIR)/$(DEV_SEED_FILE)}"; \
	echo "Dev config:     $(CURDIR)/$(DEV_CONFIG)"; \
	echo "Go server:      http://localhost:8080"; \
	echo "Vite server:    http://localhost:5173"; \
	echo "Database URL:   $$db_url"; \
	echo "Database path:  $(CURDIR)/$(DEV_DB_FILE)"; \
	echo "Cache path:     $(CURDIR)/$(DEV_CACHE_FILE)"; \
	echo "Seed pack:      $(SEED)"; \
	echo "Seed file:      $$seed_file"; \
	echo "Mock OIDC:      enabled"; \
	echo "Admin login:    admin / admin123"; \
	echo "Admin PAT:      zitadel-dev-pat-do-not-use-in-production"
	@go run ./cmd/zitadel seed validate --file "$(DEV_SEED_FILE)"

# Run all tests (requires webdist for embed).
test: webdist
	go test ./... -v -count=1 -timeout 120s

# CI test target — with race detector.
ci-test: webdist
	go test -race -count=1 -timeout 120s ./...

# Run integration tests (requires docker for testcontainers).
test-integration:
	go test -v -count=1 -timeout 300s ./internal/database/...

# Run web component tests (Vitest).
test-web: node_modules
	npm test -w web

# Run E2E browser tests (Playwright).
test-e2e: build
	npm test -w e2e

# TypeScript typecheck (vue-tsc).
typecheck: node_modules
	npm run typecheck -w web

# ESLint for Vue/TS files.
lint-web: node_modules
	npm run lint -w web

# Run all fuzz tests (default 10s per target).
fuzz: webdist
	@echo "═══ Fuzz Tests (10s each) ═══"
	go test -fuzz FuzzParseIDTokenClaims -fuzztime 10s ./internal/login/
	go test -fuzz FuzzMapClaims -fuzztime 10s ./internal/login/
	go test -fuzz FuzzAPIJSON -fuzztime 10s ./internal/api/
	go test -fuzz FuzzSessionToken -fuzztime 10s ./internal/api/
	go test -fuzz FuzzBearerTokenResolution -fuzztime 10s ./internal/api/
	go test -fuzz FuzzCookieTokenResolution -fuzztime 10s ./internal/api/
	go test -fuzz FuzzXIdentityIdHeader -fuzztime 10s ./internal/api/
	go test -fuzz FuzzCookieVerify -fuzztime 10s ./internal/session/
	go test -fuzz FuzzCookieSign -fuzztime 10s ./internal/session/
	go test -fuzz FuzzCookieSignVerifyRoundTrip -fuzztime 10s ./internal/session/
	go test -fuzz FuzzExtractHash -fuzztime 10s ./internal/auth/
	go test -fuzz FuzzPasswordHash -fuzztime 10s ./internal/auth/
	go test -fuzz FuzzNew -fuzztime 10s ./internal/id/
	go test -fuzz FuzzRedactor_IsSensitive -fuzztime 10s ./internal/logging/
	go test -fuzz FuzzRedactor_RedactRecord -fuzztime 10s ./internal/logging/
	go test -fuzz FuzzRedactor_RedactValue_Group -fuzztime 10s ./internal/logging/
	go test -fuzz FuzzCircuitBreaker -fuzztime 10s ./internal/logging/
	go test -fuzz FuzzFanOutHandler -fuzztime 10s ./internal/logging/
	@echo "✅ fuzz complete"

# Lint (requires golangci-lint installed).
lint: webdist
	golangci-lint run ./...

# Format Go code.
fmt:
	gofmt -w .

# Check Go formatting.
fmt-check:
	@if [ -n "$$(gofmt -l .)" ]; then \
		echo "Files not formatted:"; \
		gofmt -l .; \
		exit 1; \
	fi

# Go vet.
vet: webdist
	go vet ./...

# Build via goreleaser (snapshot, no publish).
build: webdist
	goreleaser build --snapshot --clean --single-target

# Full release build (snapshot).
release-snapshot: webdist
	goreleaser release --snapshot --clean

# ─── SDK Generation ────────────────────────────────────────

# Export OpenAPI 3.1 spec from Go types (no server/DB needed).
openapi-export: webdist
	go run ./cmd/zitadel openapi-export > packages/openapi.json

# Generate TypeScript SDK from OpenAPI spec.
client-js: openapi-export node_modules
	npm run generate -w packages/client-js

# Generate all code.
generate: client-js
	@echo "✅ SDK generated"

# ─── Quality (all-in-one CI gate) ─────────────────────────
#
# Runs the same checks as CI. Use before committing.
# Matches ci.yml: lint-go + lint-web + test-go + test-web
#
#   1. fmt-check       — formatting
#   2. go vet          — static analysis
#   3. golangci-lint    — deep linting (optional locally)
#   4. typecheck       — vue-tsc (matches CI lint-web)
#   5. lint-web        — eslint for Vue/TS
#   6. go test -race   — Go tests with race detector (matches CI)
#   7. test-web        — Vitest (matches CI test-web)

quality: webdist node_modules
	@echo "═══ go fmt ═══"
	@$(MAKE) fmt-check
	@echo ""
	@echo "═══ go vet ═══"
	go vet ./...
	@echo ""
	@echo "═══ golangci-lint ═══"
	@if command -v golangci-lint >/dev/null 2>&1; then \
		golangci-lint run ./...; \
	else \
		echo "(skipped — golangci-lint not installed)"; \
	fi
	@echo ""
	@echo "═══ typecheck (vue-tsc) ═══"
	npm run typecheck -w web
	@echo ""
	@echo "═══ eslint ═══"
	@npm run lint -w web 2>/dev/null || echo "(skipped — eslint not configured or had warnings)"
	@echo ""
	@echo "═══ go test -race ═══"
	go test -race -count=1 -timeout 240s ./...
	@echo ""
	@echo "═══ web tests (vitest) ═══"
	npm test -w web
	@echo ""
	@echo "✅ quality gate passed"

# Alias for quality.
check: quality

# ─── Clean ─────────────────────────────────────────────────

clean:
	rm -rf dist/
	rm -rf internal/server/webdist/
	rm -rf web/dist/
	rm -f zitadel

# ─── Performance Benchmarks ───────────────────────────────

.PHONY: bench bench-scale

bench: ## Run benchmarks (default GOMAXPROCS)
	@echo "═══ Benchmarks (GOMAXPROCS=$$(sysctl -n hw.ncpu 2>/dev/null || nproc)) ═══"
	go test -bench=. -benchmem -count=3 -timeout 300s \
		./internal/database/ ./internal/api/ 2>&1 | grep -v 'schema ready\|Columns in\|seeded\|bootstrapped\|alias.*registered\|analytics.*OLTP\|OIDC Provider\|Path config'

bench-scale: ## Run vCPU scaling sweep (1 → N cores)
	@mkdir -p bench-results
	@for procs in 1 2 4 $$(sysctl -n hw.ncpu 2>/dev/null || nproc); do \
		echo ""; \
		echo "═══ GOMAXPROCS=$$procs ═══"; \
		GOMAXPROCS=$$procs go test -bench=. -benchmem -count=5 -timeout 600s \
			./internal/database/ ./internal/api/ 2>&1 \
			| grep -v 'schema ready\|Columns in\|seeded\|bootstrapped\|alias.*registered\|analytics.*OLTP\|OIDC Provider\|Path config' \
			| tee bench-results/bench-$$procs.txt; \
	done
	@echo ""
	@echo "═══ Scaling Analysis ═══"
	@if command -v benchstat >/dev/null 2>&1; then \
		benchstat bench-results/bench-1.txt bench-results/bench-2.txt bench-results/bench-4.txt; \
	else \
		echo "(install benchstat: go install golang.org/x/perf/cmd/benchstat@latest)"; \
	fi
