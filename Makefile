.PHONY: dev test fuzz lint generate build clean

# Development — run the server locally.
dev:
	go run ./cmd/zitadel serve

# Run all tests.
test:
	go test ./... -v -count=1

# Run fuzz tests (default 10s per target).
fuzz:
	go test ./internal/id/ -fuzz=. -fuzztime=10s

# Lint (requires golangci-lint).
lint:
	golangci-lint run ./...

# Generate code (proto → ConnectRPC, templ, sqlc).
generate:
	$(shell go env GOPATH)/bin/buf generate
	@echo "TODO: templ generate"
	@echo "TODO: sqlc generate"

# Build via goreleaser (snapshot, no publish).
build:
	goreleaser build --snapshot --clean --single-target

# Full release build (snapshot).
release-snapshot:
	goreleaser release --snapshot --clean

# Clean build artifacts.
clean:
	rm -rf dist/
	rm -f zitadel
