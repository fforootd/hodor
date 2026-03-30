package config

import (
	"os"
	"testing"
)

func TestDefaults(t *testing.T) {
	cfg := Defaults()

	if cfg.Server.Port != 8080 {
		t.Errorf("default port = %d, want 8080", cfg.Server.Port)
	}
	if cfg.Database.URL != DefaultDatabaseURL {
		t.Errorf("default db url = %q, want %q", cfg.Database.URL, DefaultDatabaseURL)
	}
	if cfg.Observability.LogLevel != "info" {
		t.Errorf("default log level = %q, want info", cfg.Observability.LogLevel)
	}
	if cfg.Observability.CachePath != DefaultCachePath {
		t.Errorf("default cache path = %q, want %q", cfg.Observability.CachePath, DefaultCachePath)
	}
	if cfg.Workers.NotificationWorkers != 1 {
		t.Errorf("default notification workers = %d, want 1", cfg.Workers.NotificationWorkers)
	}
}

func TestLoadEmpty(t *testing.T) {
	cfg, err := Load("")
	if err != nil {
		t.Fatalf("Load(\"\") error: %v", err)
	}
	if cfg.Server.Port != 8080 {
		t.Errorf("port = %d, want 8080", cfg.Server.Port)
	}
}

func TestEnvOverride(t *testing.T) {
	t.Setenv("ZITADEL_PORT", "9090")
	t.Setenv("ZITADEL_DATABASE_URL", "postgres://localhost/test")
	t.Setenv("ZITADEL_COOKIE_SECRETS", "alpha,beta")

	cfg, err := Load("")
	if err != nil {
		t.Fatalf("Load error: %v", err)
	}
	if cfg.Server.Port != 9090 {
		t.Errorf("port = %d, want 9090", cfg.Server.Port)
	}
	if cfg.Database.URL != "postgres://localhost/test" {
		t.Errorf("db url = %q, want postgres://localhost/test", cfg.Database.URL)
	}
	if got, want := cfg.Server.CookieSecrets, []string{"alpha", "beta"}; len(got) != len(want) || got[0] != want[0] || got[1] != want[1] {
		t.Errorf("cookie secrets = %#v, want %#v", got, want)
	}
}

func TestLoadTOML(t *testing.T) {
	content := `
[server]
port = 3333
external_domain = "auth.example.com"

[database]
url = "postgres://user:pass@db:5432/zitadel"
`
	f, err := os.CreateTemp(t.TempDir(), "zitadel-*.toml")
	if err != nil {
		t.Fatal(err)
	}
	if _, err := f.WriteString(content); err != nil {
		t.Fatal(err)
	}
	f.Close()

	cfg, err := Load(f.Name())
	if err != nil {
		t.Fatalf("Load(%q) error: %v", f.Name(), err)
	}
	if cfg.Server.Port != 3333 {
		t.Errorf("port = %d, want 3333", cfg.Server.Port)
	}
	if cfg.Server.ExternalDomain != "auth.example.com" {
		t.Errorf("domain = %q, want auth.example.com", cfg.Server.ExternalDomain)
	}
}
