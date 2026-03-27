.PHONY: dev dev-hot dev-full dev-clean test fuzz lint generate build clean web web-install webdist ci-test fmt vet release-snapshot

# ─── Build DAG ──────────────────────────────────────────────
# web → webdist → Go binary
#
#   make build         (full pipeline: web → webdist → goreleaser)
#   make dev           (fast: assumes webdist exists, runs Go server)
#   make dev-hot       (Vite HMR on :5173 + Go server on :8080)
#   make ci-test       (CI: web → webdist → go test)

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

# ─── Go ────────────────────────────────────────────────────

# Development — run server with embedded assets.
dev: webdist
	go run ./cmd/zitadel serve

# Development with mock OIDC + seed data.
dev-full: webdist
	go run ./cmd/zitadel serve -c fixtures/zitadel.dev.toml

# Hot reload development — Vite HMR on :5173 proxying to Go on :8080.
# Access the app at http://localhost:5173 for instant CSS/JS reloads.
# Go server still needs manual restart on .go changes (use `air` for that).
dev-hot: node_modules
	@echo "─── Starting Vite dev server (:5173) + Go server (:8080) ───"
	@echo "→  Open http://localhost:5173 for hot-reload UI"
	@echo "→  API calls proxy to http://localhost:8080"
	@-pkill -f "cmd/zitadel" 2>/dev/null || true
	@rm -f zitadel.db-journal zitadel.db-wal zitadel.db-shm
	@sleep 0.5
	@trap 'kill 0' EXIT; \
	npm run dev & \
	go run ./cmd/zitadel serve -c fixtures/zitadel.dev.toml

# Clean start — wipe DB and restart with dev config.
dev-clean:
	rm -f zitadel.db zitadel.db-shm zitadel.db-wal zitadel.db-journal
	$(MAKE) dev-hot

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

# Run fuzz tests (default 10s per target).
fuzz: webdist
	go test -fuzz FuzzParseIDTokenClaims -fuzztime 10s ./internal/login/
	go test -fuzz FuzzMapClaims -fuzztime 10s ./internal/login/

# Lint (requires golangci-lint installed).
lint: webdist
	golangci-lint run ./...

# Format Go code.
fmt:
	gofmt -w .

# Go vet.
vet: webdist
	go vet ./...

# Build via goreleaser (snapshot, no publish).
build: webdist
	goreleaser build --snapshot --clean --single-target

# Full release build (snapshot).
release-snapshot: webdist
	goreleaser release --snapshot --clean

# Generate code (proto, templ, sqlc — placeholders).
generate:
	@echo "TODO: buf generate"
	@echo "TODO: templ generate"
	@echo "TODO: sqlc generate"

# ─── Clean ─────────────────────────────────────────────────

clean:
	rm -rf dist/
	rm -rf internal/server/webdist/
	rm -rf web/dist/
	rm -f zitadel
