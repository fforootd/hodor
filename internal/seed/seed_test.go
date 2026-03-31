package seed

import (
	"context"
	"os"
	"path/filepath"
	"testing"

	"github.com/zitadel/zitadel/internal/bootstrap"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/oidcop"
	providers "github.com/zitadel/zitadel/internal/provider"
)

func TestSubstituteEnvVars_Basic(t *testing.T) {
	os.Setenv("TEST_CLIENT_ID", "my-client-123")
	defer os.Unsetenv("TEST_CLIENT_ID")

	input := []byte(`client_id: ${TEST_CLIENT_ID}`)
	result := substituteEnvVars(input)

	if string(result) != "client_id: my-client-123" {
		t.Errorf("got %q", string(result))
	}
}

func TestSubstituteEnvVars_Default(t *testing.T) {
	os.Unsetenv("MISSING_VAR")

	input := []byte(`value: ${MISSING_VAR:-fallback_value}`)
	result := substituteEnvVars(input)

	if string(result) != "value: fallback_value" {
		t.Errorf("got %q", string(result))
	}
}

func TestSubstituteEnvVars_EnvOverridesDefault(t *testing.T) {
	os.Setenv("PRESENT_VAR", "real-value")
	defer os.Unsetenv("PRESENT_VAR")

	input := []byte(`value: ${PRESENT_VAR:-fallback}`)
	result := substituteEnvVars(input)

	if string(result) != "value: real-value" {
		t.Errorf("got %q", string(result))
	}
}

func TestSubstituteEnvVars_NoVars(t *testing.T) {
	input := []byte(`plain: text without vars`)
	result := substituteEnvVars(input)

	if string(result) != "plain: text without vars" {
		t.Errorf("got %q", string(result))
	}
}

func TestSubstituteEnvVars_MultipleVars(t *testing.T) {
	os.Setenv("VAR_A", "alpha")
	os.Setenv("VAR_B", "beta")
	defer os.Unsetenv("VAR_A")
	defer os.Unsetenv("VAR_B")

	input := []byte(`a: ${VAR_A}, b: ${VAR_B}`)
	result := substituteEnvVars(input)

	if string(result) != "a: alpha, b: beta" {
		t.Errorf("got %q", string(result))
	}
}

func TestLoadAndApply_FileNotFound(t *testing.T) {
	err := LoadAndApply(context.TODO(), nil, "/nonexistent/file.yaml")
	if err == nil {
		t.Error("expected error for missing file")
	}
}

func TestLoadFile_RejectsDuplicateUsers(t *testing.T) {
	path := filepath.Join(t.TempDir(), "dup.yaml")
	content := `
users:
  - identifier: jane@example.com
  - identifier: jane@example.com
`
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}

	_, err := LoadFile(path)
	if err == nil {
		t.Fatal("expected validation error")
	}
}

func TestLoadFile_RejectsDuplicateApps(t *testing.T) {
	path := filepath.Join(t.TempDir(), "dup-apps.yaml")
	content := `
apps:
  - name: First app
    client_id: duplicate-client
  - name: Second app
    client_id: duplicate-client
`
	if err := os.WriteFile(path, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}

	_, err := LoadFile(path)
	if err == nil {
		t.Fatal("expected validation error")
	}
}

func TestLoadAndApply_Idempotent(t *testing.T) {
	dir := t.TempDir()
	db, err := database.Open("sqlite://" + filepath.Join(dir, "zitadel.db"))
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}

	seedPath := filepath.Join(dir, "seed.yaml")
	content := `
users:
  - identifier: admin
    display_name: Admin
    password: admin123
    on_conflict: update
    pats:
      - name: dev-admin-token
        token: zitadel-dev-pat
        scopes: [admin]
`
	if err := os.WriteFile(seedPath, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}

	if err := bootstrap.EnsureAdmin(context.Background(), db, seedPath); err != nil {
		t.Fatalf("bootstrap: %v", err)
	}
	if err := LoadAndApply(context.Background(), db.SQL(), seedPath); err != nil {
		t.Fatalf("first apply: %v", err)
	}
	if err := LoadAndApply(context.Background(), db.SQL(), seedPath); err != nil {
		t.Fatalf("second apply: %v", err)
	}

	var users int
	if err := db.SQL().QueryRow(`SELECT COUNT(*) FROM users WHERE identifier = 'admin'`).Scan(&users); err != nil {
		t.Fatalf("count users: %v", err)
	}
	if users != 1 {
		t.Fatalf("expected 1 admin user, got %d", users)
	}

	var pats int
	if err := db.SQL().QueryRow(`SELECT COUNT(*) FROM tokens WHERE name = 'dev-admin-token' AND type = 'pat'`).Scan(&pats); err != nil {
		t.Fatalf("count pats: %v", err)
	}
	if pats != 1 {
		t.Fatalf("expected 1 PAT, got %d", pats)
	}
}

func TestLoadAndApply_SeedsAppsWithHashedClientSecrets(t *testing.T) {
	dir := t.TempDir()
	db, err := database.Open("sqlite://" + filepath.Join(dir, "zitadel.db"))
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}

	seedPath := filepath.Join(dir, "seed-app.yaml")
	content := `
apps:
  - name: E2E OIDC App
    client_id: e2e-oidc-client
    client_secret: e2e-oidc-secret
    app_type: web
    redirect_uris:
      - http://127.0.0.1:9876/callback
    post_logout_redirect_uris:
      - http://127.0.0.1:9876/logout
    grant_types: [authorization_code]
    response_types: [code]
    on_conflict: update
`
	if err := os.WriteFile(seedPath, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}

	if err := bootstrap.EnsureAdmin(context.Background(), db, ""); err != nil {
		t.Fatalf("bootstrap: %v", err)
	}
	if err := LoadAndApply(context.Background(), db.SQL(), seedPath); err != nil {
		t.Fatalf("first apply: %v", err)
	}
	if err := LoadAndApply(context.Background(), db.SQL(), seedPath); err != nil {
		t.Fatalf("second apply: %v", err)
	}

	var clientSecret string
	if err := db.SQL().QueryRow(`SELECT client_secret FROM apps WHERE client_id = 'e2e-oidc-client'`).Scan(&clientSecret); err != nil {
		t.Fatalf("load client secret: %v", err)
	}
	if clientSecret == "" || clientSecret == "e2e-oidc-secret" {
		t.Fatalf("expected stored client secret to be hashed, got %q", clientSecret)
	}

	if err := oidcop.NewStorage(db, nil).AuthorizeClientIDSecret(context.Background(), "e2e-oidc-client", "e2e-oidc-secret"); err != nil {
		t.Fatalf("AuthorizeClientIDSecret() error = %v", err)
	}

	var apps int
	if err := db.SQL().QueryRow(`SELECT COUNT(*) FROM apps WHERE client_id = 'e2e-oidc-client'`).Scan(&apps); err != nil {
		t.Fatalf("count apps: %v", err)
	}
	if apps != 1 {
		t.Fatalf("expected 1 seeded app, got %d", apps)
	}
}

func TestLoadAndApply_SeedsProvidersWithCanonicalConnectionConfig(t *testing.T) {
	dir := t.TempDir()
	db, err := database.Open("sqlite://" + filepath.Join(dir, "zitadel.db"))
	if err != nil {
		t.Fatalf("open db: %v", err)
	}
	defer db.Close()

	if err := database.Migrate(db); err != nil {
		t.Fatalf("migrate: %v", err)
	}

	seedPath := filepath.Join(dir, "seed-provider.yaml")
	content := `
providers:
  - id: prov_seed_test
    name: Seeded Mock OIDC
    protocol: oidc
    template: custom
    config:
      issuer: http://127.0.0.1:19998
      client_id: mock-client-id
      client_secret: mock-client-secret
      scopes: openid email profile
`
	if err := os.WriteFile(seedPath, []byte(content), 0o600); err != nil {
		t.Fatal(err)
	}

	if err := bootstrap.EnsureAdmin(context.Background(), db, ""); err != nil {
		t.Fatalf("bootstrap: %v", err)
	}
	if err := LoadAndApply(context.Background(), db.SQL(), seedPath); err != nil {
		t.Fatalf("apply seed: %v", err)
	}

	repo := providers.NewRepository(db.SQL())
	prov, err := repo.Get(context.Background(), "prov_seed_test")
	if err != nil {
		t.Fatalf("load provider: %v", err)
	}

	if got, want := prov.Connection["issuer"], "http://127.0.0.1:19998"; got != want {
		t.Fatalf("provider issuer = %v, want %v", got, want)
	}
	if got, want := prov.Connection["client_id"], "mock-client-id"; got != want {
		t.Fatalf("provider client_id = %v, want %v", got, want)
	}
	if got, want := prov.CatalogRef.TemplateID, "custom"; got != want {
		t.Fatalf("provider template = %v, want %v", got, want)
	}
}
