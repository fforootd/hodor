package tenantaudit

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestRequestScopedStorageUsesScopedDB(t *testing.T) {
	root := repoRoot(t)
	files := []string{
		"internal/auth/password.go",
		"internal/events/store.go",
		"internal/loginflow/resolver.go",
		"internal/loginflow/store.go",
		"internal/oidcop/storage_auth_request.go",
		"internal/oidcop/storage_client.go",
		"internal/oidcop/storage_tokens.go",
		"internal/oidcop/storage_userinfo.go",
		"internal/session/store.go",
	}

	for _, rel := range files {
		content := readAuditFile(t, filepath.Join(root, rel))
		if strings.Contains(content, ".db.SQL().") {
			t.Fatalf("%s must use ScopedDB/ScopedTx instead of raw db.SQL()", rel)
		}
	}
}

func TestAPIHandlersDoNotOwnStorageSQL(t *testing.T) {
	root := repoRoot(t)
	files := []string{
		"internal/api/event.go",
		"internal/api/login_flow_assets.go",
		"internal/api/login_flow_crud.go",
		"internal/api/login_flow_resolution.go",
		"internal/api/session_create.go",
		"internal/api/session_read.go",
	}

	for _, rel := range files {
		content := readAuditFile(t, filepath.Join(root, rel))
		for _, pattern := range []string{"SELECT ", "INSERT INTO", "UPDATE ", "DELETE FROM"} {
			if strings.Contains(content, pattern) {
				t.Fatalf("%s still owns storage SQL pattern %q", rel, pattern)
			}
		}
	}
}

func TestBackendAgnosticStorageAvoidsSQLiteOnlySQL(t *testing.T) {
	root := repoRoot(t)
	checks := map[string][]string{
		"internal/api/token.go":                   {"datetime('now'", " GLOB ", "? GLOB"},
		"internal/crypto/store.go":                {"INSERT OR REPLACE", "datetime('now'", "json_extract("},
		"internal/jobs/gc.go":                     {"datetime('now'", " GLOB ", "? GLOB"},
		"internal/oidcop/storage_auth_request.go": {"datetime('now'"},
	}

	for rel, forbidden := range checks {
		content := readAuditFile(t, filepath.Join(root, rel))
		for _, pattern := range forbidden {
			if strings.Contains(content, pattern) {
				t.Fatalf("%s contains backend-specific SQL pattern %q", rel, pattern)
			}
		}
	}
}

func readAuditFile(t *testing.T, path string) string {
	t.Helper()
	content, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	return string(content)
}
