package ratelimit

import (
	"context"
	"net"
	"net/http"
	"net/http/httptest"
	"testing"
	"time"

	"database/sql"

	_ "modernc.org/sqlite"
)

func TestMemoryStore_Allow(t *testing.T) {
	store := NewMemoryStore(time.Hour) // long GC interval for tests
	defer store.Stop()
	ctx := context.Background()

	// Allow 5 requests per minute, burst of 5.
	for i := 0; i < 5; i++ {
		d, err := store.Allow(ctx, "test", 5, 5, time.Minute)
		if err != nil {
			t.Fatal(err)
		}
		if !d.Allowed {
			t.Errorf("request %d should be allowed", i+1)
		}
	}

	// 6th request should be denied.
	d, err := store.Allow(ctx, "test", 5, 5, time.Minute)
	if err != nil {
		t.Fatal(err)
	}
	if d.Allowed {
		t.Error("6th request should be denied")
	}
	if d.RetryAfter <= 0 {
		t.Error("RetryAfter should be positive")
	}
}

func TestMemoryStore_KeyIsolation(t *testing.T) {
	store := NewMemoryStore(time.Hour)
	defer store.Stop()
	ctx := context.Background()

	// Exhaust bucket for key "a".
	for i := 0; i < 3; i++ {
		if _, err := store.Allow(ctx, "a", 3, 3, time.Minute); err != nil {
			t.Fatal(err)
		}
	}

	// Key "b" should still have capacity.
	d, _ := store.Allow(ctx, "b", 3, 3, time.Minute)
	if !d.Allowed {
		t.Error("key 'b' should be allowed (isolated from 'a')")
	}
}

func TestMemoryStore_Refill(t *testing.T) {
	store := NewMemoryStore(time.Hour)
	defer store.Stop()
	ctx := context.Background()

	// Use a high rate: 6000 per minute = 100 per second.
	// Exhaust the burst.
	for i := 0; i < 5; i++ {
		if _, err := store.Allow(ctx, "refill", 6000, 5, time.Minute); err != nil {
			t.Fatal(err)
		}
	}

	// Should be denied immediately.
	d, _ := store.Allow(ctx, "refill", 6000, 5, time.Minute)
	if d.Allowed {
		t.Error("should be denied after burst exhaustion")
	}

	// Wait 100ms — at 100/s rate, ~10 tokens should refill.
	time.Sleep(100 * time.Millisecond)
	d, _ = store.Allow(ctx, "refill", 6000, 5, time.Minute)
	if !d.Allowed {
		t.Error("should be allowed after refill")
	}
}

func TestMemoryStore_Sweep(t *testing.T) {
	store := NewMemoryStore(time.Hour) // manual sweep
	defer store.Stop()
	ctx := context.Background()

	if _, err := store.Allow(ctx, "sweep-test", 10, 10, 1*time.Millisecond); err != nil {
		t.Fatal(err)
	}

	if store.Len() != 1 {
		t.Fatalf("expected 1 bucket, got %d", store.Len())
	}

	// Wait for expiry (2× window = 2ms).
	time.Sleep(10 * time.Millisecond)
	store.sweep()

	if store.Len() != 0 {
		t.Errorf("expected 0 buckets after sweep, got %d", store.Len())
	}
}

func TestIsExempt(t *testing.T) {
	tests := []struct {
		path string
		want bool
	}{
		{"/healthz", true},
		{"/readyz", true},
		{"/v1/users", false},
		{"/", false},
	}
	for _, tt := range tests {
		if got := IsExempt(tt.path); got != tt.want {
			t.Errorf("IsExempt(%q) = %v, want %v", tt.path, got, tt.want)
		}
	}
}

func TestIsWhitelisted(t *testing.T) {
	cfg := DefaultConfig()
	// No whitelist = not whitelisted.
	if isWhitelisted("192.168.1.1", cfg.WhitelistIPs) {
		t.Error("should not be whitelisted with empty list")
	}
}

func TestFormatRetryAfter(t *testing.T) {
	tests := []struct {
		d    time.Duration
		want string
	}{
		{5 * time.Second, "5"},
		{500 * time.Millisecond, "1"}, // rounds up to 1
		{0, "1"},
	}
	for _, tt := range tests {
		if got := FormatRetryAfter(tt.d); got != tt.want {
			t.Errorf("FormatRetryAfter(%v) = %q, want %q", tt.d, got, tt.want)
		}
	}
}

// testClientIP is a simple ClientIPFunc for tests.
func testClientIP(r *http.Request) string {
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return r.RemoteAddr
	}
	return host
}

func TestMiddleware_ExemptPaths(t *testing.T) {
	store := NewMemoryStore(time.Hour)
	defer store.Stop()
	db := testDB(t)
	limiter := New(store, db)

	handler := Middleware(limiter, testClientIP)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest("GET", "/healthz", nil)
	req.RemoteAddr = "127.0.0.1:1234"
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("healthz should be exempt, got %d", rec.Code)
	}
}

func TestMiddleware_RateLimits(t *testing.T) {
	store := NewMemoryStore(time.Hour)
	defer store.Stop()
	db := testDB(t)

	// Set instance-level rate limit: 3 RPM, burst 3.
	_, err := db.Exec(`INSERT INTO settings (id, type, scope, scope_id, data) VALUES (?, ?, ?, ?, ?)`,
		"test-rl", "rate_limit", "instance", "",
		`{"requests_per_minute": 3, "burst": 3, "by_ip": true}`,
	)
	if err != nil {
		t.Fatal(err)
	}

	limiter := New(store, db)

	handler := Middleware(limiter, testClientIP)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	// Send 3 requests — all should pass.
	for i := 0; i < 3; i++ {
		req := httptest.NewRequest("GET", "/v1/users", nil)
		req.RemoteAddr = "10.0.0.1:1234"
		rec := httptest.NewRecorder()
		handler.ServeHTTP(rec, req)

		if rec.Code != http.StatusOK {
			t.Errorf("request %d: got %d, want 200", i+1, rec.Code)
		}
	}

	// 4th request should be rate limited.
	req := httptest.NewRequest("GET", "/v1/users", nil)
	req.RemoteAddr = "10.0.0.1:1234"
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusTooManyRequests {
		t.Errorf("4th request: got %d, want 429", rec.Code)
	}
	if rec.Header().Get("Retry-After") == "" {
		t.Error("missing Retry-After header")
	}
	if rec.Header().Get("X-Ratelimit-Limit") != "3" {
		t.Errorf("X-Ratelimit-Limit = %q, want 3", rec.Header().Get("X-Ratelimit-Limit"))
	}
}

func TestMiddleware_NoSettings_FailOpen(t *testing.T) {
	store := NewMemoryStore(time.Hour)
	defer store.Stop()
	db := testDB(t)
	limiter := New(store, db)

	handler := Middleware(limiter, testClientIP)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	// No rate_limit settings in DB → defaults apply (1000 RPM).
	req := httptest.NewRequest("GET", "/v1/users", nil)
	req.RemoteAddr = "10.0.0.1:1234"
	rec := httptest.NewRecorder()
	handler.ServeHTTP(rec, req)

	if rec.Code != http.StatusOK {
		t.Errorf("should pass with defaults, got %d", rec.Code)
	}
}

// --- Benchmarks ---

func BenchmarkMemoryStore_Allow(b *testing.B) {
	store := NewMemoryStore(time.Hour)
	defer store.Stop()
	ctx := context.Background()

	b.ResetTimer()
	b.RunParallel(func(pb *testing.PB) {
		i := 0
		for pb.Next() {
			// Use rotating keys to simulate different IPs.
			key := "bench:" + string(rune('A'+i%26))
			if _, err := store.Allow(ctx, key, 10000, 100, time.Minute); err != nil {
				b.Fatal(err)
			}
			i++
		}
	})
}

func BenchmarkMemoryStore_Allow_SingleKey(b *testing.B) {
	store := NewMemoryStore(time.Hour)
	defer store.Stop()
	ctx := context.Background()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := store.Allow(ctx, "single", 1000000, 1000000, time.Minute); err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkMiddleware(b *testing.B) {
	store := NewMemoryStore(time.Hour)
	defer store.Stop()

	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		b.Fatal(err)
	}
	defer db.Close()

	db.Exec(`
		CREATE TABLE settings (
			id TEXT PRIMARY KEY, type TEXT NOT NULL,
			scope TEXT NOT NULL DEFAULT 'instance', scope_id TEXT NOT NULL DEFAULT '',
			data TEXT NOT NULL DEFAULT '{}',
			created_at TEXT NOT NULL DEFAULT (datetime('now')),
			updated_at TEXT NOT NULL DEFAULT (datetime('now')),
			UNIQUE(type, scope, scope_id)
		)`)
	db.Exec(`
		CREATE TABLE schemas (
			id TEXT PRIMARY KEY, type TEXT NOT NULL,
			org_id TEXT DEFAULT '1', schema TEXT DEFAULT '{}',
			version INTEGER DEFAULT 1, is_default BOOLEAN DEFAULT false,
			visibility TEXT DEFAULT 'private', message TEXT DEFAULT '',
			created_by TEXT DEFAULT '', created_at TEXT DEFAULT (datetime('now'))
		)`)
	db.Exec(`INSERT INTO schemas (id, type, is_default) VALUES ('action_v1', 'action', true)`)
	db.Exec(`
		CREATE TABLE entities (
			id TEXT PRIMARY KEY, schema_id TEXT DEFAULT '',
			identifier TEXT NOT NULL DEFAULT '', org_id TEXT NOT NULL DEFAULT '',
			data TEXT NOT NULL DEFAULT '{}',
			created_at TEXT NOT NULL DEFAULT (datetime('now')),
			updated_at TEXT NOT NULL DEFAULT (datetime('now'))
		)`)

	// Set a high limit so we benchmark the hot path (allowed), not the deny path.
	db.Exec(`INSERT INTO settings (id, type, scope, scope_id, data) VALUES ('b', 'rate_limit', 'instance', '', '{"requests_per_minute": 1000000, "burst": 1000000}')`)

	limiter := New(store, db)
	handler := Middleware(limiter, testClientIP)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	b.ResetTimer()
	b.RunParallel(func(pb *testing.PB) {
		for pb.Next() {
			req := httptest.NewRequest("GET", "/v1/users", nil)
			req.RemoteAddr = "10.0.0.1:1234"
			rec := httptest.NewRecorder()
			handler.ServeHTTP(rec, req)
		}
	})
}

func BenchmarkMiddleware_Exempt(b *testing.B) {
	store := NewMemoryStore(time.Hour)
	defer store.Stop()

	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		b.Fatal(err)
	}
	defer db.Close()

	db.Exec(`CREATE TABLE settings (id TEXT PRIMARY KEY, type TEXT NOT NULL, scope TEXT NOT NULL DEFAULT 'instance', scope_id TEXT NOT NULL DEFAULT '', data TEXT NOT NULL DEFAULT '{}', created_at TEXT, updated_at TEXT, UNIQUE(type, scope, scope_id))`)
	db.Exec(`CREATE TABLE schemas (id TEXT PRIMARY KEY, type TEXT NOT NULL, org_id TEXT DEFAULT '1', schema TEXT DEFAULT '{}', version INTEGER DEFAULT 1, is_default BOOLEAN DEFAULT false, visibility TEXT DEFAULT 'private', message TEXT DEFAULT '', created_by TEXT DEFAULT '', created_at TEXT DEFAULT (datetime('now')))`)
	db.Exec(`INSERT INTO schemas (id, type, is_default) VALUES ('action_v1', 'action', true)`)
	db.Exec(`CREATE TABLE entities (id TEXT PRIMARY KEY, schema_id TEXT DEFAULT '', identifier TEXT NOT NULL DEFAULT '', org_id TEXT NOT NULL DEFAULT '', data TEXT NOT NULL DEFAULT '{}', created_at TEXT, updated_at TEXT)`)

	limiter := New(store, db)
	handler := Middleware(limiter, testClientIP)(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		req := httptest.NewRequest("GET", "/healthz", nil)
		req.RemoteAddr = "10.0.0.1:1234"
		rec := httptest.NewRecorder()
		handler.ServeHTTP(rec, req)
	}
}

func BenchmarkConfigResolve(b *testing.B) {
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		b.Fatal(err)
	}
	defer db.Close()

	db.Exec(`CREATE TABLE settings (id TEXT PRIMARY KEY, type TEXT NOT NULL, scope TEXT NOT NULL DEFAULT 'instance', scope_id TEXT NOT NULL DEFAULT '', data TEXT NOT NULL DEFAULT '{}', created_at TEXT, updated_at TEXT, UNIQUE(type, scope, scope_id))`)
	db.Exec(`INSERT INTO settings (id, type, scope, scope_id, data) VALUES ('i', 'rate_limit', 'instance', '', '{"requests_per_minute": 1000, "burst": 50}')`)
	db.Exec(`INSERT INTO settings (id, type, scope, scope_id, data) VALUES ('o', 'rate_limit', 'org', 'org1', '{"requests_per_minute": 200}')`)

	store := NewMemoryStore(time.Hour)
	defer store.Stop()
	limiter := New(store, db)
	ctx := context.Background()

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		if _, err := limiter.resolveConfig(ctx, "org1", ""); err != nil {
			b.Fatal(err)
		}
	}
}

// testDB creates an in-memory SQLite database with required tables for testing.
func testDB(t *testing.T) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}

	// Settings table.
	_, err = db.Exec(`
		CREATE TABLE settings (
			id         TEXT PRIMARY KEY,
			type       TEXT NOT NULL,
			scope      TEXT NOT NULL DEFAULT 'instance',
			scope_id   TEXT NOT NULL DEFAULT '',
			data       TEXT NOT NULL DEFAULT '{}',
			created_at TEXT NOT NULL DEFAULT (datetime('now')),
			updated_at TEXT NOT NULL DEFAULT (datetime('now')),
			UNIQUE(type, scope, scope_id)
		)`)
	if err != nil {
		t.Fatal(err)
	}

	// Schemas table (for entity type resolution).
	_, err = db.Exec(`
		CREATE TABLE schemas (
			id         TEXT PRIMARY KEY,
			type       TEXT NOT NULL,
			org_id     TEXT DEFAULT '1',
			schema     TEXT DEFAULT '{}',
			version    INTEGER DEFAULT 1,
			is_default BOOLEAN DEFAULT false,
			visibility TEXT DEFAULT 'private',
			message    TEXT DEFAULT '',
			created_by TEXT DEFAULT '',
			created_at TEXT DEFAULT (datetime('now'))
		)`)
	if err != nil {
		t.Fatal(err)
	}
	db.Exec(`INSERT INTO schemas (id, type, is_default) VALUES ('action_v1', 'action', true)`)

	// Entities table.
	_, err = db.Exec(`
		CREATE TABLE entities (
			id          TEXT PRIMARY KEY,
			schema_id   TEXT DEFAULT '',
			identifier  TEXT NOT NULL DEFAULT '',
			org_id      TEXT NOT NULL DEFAULT '',
			data        TEXT NOT NULL DEFAULT '{}',
			created_at  TEXT NOT NULL DEFAULT (datetime('now')),
			updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
		)`)
	if err != nil {
		t.Fatal(err)
	}

	t.Cleanup(func() { db.Close() })
	return db
}
