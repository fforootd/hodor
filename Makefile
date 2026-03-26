.PHONY: dev dev-hot dev-full test fuzz lint generate build clean web web-install webdist ci-test fmt vet release-snapshot

# ─── Build DAG ──────────────────────────────────────────────
# web → webdist → Go binary
#
#   make build         (full pipeline: web → webdist → goreleaser)
#   make dev           (fast: assumes webdist exists, runs Go server)
#   make dev-hot       (Vite HMR on :5173 + Go server on :8080)
#   make ci-test       (CI: web → webdist → go test)

# ─── Web (Vue/Vite) ────────────────────────────────────────

# Install web dependencies.
web-install: web/package.json
	cd web && npm ci --prefer-offline

web/node_modules: web/package.json
	cd web && npm ci --prefer-offline

# Build Vue apps (login, console, account).
web/dist: web/node_modules $(shell find web/src -type f 2>/dev/null)
	cd web && npx vite build

web: web/dist

# Copy built web assets into Go embed directory.
internal/server/webdist: web/dist
	rm -rf internal/server/webdist
	cp -r web/dist internal/server/webdist

webdist: internal/server/webdist

# ─── Go ────────────────────────────────────────────────────

# Development — run server with embedded assets.
dev: webdist
	go run ./cmd/zitadel serve

# Development with mock OIDC + seed data.
dev-full: webdist
	go run ./cmd/zitadel serve --mock-oidc --seed fixtures/dev-seed.yaml

# Hot reload development — Vite HMR on :5173 proxying to Go on :8080.
# Access the app at http://localhost:5173 for instant CSS/JS reloads.
# Go server still needs manual restart on .go changes (use `air` for that).
dev-hot: web/node_modules
	@echo "─── Starting Vite dev server (:5173) + Go server (:8080) ───"
	@echo "→  Open http://localhost:5173 for hot-reload UI"
	@echo "→  API calls proxy to http://localhost:8080"
	@trap 'kill 0' EXIT; \
	cd web && npm run dev & \
	go run ./cmd/zitadel serve

# Run all tests (requires webdist for embed).
test: webdist
	go test ./... -v -count=1 -timeout 120s

# CI test target — with race detector.
ci-test: webdist
	go test -race -count=1 -timeout 120s ./...

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
