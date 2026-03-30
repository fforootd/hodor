package api_test

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/bootstrap"
	"github.com/zitadel/zitadel/internal/config"
	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/server"
)

// benchServer creates a full Zitadel server for API benchmarks.
type benchServer struct {
	ts    *httptest.Server
	db    *database.DB
	token string   // admin PAT for authenticated requests
	ids   []string // pre-seeded identity IDs
}

// benchRandN returns a deterministic-ish index for benchmark workload distribution.
// Uses time-based entropy — crypto strength is unnecessary for test index selection.
func benchRandN(n int) int {
	return int(uint(time.Now().UnixNano()) % uint(n))
}

func newBenchServer(b *testing.B, seedCount int) *benchServer {
	b.Helper()

	dir := b.TempDir()
	dbPath := filepath.Join(dir, "bench.db")

	cfg := config.Defaults()
	cfg.Database.URL = "sqlite://" + dbPath

	db, err := database.Open(cfg.Database.URL)
	if err != nil {
		b.Fatalf("open db: %v", err)
	}
	if err := database.Migrate(db); err != nil {
		b.Fatalf("migrate: %v", err)
	}
	if err := bootstrap.EnsureAdmin(b.Context(), db, ""); err != nil {
		b.Fatalf("bootstrap: %v", err)
	}

	bus := eventbus.New()
	srv := server.New(cfg, db, bus)
	ts := httptest.NewServer(srv.Handler())

	b.Cleanup(func() {
		ts.Close()
		db.SQL().Exec("PRAGMA wal_checkpoint(TRUNCATE)")
		db.Close()
	})

	// Get admin ID and create a PAT.
	var adminID string
	db.SQL().QueryRow(`SELECT id FROM users WHERE identifier = 'admin'`).Scan(&adminID)

	token := createBenchPAT(b, db, adminID)

	// Pre-seed identities.
	ids := make([]string, seedCount)
	now := time.Now().UTC().Format("2006-01-02 15:04:05")
	tx, _ := db.SQL().Begin()
	for i := 0; i < seedCount; i++ {
		ids[i] = id.New()
		tx.Exec(
			`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, metadata, created_at, updated_at)
			 VALUES (?, '_global', ?, ?, 'human', 'active', '{}', ?, ?)`,
			ids[i], fmt.Sprintf("bench-user-%d", i), fmt.Sprintf("Bench User %d", i), now, now)
	}
	if err := tx.Commit(); err != nil {
		b.Fatalf("commit seed: %v", err)
	}

	return &benchServer{ts: ts, db: db, token: token, ids: ids}
}

func createBenchPAT(b *testing.B, db *database.DB, userID string) string {
	b.Helper()

	raw := "zit_pat_" + id.New()
	hash := crypto.HashTokenHex(raw)
	tokenID := id.New()
	now := time.Now().UTC().Format("2006-01-02 15:04:05")

	_, err := db.SQL().Exec(
		`INSERT INTO tokens (id, type, token_hash, user_id, name, scopes, created_at)
		 VALUES (?, 'pat', ?, ?, 'bench-pat', '["admin"]', ?)`,
		tokenID, hash, userID, now)
	if err != nil {
		b.Fatalf("insert PAT: %v", err)
	}
	return raw
}

func benchDoJSON(b *testing.B, bs *benchServer, method, path string, body any) (int, map[string]any) {
	b.Helper()
	var reqBody io.Reader
	if body != nil {
		j, _ := json.Marshal(body)
		reqBody = bytes.NewReader(j)
	}
	req, _ := http.NewRequest(method, bs.ts.URL+path, reqBody)
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+bs.token)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		b.Fatalf("%s %s: %v", method, path, err)
	}
	defer resp.Body.Close()
	var result map[string]any
	_ = json.NewDecoder(resp.Body).Decode(&result)
	return resp.StatusCode, result
}

const apiSeedSize = 500

// ──────────────────────────────────────────────────────────────
// Layer 2: API single-operation benchmarks
// ──────────────────────────────────────────────────────────────

func BenchmarkAPICreateIdentity(b *testing.B) {
	bs := newBenchServer(b, 0)

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		status, _ := benchDoJSON(b, bs, "POST", "/v1/users", map[string]any{
			"identifier":   fmt.Sprintf("api-create-%d", i),
			"display_name": fmt.Sprintf("API Create %d", i),
		})
		if status != http.StatusCreated {
			b.Fatalf("create: got %d", status)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "identities/sec")
}

func BenchmarkAPIGetIdentity(b *testing.B) {
	bs := newBenchServer(b, apiSeedSize)

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		target := bs.ids[benchRandN(len(bs.ids))]
		status, _ := benchDoJSON(b, bs, "GET", "/v1/users/"+target, nil)
		if status != http.StatusOK {
			b.Fatalf("get: got %d", status)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "reads/sec")
}

func BenchmarkAPIListIdentities(b *testing.B) {
	bs := newBenchServer(b, apiSeedSize)

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		status, _ := benchDoJSON(b, bs, "GET", "/v1/users?limit=50", nil)
		if status != http.StatusOK {
			b.Fatalf("list: got %d", status)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "pages/sec")
}

func BenchmarkAPICreateSession(b *testing.B) {
	bs := newBenchServer(b, 100)

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		userID := bs.ids[benchRandN(len(bs.ids))]
		status, _ := benchDoJSON(b, bs, "POST", "/v1/sessions", map[string]any{
			"user_id": userID,
		})
		if status != http.StatusCreated {
			b.Fatalf("create session: got %d", status)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "sessions/sec")
}

func BenchmarkAPIResolveToken(b *testing.B) {
	bs := newBenchServer(b, 10)
	// The admin PAT is validated on every request — this IS the token resolve path.

	b.ResetTimer()
	b.ReportAllocs()

	for i := 0; i < b.N; i++ {
		status, _ := benchDoJSON(b, bs, "GET", "/v1/users?limit=1", nil)
		if status != http.StatusOK {
			b.Fatalf("resolve: got %d", status)
		}
	}
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "tokens/sec")
}

// ──────────────────────────────────────────────────────────────
// Layer 2: Concurrency benchmarks (vCPU scaling focus)
// ──────────────────────────────────────────────────────────────

func BenchmarkAPIParallelReads(b *testing.B) {
	bs := newBenchServer(b, apiSeedSize)

	b.ResetTimer()
	b.ReportAllocs()

	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			target := bs.ids[benchRandN(len(bs.ids))]
			req, _ := http.NewRequest("GET", bs.ts.URL+"/v1/users/"+target, nil)
			req.Header.Set("Authorization", "Bearer "+bs.token)
			resp, err := http.DefaultClient.Do(req)
			if err != nil {
				b.Fatalf("parallel get: %v", err)
			}
			_, _ = io.Copy(io.Discard, resp.Body)
			resp.Body.Close()
		}
	})
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "reads/sec")
}

func BenchmarkAPIParallelMixed(b *testing.B) {
	bs := newBenchServer(b, apiSeedSize)

	b.ResetTimer()
	b.ReportAllocs()

	b.RunParallel(func(pb *testing.PB) {
		i := 0
		for pb.Next() {
			if i%5 == 0 {
				// 20% writes — create identity
				body, _ := json.Marshal(map[string]any{
					"identifier":   fmt.Sprintf("par-mixed-%s", id.New()),
					"display_name": "Parallel Mixed",
				})
				req, _ := http.NewRequest("POST", bs.ts.URL+"/v1/users", bytes.NewReader(body))
				req.Header.Set("Content-Type", "application/json")
				req.Header.Set("Authorization", "Bearer "+bs.token)
				resp, err := http.DefaultClient.Do(req)
				if err != nil {
					b.Fatalf("parallel create: %v", err)
				}
				_, _ = io.Copy(io.Discard, resp.Body)
				resp.Body.Close()
			} else {
				// 80% reads
				target := bs.ids[benchRandN(len(bs.ids))]
				req, _ := http.NewRequest("GET", bs.ts.URL+"/v1/users/"+target, nil)
				req.Header.Set("Authorization", "Bearer "+bs.token)
				resp, err := http.DefaultClient.Do(req)
				if err != nil {
					b.Fatalf("parallel get: %v", err)
				}
				_, _ = io.Copy(io.Discard, resp.Body)
				resp.Body.Close()
			}
			i++
		}
	})
	b.ReportMetric(float64(b.N)/b.Elapsed().Seconds(), "ops/sec")
}
