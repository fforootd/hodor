// Package testutil provides shared helpers for integration tests.
package testutil

import (
	"bytes"
	"crypto/rand"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"path/filepath"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/bootstrap"
	"github.com/zitadel/zitadel/internal/config"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/eventbus"
	"github.com/zitadel/zitadel/internal/server"
)

// TestServer wraps a full Zitadel server for integration testing.
type TestServer struct {
	Server   *httptest.Server
	DB       *database.DB
	Config   *config.Config
	Bus      *eventbus.Bus
	AdminPwd string // bootstrap admin password
	t        *testing.T
}

// NewTestServer creates a full Zitadel server backed by SQLite in a temp dir.
// The server is automatically cleaned up when the test finishes.
func NewTestServer(t *testing.T) *TestServer {
	t.Helper()

	dir := t.TempDir()
	dbPath := filepath.Join(dir, "test.db")
	t.Logf("TestServer dbPath: %s", dbPath)

	cfg := config.Defaults()
	cfg.Database.URL = "sqlite://" + dbPath

	db, err := database.Open(cfg.Database.URL)
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	if err := database.EnsureSchema(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}

	// Bootstrap creates admin user and captures the password.
	if err := bootstrap.EnsureAdmin(t.Context(), db, ""); err != nil {
		t.Fatalf("bootstrap: %v", err)
	}

	// Read admin password from the identity.
	var adminPwd string
	db.SQL().QueryRow(`SELECT password_hash FROM passwords LIMIT 1`).Scan(&adminPwd)

	bus := eventbus.New()

	srv := server.New(cfg, db, bus)
	handler := srv.Handler()
	ts := httptest.NewServer(handler)

	t.Cleanup(func() {
		ts.Close()
		// Checkpoint WAL before closing to prevent dangling -wal/-shm files
		// that cause t.TempDir() cleanup failures.
		db.SQL().Exec("PRAGMA wal_checkpoint(TRUNCATE)")
		db.Close()
	})

	return &TestServer{
		Server:   ts,
		DB:       db,
		Config:   cfg,
		Bus:      bus,
		AdminPwd: adminPwd,
		t:        t,
	}
}

// URL returns the test server base URL.
func (ts *TestServer) URL() string {
	return ts.Server.URL
}

// LoginAdmin returns a session token for the bootstrap admin identity.
func (ts *TestServer) LoginAdmin() string {
	ts.t.Helper()
	var adminID int64
	err := ts.DB.SQL().QueryRow(`SELECT id FROM entities WHERE identifier = 'admin'`).Scan(&adminID)
	if err != nil {
		ts.t.Fatalf("find admin: %v", err)
	}
	return ts.CreateSession(adminID)
}

// CreateIdentity inserts a new identity into the database directly for testing.
func (ts *TestServer) CreateIdentity(identifier, displayName string) int64 {
	ts.t.Helper()

	var identityID int64
	err := ts.DB.SQL().QueryRow(`SELECT id FROM entities WHERE identifier = ?`, identifier).Scan(&identityID)
	if err == nil {
		return identityID
	}

	identityID = time.Now().UnixNano() + int64(ts.t.Name()[0])

	now := time.Now().UTC().Format(time.RFC3339)
	_, err = ts.DB.SQL().Exec(
		`INSERT INTO entities (id, org_id, identifier, display_name, state, profile, metadata, created_at, updated_at)
		 VALUES (?, 1, ?, ?, 'active', '{}', '{}', ?, ?)`,
		identityID, identifier, displayName, now, now)
	if err != nil {
		ts.t.Fatalf("insert identity: %v", err)
	}
	return identityID
}

// CreateSession inserts a valid session directly into the DB and returns the raw token.
// The token is prefixed with zit_ses_ and inserted into both sessions and tokens tables.
func (ts *TestServer) CreateSession(identityID int64) string {
	ts.t.Helper()

	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		ts.t.Fatalf("rand.Read: %v", err)
	}
	raw := "zit_ses_" + hex.EncodeToString(b)
	h := sha256.Sum256([]byte(raw))
	hash := hex.EncodeToString(h[:])

	sessionID := time.Now().UnixNano()
	tokenID := sessionID + 1
	now := time.Now().UTC().Format(time.RFC3339)
	expiresAt := time.Now().UTC().Add(24 * time.Hour).Format(time.RFC3339)

	_, err := ts.DB.SQL().Exec(
		`INSERT INTO sessions (id, entity_id, org_id, token_hash, user_agent, ip_address, metadata, created_at, expires_at)
		 VALUES (?, ?, 1, ?, 'testutil', '127.0.0.1', '{}', ?, ?)`,
		sessionID, identityID, hash, now, expiresAt)
	if err != nil {
		ts.t.Fatalf("insert session: %v", err)
	}

	// Also insert into the unified tokens table.
	_, err = ts.DB.SQL().Exec(
		`INSERT INTO tokens (id, type, token_hash, entity_id, session_id, scopes, expires_at, created_at)
		 VALUES (?, 'session', ?, ?, ?, '[]', ?, ?)`,
		tokenID, hash, identityID, sessionID, expiresAt, now)
	if err != nil {
		ts.t.Fatalf("insert token: %v", err)
	}

	return raw
}

// Get makes a GET request and returns the decoded JSON response.
func (ts *TestServer) Get(path string) map[string]any {
	ts.t.Helper()
	resp, err := http.Get(ts.URL() + path)
	if err != nil {
		ts.t.Fatalf("GET %s: %v", path, err)
	}
	defer resp.Body.Close()
	return ts.decodeJSON(resp.Body)
}

// GetRaw makes a GET request and returns status code + body.
func (ts *TestServer) GetRaw(path string) (int, map[string]any) {
	ts.t.Helper()
	resp, err := http.Get(ts.URL() + path)
	if err != nil {
		ts.t.Fatalf("GET %s: %v", path, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode, ts.decodeJSON(resp.Body)
}

// PostJSON makes a POST request with a JSON body and returns the decoded response.
func (ts *TestServer) PostJSON(path string, body map[string]any) map[string]any {
	ts.t.Helper()
	b, _ := json.Marshal(body)
	resp, err := http.Post(ts.URL()+path, "application/json", bytes.NewReader(b))
	if err != nil {
		ts.t.Fatalf("POST %s: %v", path, err)
	}
	defer resp.Body.Close()
	return ts.decodeJSON(resp.Body)
}

// PostJSONWithCookie makes a POST with a session cookie.
func (ts *TestServer) PostJSONWithCookie(path string, body map[string]any, token string) (int, map[string]any) {
	ts.t.Helper()
	b, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", ts.URL()+path, bytes.NewReader(b))
	req.Header.Set("Content-Type", "application/json")
	req.AddCookie(&http.Cookie{Name: "__zitadel_session", Value: token})
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		ts.t.Fatalf("POST %s: %v", path, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode, ts.decodeJSON(resp.Body)
}

// GetWithCookie makes a GET with a session cookie.
func (ts *TestServer) GetWithCookie(path string, token string) (int, map[string]any) {
	ts.t.Helper()
	req, _ := http.NewRequest("GET", ts.URL()+path, nil)
	req.AddCookie(&http.Cookie{Name: "__zitadel_session", Value: token})
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		ts.t.Fatalf("GET %s: %v", path, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode, ts.decodeJSON(resp.Body)
}

// PatchJSONWithCookie makes a PATCH with a session cookie.
func (ts *TestServer) PatchJSONWithCookie(path string, body map[string]any, token string) (int, map[string]any) {
	ts.t.Helper()
	b, _ := json.Marshal(body)
	req, _ := http.NewRequest("PATCH", ts.URL()+path, bytes.NewReader(b))
	req.Header.Set("Content-Type", "application/json")
	req.AddCookie(&http.Cookie{Name: "__zitadel_session", Value: token})
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		ts.t.Fatalf("PATCH %s: %v", path, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode, ts.decodeJSON(resp.Body)
}

// DeleteWithCookie makes a DELETE with a session cookie.
func (ts *TestServer) DeleteWithCookie(path string, token string) (int, map[string]any) {
	ts.t.Helper()
	req, _ := http.NewRequest("DELETE", ts.URL()+path, nil)
	req.AddCookie(&http.Cookie{Name: "__zitadel_session", Value: token})
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		ts.t.Fatalf("DELETE %s: %v", path, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode, ts.decodeJSON(resp.Body)
}

func (ts *TestServer) decodeJSON(r io.Reader) map[string]any {
	var result map[string]any
	json.NewDecoder(r).Decode(&result)
	return result
}

// GetWithBearer makes a GET with an Authorization: Bearer header.
func (ts *TestServer) GetWithBearer(path string, token string) (int, map[string]any) {
	ts.t.Helper()
	req, _ := http.NewRequest("GET", ts.URL()+path, nil)
	req.Header.Set("Authorization", "Bearer "+token)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		ts.t.Fatalf("GET %s: %v", path, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode, ts.decodeJSON(resp.Body)
}

// PostJSONWithBearer makes a POST with a Bearer token.
func (ts *TestServer) PostJSONWithBearer(path string, body map[string]any, token string) (int, map[string]any) {
	ts.t.Helper()
	b, _ := json.Marshal(body)
	req, _ := http.NewRequest("POST", ts.URL()+path, bytes.NewReader(b))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+token)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		ts.t.Fatalf("POST %s: %v", path, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode, ts.decodeJSON(resp.Body)
}

// PatchJSONWithBearer makes a PATCH with a Bearer token.
func (ts *TestServer) PatchJSONWithBearer(path string, body map[string]any, token string) (int, map[string]any) {
	ts.t.Helper()
	b, _ := json.Marshal(body)
	req, _ := http.NewRequest("PATCH", ts.URL()+path, bytes.NewReader(b))
	req.Header.Set("Content-Type", "application/json")
	req.Header.Set("Authorization", "Bearer "+token)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		ts.t.Fatalf("PATCH %s: %v", path, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode, ts.decodeJSON(resp.Body)
}

// DeleteWithBearer makes a DELETE with a Bearer token.
func (ts *TestServer) DeleteWithBearer(path string, token string) (int, map[string]any) {
	ts.t.Helper()
	req, _ := http.NewRequest("DELETE", ts.URL()+path, nil)
	req.Header.Set("Authorization", "Bearer "+token)
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		ts.t.Fatalf("DELETE %s: %v", path, err)
	}
	defer resp.Body.Close()
	return resp.StatusCode, ts.decodeJSON(resp.Body)
}
