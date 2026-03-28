# ADR-014: Performance Testing Strategy — vCPU Scaling & Hot Path Benchmarks

- **Status**: Proposed
- **Date**: 2026-03-28
- **Authors**: Zitadel Architecture Team

## Context

Zitadel's hot path consists of:
1. **Identity CRUD** — Create/Read/Update/Delete user entities (the most common admin API operation)
2. **Session lifecycle** — Create session + issue token (every login), verify token (every authenticated request), revoke session (logout)
3. **Token resolution** — Validate bearer tokens on every API request (the single most-called code path)

As a self-hosted identity provider, Zitadel must provide predictable performance characteristics so operators can capacity-plan. The key question: **how does throughput scale per vCPU?** — i.e., can an operator double throughput by doubling CPU allocation?

### What Layer to Test?

We need both layers because they have different scaling profiles:

| Layer | What it tells you | Scaling factor |
|-------|------------------|----------------|
| **DB benchmarks** (`testing.B`) | Pure data-path throughput: SQLite WAL contention, query plan quality, index hit rates | Single-writer lock (SQLite) or connection pool (Postgres) |
| **API benchmarks** (HTTP round-trip) | End-to-end latency including JSON ser/de, middleware, auth, cookie handling | `GOMAXPROCS` / goroutine scheduling |

**DB-only benchmarks** answer: "Is the schema efficient? Does WAL mode scale under concurrent reads?"
**API benchmarks** answer: "What latency does a real client see? Where is time actually spent?"
**vCPU scaling** answers: "Should we give this workload more CPU or more memory?"

### Why Go's `testing.B` + `GOMAXPROCS`?

Go's standard benchmark tooling supports everything we need without external dependencies:
- `b.RunParallel()` — concurrent load with controlled goroutine count
- `GOMAXPROCS=N` — restrict Go scheduler to N OS threads, simulating N vCPUs
- `-benchmem` — reports allocations per operation
- `-count` — repeat for statistical stability
- JSON output — machine-parseable for CI trend tracking

No need for k6, wrk, or external load generators at this stage. Go benchmarks give us reproducible, single-binary results that run in CI.

## Decision

Implement a **two-layer benchmark suite** using Go's `testing.B` framework:

### Layer 1: Database Benchmarks (`internal/database/bench_test.go`)

Direct SQL operations against a real SQLite database, measuring raw data-path performance.

| Benchmark | What it measures |
|-----------|-----------------|
| `BenchmarkInsertIdentity` | Single identity INSERT (write throughput) |
| `BenchmarkGetIdentity` | Single identity SELECT by PK (read latency) |
| `BenchmarkListIdentities` | Paginated list query (scan performance) |
| `BenchmarkUpdateIdentity` | Single identity UPDATE (write contention) |
| `BenchmarkInsertSession` | Session + token INSERT in a transaction |
| `BenchmarkResolveToken` | Token hash lookup → session → identity JOIN (the hottest path) |
| `BenchmarkConcurrentReads` | Parallel GET identity with `b.RunParallel()` |
| `BenchmarkConcurrentMixed` | 80% reads / 20% writes — realistic workload mix |

Each benchmark:
- Creates a fresh SQLite database in `b.TempDir()` with schema applied
- Pre-seeds N identities to warm indexes
- Uses `b.ResetTimer()` after setup to exclude setup cost
- Reports `b.ReportMetric()` for custom metrics (e.g., rows/sec)

### Layer 2: API Benchmarks (`internal/api/bench_test.go`)

Full HTTP round-trip through `httptest.Server`, measuring end-to-end latency including:
- JSON serialization/deserialization
- Auth middleware (token verification on every request)
- Session cookie handling
- Database round-trip

| Benchmark | What it measures |
|-----------|-----------------|
| `BenchmarkAPICreateIdentity` | POST /v1/entities — full create flow |
| `BenchmarkAPIGetIdentity` | GET /v1/entities/{id} — authenticated read |
| `BenchmarkAPIListIdentities` | GET /v1/entities — paginated list |
| `BenchmarkAPICreateSession` | POST /v1/sessions — login flow + token issuance |
| `BenchmarkAPIResolveToken` | GET /v1/account/profile — token verification hot path |
| `BenchmarkAPIParallelReads` | Concurrent GET /v1/entities/{id} with RunParallel |
| `BenchmarkAPIParallelMixed` | Concurrent 80/20 read/write mix |

Each benchmark:
- Uses `testutil.NewTestServer(b)` (adapted for `testing.B`)  
- Pre-authenticates with an admin PAT to exclude login overhead
- Uses `b.ResetTimer()` after setup

### vCPU Scaling Analysis

To answer "how does Zitadel scale per vCPU", we run the benchmark suite across multiple `GOMAXPROCS` values:

```bash
# Run the scaling sweep
for procs in 1 2 4 8; do
  GOMAXPROCS=$procs go test -bench=. -benchmem -count=5 -timeout 600s \
    ./internal/database/ ./internal/api/ \
    | tee bench-$procs.txt
done
```

Then use `benchstat` to compare across GOMAXPROCS values:

```bash
benchstat bench-1.txt bench-2.txt bench-4.txt bench-8.txt
```

This produces a table showing:
- **Linear scaling**: ops/sec doubles when GOMAXPROCS doubles → CPU-bound, add more vCPUs
- **Sub-linear scaling**: ops/sec plateaus → contention (SQLite single-writer, mutex, etc.)  
- **Regression**: ops/sec decreases → lock contention overhead exceeds parallelism benefit

### Makefile Integration

```makefile
.PHONY: bench bench-scale

bench:  ## Run benchmarks (default GOMAXPROCS)
    go test -bench=. -benchmem -count=3 -timeout 300s \
        ./internal/database/ ./internal/api/

bench-scale:  ## Run vCPU scaling sweep (1→N cores)
    @mkdir -p bench-results
    @for procs in 1 2 4 $$(sysctl -n hw.ncpu 2>/dev/null || nproc); do \
        echo "═══ GOMAXPROCS=$$procs ═══"; \
        GOMAXPROCS=$$procs go test -bench=. -benchmem -count=5 -timeout 600s \
            ./internal/database/ ./internal/api/ \
            | tee bench-results/bench-$$procs.txt; \
    done
    @echo ""
    @echo "═══ Scaling Analysis ═══"
    @go run golang.org/x/perf/cmd/benchstat@latest \
        bench-results/bench-1.txt bench-results/bench-2.txt \
        bench-results/bench-4.txt
```

### CI Integration

Performance benchmarks are **not** part of the regular CI quality gate (they're slow and noisy). Instead:

1. **On-demand**: triggered manually via `workflow_dispatch` or `/bench` comment
2. **Nightly**: scheduled run on `main` to track regressions
3. **PR comparison**: optional — compare branch vs main using `benchstat`

```yaml
# .github/workflows/bench.yml
name: Benchmarks
on:
  workflow_dispatch:
  schedule:
    - cron: '0 6 * * *'  # daily 06:00 UTC
```

## Test Design Principles

### 1. Seed Size Matters
Pre-seed with **1,000 identities** to give the query planner realistic cardinality. An empty database produces artificially fast benchmarks that don't reflect production.

### 2. Deterministic Setup, Random Access
Use sequential creation during setup, but random access patterns during measurement (random PK lookups) to avoid caching artifacts.

### 3. Transactions Match Production
The session-creation benchmark must use the same transaction pattern as production code (BEGIN → INSERT session → INSERT token → COMMIT) to measure real lock contention.

### 4. Report Custom Metrics
Beyond ns/op and allocs/op, report domain-specific metrics:
- **tokens/sec** for token resolution
- **sessions/sec** for session creation  
- **identities/sec** for CRUD operations

### 5. Baseline Expectations

These are initial targets based on comparable systems:

| Operation | Target (1 vCPU, SQLite) | Target (4 vCPU, Postgres) |
|-----------|------------------------|--------------------------|
| Token resolve | > 10,000 ops/sec | > 40,000 ops/sec |
| Identity GET | > 8,000 ops/sec | > 30,000 ops/sec |
| Identity CREATE | > 2,000 ops/sec | > 8,000 ops/sec |
| Session CREATE | > 1,500 ops/sec | > 6,000 ops/sec |
| Identity LIST (page 50) | > 1,000 ops/sec | > 4,000 ops/sec |

These are aspirational. The first run establishes actual baselines.

## Consequences

### Positive
- **Data-driven capacity planning**: operators can predict throughput from vCPU count
- **Regression detection**: nightly benchmarks catch performance degradations early
- **Architecture validation**: the scaling profile reveals whether to invest in DB optimization (sub-linear) or API optimization (linear but slow)
- **No external dependencies**: uses Go's stdlib `testing.B` — runs on any machine

### Negative
- **SQLite ≠ Postgres scaling**: SQLite's single-writer lock means write benchmarks won't predict Postgres performance. Mitigated by: DB benchmarks also run against Postgres in the `test-postgres` CI job.
- **Machine noise**: laptop benchmarks have variance. Mitigated by: `-count=5` + `benchstat` for statistical analysis.
- **Maintenance**: benchmarks must be updated when API surface changes.

## File Layout

```
internal/
├── database/
│   └── bench_test.go      # Layer 1: DB-level benchmarks
├── api/
│   └── bench_test.go      # Layer 2: API-level benchmarks
└── testutil/
    └── testutil.go         # NewBenchServer helper (extends TestServer for testing.B)

docs/adr/
└── 014-performance-testing.md   # This document

Makefile                     # bench + bench-scale targets
```

## References

- [Go benchmark best practices](https://dave.cheney.net/2013/06/30/how-to-write-benchmarks-in-go)
- [`benchstat` documentation](https://pkg.go.dev/golang.org/x/perf/cmd/benchstat)
- [SQLite WAL mode performance](https://www.sqlite.org/wal.html)
- [GOMAXPROCS and goroutine scheduling](https://pkg.go.dev/runtime#GOMAXPROCS)
