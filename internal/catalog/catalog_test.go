package catalog

import (
	"context"
	"database/sql"
	"encoding/json"
	"testing"

	_ "modernc.org/sqlite"

	"github.com/zitadel/zitadel/internal/config"
)

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

func setupTestDB(t *testing.T) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() { db.Close() })

	db.Exec(`CREATE TABLE schemas (
		id TEXT PRIMARY KEY,
		type TEXT NOT NULL,
		org_id TEXT NOT NULL DEFAULT '1',
		schema TEXT NOT NULL DEFAULT '{}',
		version INTEGER DEFAULT 1,
		is_default BOOLEAN DEFAULT false,
		visibility TEXT NOT NULL DEFAULT 'private',
		message TEXT DEFAULT '',
		created_by TEXT DEFAULT '',
		created_at TEXT NOT NULL DEFAULT (datetime('now'))
	)`)
	db.Exec(`CREATE TABLE users (
		id            TEXT PRIMARY KEY,
		org_id        TEXT NOT NULL DEFAULT '1',
		identifier    TEXT NOT NULL DEFAULT '',
		display_name  TEXT DEFAULT '',
		user_type     TEXT NOT NULL DEFAULT 'human',
		state         TEXT NOT NULL DEFAULT 'active',
		schema_id     TEXT DEFAULT '',
		metadata      TEXT DEFAULT '{}',
		created_at    TEXT NOT NULL DEFAULT (datetime('now')),
		updated_at    TEXT NOT NULL DEFAULT (datetime('now')),
		UNIQUE(org_id, identifier)
	)`)
	db.Exec(`CREATE TABLE actions (
		id           TEXT PRIMARY KEY,
		org_id       TEXT NOT NULL DEFAULT '1',
		name         TEXT NOT NULL DEFAULT '',
		hook         TEXT NOT NULL DEFAULT 'on_event',
		action_type  TEXT NOT NULL DEFAULT 'expr',
		trigger_expr TEXT DEFAULT 'true',
		config       TEXT NOT NULL DEFAULT '{}',
		priority     INTEGER DEFAULT 0,
		enabled      BOOLEAN DEFAULT 1,
		fail_open    BOOLEAN DEFAULT 0,
		timeout_ms   INTEGER DEFAULT 5000,
		schema_id    TEXT DEFAULT '',
		metadata     TEXT DEFAULT '{}',
		created_at   TEXT NOT NULL DEFAULT (datetime('now')),
		updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
	)`)
	db.Exec(`CREATE TABLE login_flows (
		id         TEXT PRIMARY KEY,
		org_id     TEXT NOT NULL DEFAULT '1',
		name       TEXT NOT NULL DEFAULT '',
		preset     TEXT DEFAULT 'identifier_first',
		steps      TEXT NOT NULL DEFAULT '[]',
		config     TEXT NOT NULL DEFAULT '{}',
		is_default BOOLEAN DEFAULT 0,
		enabled    BOOLEAN DEFAULT 1,
		schema_id  TEXT DEFAULT '',
		metadata   TEXT DEFAULT '{}',
		created_at TEXT NOT NULL DEFAULT (datetime('now')),
		updated_at TEXT NOT NULL DEFAULT (datetime('now'))
	)`)
	db.Exec(`CREATE TABLE providers (
		id              TEXT PRIMARY KEY,
		org_id          TEXT NOT NULL DEFAULT '1',
		name            TEXT NOT NULL DEFAULT '',
		protocol        TEXT NOT NULL DEFAULT 'oidc',
		template        TEXT NOT NULL DEFAULT 'custom',
		config          TEXT NOT NULL DEFAULT '{}',
		claim_overrides TEXT NOT NULL DEFAULT '{}',
		auto_register   BOOLEAN NOT NULL DEFAULT 1,
		enabled         BOOLEAN NOT NULL DEFAULT 1,
		display_order   INTEGER NOT NULL DEFAULT 0,
		schema_id       TEXT DEFAULT '',
		metadata        TEXT DEFAULT '{}',
		created_at      TEXT NOT NULL DEFAULT (datetime('now')),
		updated_at      TEXT NOT NULL DEFAULT (datetime('now')),
		UNIQUE(org_id, name)
	)`)
	db.Exec(`CREATE TABLE cache (
		namespace TEXT NOT NULL DEFAULT 'default',
		key TEXT NOT NULL,
		data TEXT NOT NULL,
		expires_at TEXT,
		fetched_at TEXT NOT NULL DEFAULT (datetime('now')),
		PRIMARY KEY (namespace, key)
	)`)

	// Seed schemas for common types.
	for _, st := range []string{"action", "provider", "authorization", "login_flow", "fga_model", "human_user"} {
		db.Exec(`INSERT INTO schemas (id, type, is_default) VALUES (?, ?, true)`, st+"_v1", st)
	}

	return db
}

func seedTestEntities(t *testing.T, db *sql.DB) {
	t.Helper()
	// Seed human_user entities for upgrade preview tests.
	users := []struct {
		id, name string
		data     map[string]any
	}{
		{"usr_1", "Alice Smith", map[string]any{
			"display_name": "Alice Smith",
			"email":        "alice@example.com",
			"phone":        "+1234567890",
			"locale":       "en-US",
		}},
		{"usr_2", "Bob Jones", map[string]any{
			"display_name": "Bob Jones",
			"email":        "bob@example.com",
			// no phone — will break if phone becomes required
		}},
		{"usr_3", "Charlie Brown", map[string]any{
			"display_name": "Charlie Brown",
			"email":        "charlie@example.com",
			"phone":        "",
			"locale":       "de-DE",
		}},
		{"usr_4", "Dana White", map[string]any{
			"display_name": "Dana White",
			"email":        123, // wrong type — will break if email must be string
			"phone":        "+9876543210",
		}},
		{"usr_5", "Eve Adams", map[string]any{
			"display_name": "Eve Adams",
			"email":        "eve@example.com",
			"phone":        "+1111111111",
			"locale":       "fr-FR",
			"timezone":     "Europe/Paris",
		}},
	}

	for _, u := range users {
		dataJSON, _ := json.Marshal(u.data)
		db.Exec(`INSERT INTO users (id, schema_id, identifier, display_name, metadata, org_id, created_at, updated_at)
			VALUES (?, 'human_user_v1', ?, ?, ?, '1', datetime('now'), datetime('now'))`,
			u.id, u.name, u.name, string(dataJSON))
	}
}

// ---------------------------------------------------------------------------
// Embedded catalog tests
// ---------------------------------------------------------------------------

func TestLoadEmbeddedIndex(t *testing.T) {
	idx, err := loadEmbeddedIndex()
	if err != nil {
		t.Fatalf("loadEmbeddedIndex: %v", err)
	}

	if idx.Version != "1.0" {
		t.Errorf("version = %q, want %q", idx.Version, "1.0")
	}

	if len(idx.Templates) != 11 {
		t.Errorf("template count = %d, want 11", len(idx.Templates))
	}

	// Verify all sources are "embedded".
	for _, tpl := range idx.Templates {
		if tpl.Source != "embedded" {
			t.Errorf("template %q source = %q, want %q", tpl.ID, tpl.Source, "embedded")
		}
	}
}

func TestNew_LoadsEmbedded(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	if svc.EmbeddedCount() != 11 {
		t.Errorf("EmbeddedCount = %d, want 11", svc.EmbeddedCount())
	}
}

// ---------------------------------------------------------------------------
// List tests
// ---------------------------------------------------------------------------

func TestService_List_NoFilter(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	all := svc.List("", "")
	if len(all) != 11 {
		t.Errorf("List() count = %d, want 11", len(all))
	}
}

func TestService_List_ByType(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	tests := []struct {
		typeFilter string
		want       int
	}{
		{"action", 3},
		{"provider", 5},
		{"authorization", 1},
		{"login_flow", 2},
	}

	for _, tc := range tests {
		got := svc.List(tc.typeFilter, "")
		if len(got) != tc.want {
			t.Errorf("List(%q) count = %d, want %d", tc.typeFilter, len(got), tc.want)
		}
	}
}

func TestService_List_ByTag(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	tests := []struct {
		tag     string
		minWant int
	}{
		{"security", 1},
		{"oidc", 3},
		{"developer", 2},
		{"login", 2},
		{"enterprise", 2}, // entra-id + sso-enterprise
	}

	for _, tc := range tests {
		got := svc.List("", tc.tag)
		if len(got) < tc.minWant {
			t.Errorf("List(tag=%q) count = %d, want >= %d", tc.tag, len(got), tc.minWant)
		}
	}
}

func TestService_List_TypeAndTag(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	got := svc.List("login_flow", "passkey")
	if len(got) != 1 {
		t.Errorf("List(login_flow, passkey) = %d, want 1", len(got))
	}
	if len(got) > 0 && got[0].ID != "passkey-first" {
		t.Errorf("got ID = %q, want passkey-first", got[0].ID)
	}
}

// ---------------------------------------------------------------------------
// Get tests
// ---------------------------------------------------------------------------

func TestService_Get(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	payload, tpl, err := svc.Get("rate-limit-by-path")
	if err != nil {
		t.Fatalf("Get: %v", err)
	}

	if tpl.ID != "rate-limit-by-path" {
		t.Errorf("template ID = %q", tpl.ID)
	}
	if payload.Name != "Rate Limit by Path" {
		t.Errorf("payload name = %q", payload.Name)
	}
	if _, ok := payload.Variables["path_prefix"]; !ok {
		t.Error("missing path_prefix variable")
	}
}

func TestService_Get_NotFound(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	_, _, err := svc.Get("nonexistent")
	if err == nil {
		t.Error("expected error for nonexistent template")
	}
}

func TestService_Get_AllTemplates(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	for _, tpl := range svc.List("", "") {
		payload, _, err := svc.Get(tpl.ID)
		if err != nil {
			t.Errorf("Get(%q): %v", tpl.ID, err)
			continue
		}
		if payload.Name == "" {
			t.Errorf("Get(%q): empty name", tpl.ID)
		}
		if payload.Payload == nil {
			t.Errorf("Get(%q): nil payload", tpl.ID)
		}
	}
}

func TestService_Get_LoginFlow(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	payload, tpl, err := svc.Get("passkey-first")
	if err != nil {
		t.Fatalf("Get(passkey-first): %v", err)
	}

	if tpl.Type != "login_flow" {
		t.Errorf("type = %q, want login_flow", tpl.Type)
	}

	// Verify login flow has expected nested structure.
	if _, ok := payload.Payload["login_policy"]; !ok {
		t.Error("missing login_policy in payload")
	}
	if _, ok := payload.Payload["auth_methods"]; !ok {
		t.Error("missing auth_methods in payload")
	}
	if _, ok := payload.Payload["branding"]; !ok {
		t.Error("missing branding in payload")
	}
	if _, ok := payload.Payload["actions"]; !ok {
		t.Error("missing actions in payload")
	}
}

// ---------------------------------------------------------------------------
// Install tests
// ---------------------------------------------------------------------------

func TestService_Install(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	userID, err := svc.Install(context.Background(), "rate-limit-by-path", map[string]any{
		"path_prefix":         "/v1/admin",
		"requests_per_minute": 200,
		"burst":               20,
	})
	if err != nil {
		t.Fatalf("Install: %v", err)
	}
	if userID == "" {
		t.Fatal("empty entity ID")
	}

	// Verify entity was created.
	var schemaID, dataJSON string
	err = db.QueryRow(`SELECT schema_id, metadata FROM actions WHERE id = ?`, userID).Scan(&schemaID, &dataJSON)
	if err != nil {
		t.Fatalf("query entity: %v", err)
	}

	if schemaID != "action_v1" {
		t.Errorf("schema_id = %q, want %q", schemaID, "action_v1")
	}

	var data map[string]any
	json.Unmarshal([]byte(dataJSON), &data)

	if data["display_name"] != "Rate Limit: /v1/admin" {
		t.Errorf("display_name = %q", data["display_name"])
	}

	trigger, _ := data["trigger"].(string)
	if trigger != "request.path startsWith '/v1/admin'" {
		t.Errorf("trigger = %q, want substituted path", trigger)
	}
}

func TestService_Install_WithDefaults(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	userID, err := svc.Install(context.Background(), "rate-limit-by-path", nil)
	if err != nil {
		t.Fatalf("Install: %v", err)
	}

	var dataJSON string
	db.QueryRow(`SELECT metadata FROM actions WHERE id = ?`, userID).Scan(&dataJSON)

	var data map[string]any
	json.Unmarshal([]byte(dataJSON), &data)

	if data["display_name"] != "Rate Limit: /v1/auth" {
		t.Errorf("display_name = %q, want default", data["display_name"])
	}
}

func TestService_Install_LoginFlow(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	userID, err := svc.Install(context.Background(), "passkey-first", map[string]any{
		"primary_color":      "#ff6600",
		"heading_text":       "Welcome to Acme",
		"allow_registration": false,
	})
	if err != nil {
		t.Fatalf("Install passkey-first: %v", err)
	}

	var schemaID, dataJSON string
	db.QueryRow(`SELECT schema_id, metadata FROM login_flows WHERE id = ?`, userID).Scan(&schemaID, &dataJSON)

	if schemaID != "login_flow_v1" {
		t.Errorf("schema_id = %q, want login_flow_v1", schemaID)
	}

	var data map[string]any
	json.Unmarshal([]byte(dataJSON), &data)

	// Verify variable substitution in nested structures.
	branding, _ := data["branding"].(map[string]any)
	if branding["heading"] != "Welcome to Acme" {
		t.Errorf("branding.heading = %q", branding["heading"])
	}

	colors, _ := branding["colors"].(map[string]any)
	if colors["primary"] != "#ff6600" {
		t.Errorf("branding.colors.primary = %q", colors["primary"])
	}

	loginPolicy, _ := data["login_policy"].(map[string]any)
	if loginPolicy["registration_allowed"] != false {
		t.Errorf("registration_allowed = %v, want false", loginPolicy["registration_allowed"])
	}
}

func TestService_Install_SSOEnterprise(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	userID, err := svc.Install(context.Background(), "sso-enterprise", map[string]any{
		"company_name": "ACME Corp",
	})
	if err != nil {
		t.Fatalf("Install sso-enterprise: %v", err)
	}

	var dataJSON string
	db.QueryRow(`SELECT metadata FROM login_flows WHERE id = ?`, userID).Scan(&dataJSON)

	var data map[string]any
	json.Unmarshal([]byte(dataJSON), &data)

	branding, _ := data["branding"].(map[string]any)
	if branding["heading"] != "Sign in to ACME Corp" {
		t.Errorf("heading = %q, want 'Sign in to ACME Corp'", branding["heading"])
	}

	loginPolicy, _ := data["login_policy"].(map[string]any)
	if loginPolicy["external_idp_only"] != true {
		t.Error("sso-enterprise should have external_idp_only=true")
	}
}

// ---------------------------------------------------------------------------
// Origin tracking tests
// ---------------------------------------------------------------------------

func TestInstall_HasCatalogMetadata(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	userID, err := svc.Install(context.Background(), "google-oidc", map[string]any{
		"client_id":     "test-id",
		"client_secret": "test-secret",
	})
	if err != nil {
		t.Fatalf("Install: %v", err)
	}

	var dataJSON string
	db.QueryRow(`SELECT metadata FROM providers WHERE id = ?`, userID).Scan(&dataJSON)

	var data map[string]any
	json.Unmarshal([]byte(dataJSON), &data)

	catalogMeta, ok := data["_catalog"].(map[string]any)
	if !ok {
		t.Fatal("missing _catalog metadata block")
	}

	if catalogMeta["template_id"] != "google-oidc" {
		t.Errorf("template_id = %q", catalogMeta["template_id"])
	}
	if catalogMeta["template_version"] != "1.0.0" {
		t.Errorf("template_version = %q", catalogMeta["template_version"])
	}
	hash, _ := catalogMeta["installed_hash"].(string)
	if hash == "" {
		t.Error("missing installed_hash")
	}
	if len(hash) < 10 {
		t.Errorf("installed_hash too short: %q", hash)
	}
}

func TestCatalogState_Linked(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	userID, _ := svc.Install(context.Background(), "rate-limit-by-path", map[string]any{
		"path_prefix":         "/v1/test",
		"requests_per_minute": 100,
		"burst":               10,
	})

	var dataJSON string
	db.QueryRow(`SELECT metadata FROM actions WHERE id = ?`, userID).Scan(&dataJSON)

	var data map[string]any
	json.Unmarshal([]byte(dataJSON), &data)

	state := CatalogState(data)
	if state != "linked" {
		t.Errorf("state = %q, want %q (unmodified)", state, "linked")
	}
}

func TestCatalogState_Forked(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	userID, _ := svc.Install(context.Background(), "rate-limit-by-path", nil)

	var dataJSON string
	db.QueryRow(`SELECT metadata FROM actions WHERE id = ?`, userID).Scan(&dataJSON)

	var data map[string]any
	json.Unmarshal([]byte(dataJSON), &data)

	// Simulate user edit.
	data["display_name"] = "My Custom Rate Limiter"

	state := CatalogState(data)
	if state != "forked" {
		t.Errorf("state = %q, want %q (modified)", state, "forked")
	}
}

func TestCatalogState_Custom(t *testing.T) {
	data := map[string]any{
		"display_name": "Custom Action",
		"hook":         "on_request",
	}

	state := CatalogState(data)
	if state != "custom" {
		t.Errorf("state = %q, want %q", state, "custom")
	}
}

func TestCatalogState_ForkedLoginFlow(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	userID, _ := svc.Install(context.Background(), "passkey-first", nil)

	var dataJSON string
	db.QueryRow(`SELECT metadata FROM login_flows WHERE id = ?`, userID).Scan(&dataJSON)

	var data map[string]any
	json.Unmarshal([]byte(dataJSON), &data)

	// Unmodified → linked.
	if state := CatalogState(data); state != "linked" {
		t.Errorf("unmodified state = %q, want linked", state)
	}

	// Modify branding → forked.
	branding, _ := data["branding"].(map[string]any)
	branding["heading"] = "Custom Heading"

	if state := CatalogState(data); state != "forked" {
		t.Errorf("modified state = %q, want forked", state)
	}
}

// ---------------------------------------------------------------------------
// Merge tests
// ---------------------------------------------------------------------------

func TestMerge_NoRemote(t *testing.T) {
	embedded := &Index{
		Version:   "1.0",
		Templates: []Template{{ID: "a", Version: "1.0.0"}},
	}

	merged := merge(embedded, nil)
	if len(merged.Templates) != 1 {
		t.Errorf("merge with nil = %d templates, want 1", len(merged.Templates))
	}
}

func TestMerge_NewTemplates(t *testing.T) {
	embedded := &Index{
		Version:   "1.0",
		Templates: []Template{{ID: "a", Version: "1.0.0", Source: "embedded"}},
	}
	remote := &Index{
		Version:   "1.0",
		Templates: []Template{{ID: "b", Version: "1.0.0", Source: "remote"}},
	}

	merged := merge(embedded, remote)
	if len(merged.Templates) != 2 {
		t.Errorf("merged = %d templates, want 2", len(merged.Templates))
	}
}

func TestMerge_UpgradeExisting(t *testing.T) {
	embedded := &Index{
		Templates: []Template{{ID: "a", Version: "1.0.0", Name: "old", Source: "embedded"}},
	}
	remote := &Index{
		Templates: []Template{{ID: "a", Version: "2.0.0", Name: "new", Source: "remote"}},
	}

	merged := merge(embedded, remote)
	if merged.Templates[0].Name != "new" {
		t.Errorf("name = %q, want %q (upgraded)", merged.Templates[0].Name, "new")
	}
}

func TestMerge_NoDowngrade(t *testing.T) {
	embedded := &Index{
		Templates: []Template{{ID: "a", Version: "2.0.0", Name: "new", Source: "embedded"}},
	}
	remote := &Index{
		Templates: []Template{{ID: "a", Version: "1.0.0", Name: "old", Source: "remote"}},
	}

	merged := merge(embedded, remote)
	if merged.Templates[0].Name != "new" {
		t.Error("older remote should not downgrade embedded")
	}
}

// ---------------------------------------------------------------------------
// Variable substitution tests
// ---------------------------------------------------------------------------

func TestSubstituteVars(t *testing.T) {
	defs := map[string]Var{
		"name":  {Type: "string", Default: "default_name"},
		"count": {Type: "integer", Default: 42},
	}

	payload := map[string]any{
		"title": "Hello {{name}}",
		"limit": "{{count}}",
		"nested": map[string]any{
			"inner": "{{name}} rocks",
		},
	}

	result := substituteVars(payload, defs, map[string]any{
		"name": "world",
	})

	if result["title"] != "Hello world" {
		t.Errorf("title = %q", result["title"])
	}
	if result["limit"] != 42 {
		t.Errorf("limit = %v (type %T), want 42", result["limit"], result["limit"])
	}

	nested, _ := result["nested"].(map[string]any)
	if nested["inner"] != "world rocks" {
		t.Errorf("nested.inner = %q", nested["inner"])
	}
}

func TestSubstituteVars_DeepNested(t *testing.T) {
	defs := map[string]Var{
		"color": {Type: "string", Default: "#000"},
	}
	payload := map[string]any{
		"branding": map[string]any{
			"colors": map[string]any{
				"primary": "{{color}}",
			},
		},
	}

	result := substituteVars(payload, defs, map[string]any{"color": "#ff0000"})
	branding, _ := result["branding"].(map[string]any)
	colors, _ := branding["colors"].(map[string]any)
	if colors["primary"] != "#ff0000" {
		t.Errorf("deep nested = %q, want #ff0000", colors["primary"])
	}
}

func TestSubstituteVars_Array(t *testing.T) {
	defs := map[string]Var{
		"name": {Type: "string"},
	}
	payload := map[string]any{
		"items": []any{"{{name}}", "static"},
	}

	result := substituteVars(payload, defs, map[string]any{"name": "test"})
	items, _ := result["items"].([]any)
	if len(items) != 2 || items[0] != "test" || items[1] != "static" {
		t.Errorf("array = %v", items)
	}
}

func TestSubstituteString_PreservesTypes(t *testing.T) {
	vars := map[string]any{
		"num":  100,
		"flag": true,
	}

	if v := substituteString("{{num}}", vars); v != 100 {
		t.Errorf("{{num}} = %v (type %T), want int 100", v, v)
	}
	if v := substituteString("{{flag}}", vars); v != true {
		t.Errorf("{{flag}} = %v, want true", v)
	}
	if v := substituteString("limit={{num}}", vars); v != "limit=100" {
		t.Errorf("partial = %q", v)
	}
}

// ---------------------------------------------------------------------------
// DB cache tests
// ---------------------------------------------------------------------------

func TestDBCache(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	remote := &Index{
		Version:   "1.0",
		Templates: []Template{{ID: "remote-1", Name: "Remote", Source: "remote"}},
	}
	svc.CacheToDB(remote)

	svc2 := New(config.CatalogConfig{}, db)
	all := svc2.List("", "")

	// 11 embedded + 1 remote = 12.
	if len(all) != 12 {
		t.Errorf("after cache reload = %d templates, want 12", len(all))
	}
}

func TestDBCache_CachesPersistAcrossRestarts(t *testing.T) {
	db := setupTestDB(t)
	svc := New(config.CatalogConfig{}, db)

	// Simulate remote fetch.
	remote := &Index{
		Version: "2.0",
		Templates: []Template{
			{ID: "community-action", Name: "Community", Type: "action", Version: "1.0.0", Source: "remote"},
			{ID: "community-provider", Name: "Keycloak OIDC", Type: "provider", Version: "1.0.0", Source: "remote"},
		},
	}
	svc.SetRemote(remote)
	svc.CacheToDB(remote)

	// Verify merge worked.
	all := svc.List("", "")
	if len(all) != 13 { // 11 + 2
		t.Errorf("after remote = %d, want 13", len(all))
	}

	// Restart — should load cache.
	svc2 := New(config.CatalogConfig{}, db)
	all2 := svc2.List("", "")
	if len(all2) != 13 {
		t.Errorf("after restart = %d, want 13", len(all2))
	}
}

// ---------------------------------------------------------------------------
// Schema upgrade preview tests
// ---------------------------------------------------------------------------

func TestDiffSchemas_AddOptionalField(t *testing.T) {
	oldSchema := map[string]any{
		"properties": map[string]any{
			"name": map[string]any{"type": "string"},
		},
		"required": []any{"name"},
	}
	newSchema := map[string]any{
		"properties": map[string]any{
			"name":  map[string]any{"type": "string"},
			"email": map[string]any{"type": "string"},
		},
		"required": []any{"name"},
	}

	changes := diffSchemas(oldSchema, newSchema)
	if len(changes) != 1 {
		t.Fatalf("changes = %d, want 1", len(changes))
	}
	if changes[0].Change != "field_added" {
		t.Errorf("change = %q, want field_added", changes[0].Change)
	}
	if changes[0].Severity != "info" {
		t.Errorf("severity = %q, want info (optional field)", changes[0].Severity)
	}
}

func TestDiffSchemas_AddRequiredField(t *testing.T) {
	oldSchema := map[string]any{
		"properties": map[string]any{
			"name": map[string]any{"type": "string"},
		},
		"required": []any{"name"},
	}
	newSchema := map[string]any{
		"properties": map[string]any{
			"name":  map[string]any{"type": "string"},
			"phone": map[string]any{"type": "string"},
		},
		"required": []any{"name", "phone"},
	}

	changes := diffSchemas(oldSchema, newSchema)
	var found bool
	for _, c := range changes {
		if c.Path == "properties.phone" && c.Severity == "breaking" {
			found = true
		}
	}
	if !found {
		t.Error("expected breaking change for new required field 'phone'")
	}
}

func TestDiffSchemas_AddRequiredFieldWithDefault(t *testing.T) {
	oldSchema := map[string]any{
		"properties": map[string]any{
			"name": map[string]any{"type": "string"},
		},
		"required": []any{"name"},
	}
	newSchema := map[string]any{
		"properties": map[string]any{
			"name": map[string]any{"type": "string"},
			"role": map[string]any{"type": "string", "default": "viewer"},
		},
		"required": []any{"name", "role"},
	}

	changes := diffSchemas(oldSchema, newSchema)
	var found bool
	for _, c := range changes {
		if c.Path == "properties.role" && c.Severity == "warning" {
			found = true
		}
	}
	if !found {
		t.Error("expected warning (not breaking) for new required field with default")
	}
}

func TestDiffSchemas_RemoveField(t *testing.T) {
	oldSchema := map[string]any{
		"properties": map[string]any{
			"name":  map[string]any{"type": "string"},
			"phone": map[string]any{"type": "string"},
		},
	}
	newSchema := map[string]any{
		"properties": map[string]any{
			"name": map[string]any{"type": "string"},
		},
	}

	changes := diffSchemas(oldSchema, newSchema)
	if len(changes) != 1 || changes[0].Change != "field_removed" {
		t.Errorf("changes = %v, want field_removed for phone", changes)
	}
}

func TestDiffSchemas_TypeChange(t *testing.T) {
	oldSchema := map[string]any{
		"properties": map[string]any{
			"age": map[string]any{"type": "string"},
		},
	}
	newSchema := map[string]any{
		"properties": map[string]any{
			"age": map[string]any{"type": "integer"},
		},
	}

	changes := diffSchemas(oldSchema, newSchema)
	if len(changes) != 1 || changes[0].Change != "type_changed" || changes[0].Severity != "breaking" {
		t.Errorf("changes = %v, want type_changed:breaking", changes)
	}
}

func TestDiffSchemas_RequiredToOptional(t *testing.T) {
	oldSchema := map[string]any{
		"properties": map[string]any{
			"name": map[string]any{"type": "string"},
		},
		"required": []any{"name"},
	}
	newSchema := map[string]any{
		"properties": map[string]any{
			"name": map[string]any{"type": "string"},
		},
	}

	changes := diffSchemas(oldSchema, newSchema)
	if len(changes) != 1 || changes[0].Change != "required_removed" || changes[0].Severity != "info" {
		t.Errorf("changes = %v, want required_removed:info", changes)
	}
}

func TestDiffSchemas_NilOldSchema(t *testing.T) {
	newSchema := map[string]any{
		"properties": map[string]any{
			"name": map[string]any{"type": "string"},
		},
	}

	changes := diffSchemas(nil, newSchema)
	if len(changes) != 1 || changes[0].Change != "field_added" {
		t.Errorf("nil old → new should detect field additions, got %v", changes)
	}
}

func TestPreviewUpgrade_WithEntities(t *testing.T) {
	db := setupTestDB(t)
	seedTestEntities(t, db)

	// Propose a schema that makes phone required.
	newSchema := map[string]any{
		"properties": map[string]any{
			"display_name": map[string]any{"type": "string"},
			"email":        map[string]any{"type": "string"},
			"phone":        map[string]any{"type": "string"},
			"locale":       map[string]any{"type": "string"},
		},
		"required": []any{"display_name", "email", "phone"},
	}

	report, err := PreviewUpgrade(context.Background(), db, "human_user", newSchema, 10)
	if err != nil {
		t.Fatalf("PreviewUpgrade: %v", err)
	}

	if report.TotalEntities != 5 {
		t.Errorf("total = %d, want 5", report.TotalEntities)
	}
	if report.Sampled != 5 {
		t.Errorf("sampled = %d, want 5", report.Sampled)
	}

	// Bob (no phone) and Charlie (phone="") should be breaking.
	// Dana has wrong email type → warning or breaking.
	if report.Impact.Breaking < 2 {
		t.Errorf("breaking = %d, want >= 2 (Bob + Charlie)", report.Impact.Breaking)
	}
	if report.Impact.Valid < 1 {
		t.Errorf("valid = %d, want >= 1 (Alice or Eve)", report.Impact.Valid)
	}
}

func TestPreviewUpgrade_NoEntities(t *testing.T) {
	db := setupTestDB(t)

	report, err := PreviewUpgrade(context.Background(), db, "nonexistent", map[string]any{
		"properties": map[string]any{},
	}, 10)
	if err != nil {
		t.Fatalf("PreviewUpgrade: %v", err)
	}

	if report.TotalEntities != 0 {
		t.Errorf("total = %d, want 0", report.TotalEntities)
	}
	if report.Sampled != 0 {
		t.Errorf("sampled = %d, want 0", report.Sampled)
	}
}

func TestPreviewUpgrade_AddOptionalField(t *testing.T) {
	db := setupTestDB(t)
	seedTestEntities(t, db)

	// Add an optional field — should be non-breaking.
	newSchema := map[string]any{
		"properties": map[string]any{
			"display_name":   map[string]any{"type": "string"},
			"email":          map[string]any{"type": "string"},
			"mfa_preference": map[string]any{"type": "string", "default": "prompt"},
		},
		"required": []any{"display_name"},
	}

	report, err := PreviewUpgrade(context.Background(), db, "human_user", newSchema, 10)
	if err != nil {
		t.Fatalf("PreviewUpgrade: %v", err)
	}

	if report.Impact.Breaking != 0 {
		t.Errorf("breaking = %d for optional field add, want 0", report.Impact.Breaking)
	}
}

func TestPreviewUpgrade_SampleSizeLimits(t *testing.T) {
	db := setupTestDB(t)
	seedTestEntities(t, db)

	// Sample size 2 — should only get 2 entities.
	report, err := PreviewUpgrade(context.Background(), db, "human_user", map[string]any{
		"properties": map[string]any{},
	}, 2)
	if err != nil {
		t.Fatalf("PreviewUpgrade: %v", err)
	}

	if report.Sampled != 2 {
		t.Errorf("sampled = %d, want 2", report.Sampled)
	}
	if report.TotalEntities != 5 {
		t.Errorf("total = %d, want 5", report.TotalEntities)
	}
}

func TestValidateEntity_TypeMismatch(t *testing.T) {
	schema := map[string]any{
		"properties": map[string]any{
			"email": map[string]any{"type": "string"},
			"count": map[string]any{"type": "integer"},
		},
	}

	// email is number instead of string.
	data := map[string]any{"email": 123, "count": "not-a-number"}
	result := validateEntityAgainstSchema("id1", "Test", data, schema, nil)

	if result.Status == "valid" {
		t.Error("should detect type mismatches")
	}
	if len(result.Changes) < 1 {
		t.Error("should report at least one type mismatch")
	}
}

func TestIsTypeCompatible(t *testing.T) {
	tests := []struct {
		val      any
		expected string
		want     bool
	}{
		{"hello", "string", true},
		{123, "string", false},
		{float64(42), "integer", true},
		{float64(42.5), "integer", false},
		{float64(42), "number", true},
		{true, "boolean", true},
		{"true", "boolean", false},
		{map[string]any{}, "object", true},
		{[]any{1, 2}, "array", true},
		{"hello", "array", false},
	}

	for _, tc := range tests {
		got := isTypeCompatible(tc.val, tc.expected)
		if got != tc.want {
			t.Errorf("isTypeCompatible(%v, %q) = %v, want %v", tc.val, tc.expected, got, tc.want)
		}
	}
}

// ---------------------------------------------------------------------------
// Hash and compute tests
// ---------------------------------------------------------------------------

func TestComputeHash_Deterministic(t *testing.T) {
	data := []byte(`{"hello":"world"}`)
	h1 := computeHash(data)
	h2 := computeHash(data)
	if h1 != h2 {
		t.Error("same data should produce same hash")
	}
	if h1 == "" {
		t.Error("hash should not be empty")
	}
	if !hasPrefix(h1, "sha256:") {
		t.Errorf("hash = %q, want sha256: prefix", h1)
	}
}

func TestComputeHash_Different(t *testing.T) {
	h1 := computeHash([]byte(`{"a":1}`))
	h2 := computeHash([]byte(`{"a":2}`))
	if h1 == h2 {
		t.Error("different data should produce different hashes")
	}
}

func hasPrefix(s, prefix string) bool {
	return len(s) >= len(prefix) && s[:len(prefix)] == prefix
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

func BenchmarkList_NoFilter(b *testing.B) {
	db := setupBenchDB(b)
	svc := New(config.CatalogConfig{}, db)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		svc.List("", "")
	}
}

func BenchmarkList_TypeFilter(b *testing.B) {
	db := setupBenchDB(b)
	svc := New(config.CatalogConfig{}, db)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		svc.List("action", "")
	}
}

func BenchmarkGet(b *testing.B) {
	db := setupBenchDB(b)
	svc := New(config.CatalogConfig{}, db)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, _, err := svc.Get("rate-limit-by-path")
		if err != nil {
			b.Fatal(err)
		}
	}
}

func BenchmarkCatalogState(b *testing.B) {
	db := setupBenchDB(b)
	svc := New(config.CatalogConfig{}, db)

	db.Exec(`CREATE TABLE IF NOT EXISTS schemas (id TEXT PRIMARY KEY, type TEXT NOT NULL, org_id TEXT DEFAULT '1', schema TEXT DEFAULT '{}', version INTEGER DEFAULT 1, is_default BOOLEAN DEFAULT false, visibility TEXT DEFAULT 'private', message TEXT DEFAULT '', created_by TEXT DEFAULT '', created_at TEXT DEFAULT (datetime('now')))`)
	for _, st := range []string{"action", "provider", "authorization", "login_flow", "fga_model"} {
		db.Exec(`INSERT OR IGNORE INTO schemas (id, type, is_default) VALUES (?, ?, true)`, st+"_v1", st)
	}
	db.Exec(`CREATE TABLE IF NOT EXISTS actions (id TEXT PRIMARY KEY, org_id TEXT DEFAULT '1', name TEXT DEFAULT '', hook TEXT DEFAULT 'on_event', action_type TEXT DEFAULT 'expr', trigger_expr TEXT DEFAULT 'true', config TEXT DEFAULT '{}', priority INTEGER DEFAULT 0, enabled BOOLEAN DEFAULT 1, fail_open BOOLEAN DEFAULT 0, timeout_ms INTEGER DEFAULT 5000, schema_id TEXT DEFAULT '', metadata TEXT DEFAULT '{}', created_at TEXT DEFAULT (datetime('now')), updated_at TEXT DEFAULT (datetime('now')))`)
	userID, _ := svc.Install(context.Background(), "rate-limit-by-path", nil)

	var dataJSON string
	db.QueryRow(`SELECT metadata FROM actions WHERE id = ?`, userID).Scan(&dataJSON)
	var data map[string]any
	json.Unmarshal([]byte(dataJSON), &data)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		CatalogState(data)
	}
}

func setupBenchDB(b *testing.B) *sql.DB {
	b.Helper()
	db, _ := sql.Open("sqlite", ":memory:")
	db.Exec(`CREATE TABLE cache (namespace TEXT NOT NULL DEFAULT 'default', key TEXT NOT NULL, data TEXT NOT NULL, expires_at TEXT, fetched_at TEXT NOT NULL DEFAULT (datetime('now')), PRIMARY KEY (namespace, key))`)
	b.Cleanup(func() { db.Close() })
	return db
}
