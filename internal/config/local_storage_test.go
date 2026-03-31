package config

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestResolveLocalStorage_DefaultsUseDataDir(t *testing.T) {
	cfg := Defaults()
	origDir, _ := os.Getwd()
	dir := t.TempDir()
	_ = os.Chdir(dir)
	defer func() { _ = os.Chdir(origDir) }()

	resolved, err := cfg.ResolveLocalStorage("")
	if err != nil {
		t.Fatalf("ResolveLocalStorage: %v", err)
	}

	resolvedDir := canonicalPath(t, dir)
	wantDB := filepath.Join(resolvedDir, "data", "zitadel.db")
	wantCache := filepath.Join(resolvedDir, "data", "zitadel-cache.db")
	if got := canonicalSQLiteURL(t, cfg.Database.URL); got != "sqlite://"+wantDB {
		t.Fatalf("database url = %q, want %q", got, "sqlite://"+wantDB)
	}
	if got := canonicalPath(t, cfg.Observability.CachePath); got != wantCache {
		t.Fatalf("cache path = %q, want %q", got, wantCache)
	}
	if got := canonicalPath(t, resolved.DatabasePath); got != wantDB {
		t.Fatalf("resolved database path = %q, want %q", got, wantDB)
	}
	if got := canonicalPath(t, resolved.CachePath); got != wantCache {
		t.Fatalf("resolved cache path = %q, want %q", got, wantCache)
	}
}

func TestResolveLocalStorage_ConfigDirRelative(t *testing.T) {
	cfgDir := t.TempDir()
	cfgPath := filepath.Join(cfgDir, "zitadel.toml")
	if err := os.WriteFile(cfgPath, []byte("[server]\nport = 8080\n"), 0o600); err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(cfgPath)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}

	_, err = cfg.ResolveLocalStorage(cfgPath)
	if err != nil {
		t.Fatalf("ResolveLocalStorage: %v", err)
	}

	resolvedDir := canonicalPath(t, cfgDir)
	wantDB := filepath.Join(resolvedDir, "data", "zitadel.db")
	wantCache := filepath.Join(resolvedDir, "data", "zitadel-cache.db")
	if got := canonicalSQLiteURL(t, cfg.Database.URL); got != "sqlite://"+wantDB {
		t.Fatalf("database url = %q, want %q", got, "sqlite://"+wantDB)
	}
	if got := canonicalPath(t, cfg.Observability.CachePath); got != wantCache {
		t.Fatalf("cache path = %q, want %q", got, wantCache)
	}
}

func TestResolveLocalStorage_ConfigDefaultsStillResolveRelative(t *testing.T) {
	cfgDir := t.TempDir()
	cfgPath := filepath.Join(cfgDir, "zitadel.toml")
	content := `
[database]
url = "sqlite://./data/zitadel.db"

[observability]
cache_path = "./data/zitadel-cache.db"
`
	if err := os.WriteFile(cfgPath, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(cfgPath)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}

	_, err = cfg.ResolveLocalStorage(cfgPath)
	if err != nil {
		t.Fatalf("ResolveLocalStorage: %v", err)
	}

	resolvedDir := canonicalPath(t, cfgDir)
	wantDB := filepath.Join(resolvedDir, "data", "zitadel.db")
	wantCache := filepath.Join(resolvedDir, "data", "zitadel-cache.db")
	if got := canonicalSQLiteURL(t, cfg.Database.URL); got != "sqlite://"+wantDB {
		t.Fatalf("database url = %q, want %q", got, "sqlite://"+wantDB)
	}
	if got := canonicalPath(t, cfg.Observability.CachePath); got != wantCache {
		t.Fatalf("cache path = %q, want %q", got, wantCache)
	}
}

func TestResolveLocalStorage_PreservesExplicitOverrides(t *testing.T) {
	cfgPath := filepath.Join(t.TempDir(), "zitadel.toml")
	content := `
[database]
url = "sqlite://./custom.db"

[observability]
cache_path = "./custom-cache.db"
`
	if err := os.WriteFile(cfgPath, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}

	cfg, err := Load(cfgPath)
	if err != nil {
		t.Fatalf("Load: %v", err)
	}
	if !cfg.DatabaseURLExplicit() {
		t.Fatal("expected database url to be marked explicit")
	}
	if !cfg.CachePathExplicit() {
		t.Fatal("expected cache path to be marked explicit")
	}

	_, err = cfg.ResolveLocalStorage(cfgPath)
	if err != nil {
		t.Fatalf("ResolveLocalStorage: %v", err)
	}

	wantDB := filepath.Join(canonicalPath(t, filepath.Dir(cfgPath)), "custom.db")
	wantCache := filepath.Join(canonicalPath(t, filepath.Dir(cfgPath)), "custom-cache.db")
	if got := canonicalSQLiteURL(t, cfg.Database.URL); got != "sqlite://"+wantDB {
		t.Fatalf("database url = %q, want %q", got, "sqlite://"+wantDB)
	}
	if got := canonicalPath(t, cfg.Observability.CachePath); got != wantCache {
		t.Fatalf("cache path = %q, want %q", got, wantCache)
	}
}

func TestResolveLocalStorage_AdoptsLegacyDatabase(t *testing.T) {
	cfg := Defaults()
	origDir, _ := os.Getwd()
	dir := t.TempDir()
	_ = os.Chdir(dir)
	defer func() { _ = os.Chdir(origDir) }()

	legacyDB := filepath.Join(dir, "zitadel.db")
	if err := os.WriteFile(legacyDB, []byte("legacy"), 0o600); err != nil {
		t.Fatal(err)
	}
	legacyCache := filepath.Join(dir, "zitadel-cache.db")
	if err := os.WriteFile(legacyCache, []byte("legacy-cache"), 0o600); err != nil {
		t.Fatal(err)
	}

	resolved, err := cfg.ResolveLocalStorage("")
	if err != nil {
		t.Fatalf("ResolveLocalStorage: %v", err)
	}

	if !resolved.LegacyDatabaseUsed {
		t.Fatal("expected legacy database to be adopted")
	}
	if !resolved.LegacyCacheUsed {
		t.Fatal("expected legacy cache to be adopted with legacy database")
	}
	wantLegacyDB := canonicalPath(t, legacyDB)
	wantLegacyCache := canonicalPath(t, legacyCache)
	if got := canonicalSQLiteURL(t, cfg.Database.URL); got != "sqlite://"+wantLegacyDB {
		t.Fatalf("database url = %q, want %q", got, "sqlite://"+wantLegacyDB)
	}
	if got := canonicalPath(t, cfg.Observability.CachePath); got != wantLegacyCache {
		t.Fatalf("cache path = %q, want %q", got, wantLegacyCache)
	}
}

func canonicalPath(t *testing.T, path string) string {
	t.Helper()
	resolved, err := filepath.EvalSymlinks(path)
	if err != nil {
		resolved = filepath.Clean(path)
	}
	if strings.HasPrefix(resolved, "/private/var/") {
		resolved = strings.TrimPrefix(resolved, "/private")
	}
	return resolved
}

func canonicalSQLiteURL(t *testing.T, url string) string {
	t.Helper()
	return "sqlite://" + canonicalPath(t, filepath.Clean(strings.TrimPrefix(url, "sqlite://")))
}

func TestResolveConfigRelativePath(t *testing.T) {
	cfgDir := t.TempDir()
	cfgPath := filepath.Join(cfgDir, "zitadel.toml")

	got, err := ResolveConfigRelativePath(cfgPath, "./seeds/frontend.yaml")
	if err != nil {
		t.Fatalf("ResolveConfigRelativePath: %v", err)
	}

	want := filepath.Join(canonicalPath(t, cfgDir), "seeds", "frontend.yaml")
	if got := canonicalPath(t, got); got != want {
		t.Fatalf("resolved path = %q, want %q", got, want)
	}
}
