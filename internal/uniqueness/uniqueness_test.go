package uniqueness

import (
	"context"
	"database/sql"
	"errors"
	"testing"

	_ "modernc.org/sqlite"
)

func setupTestDB(t *testing.T) *sql.DB {
	t.Helper()
	db, err := sql.Open("sqlite", ":memory:")
	if err != nil {
		t.Fatalf("open db: %v", err)
	}

	// Create minimal schema.
	_, err = db.Exec(`
		CREATE TABLE entities (
			id           TEXT PRIMARY KEY,
			org_id       TEXT NOT NULL DEFAULT '0',
			identifier   TEXT NOT NULL DEFAULT '',
			display_name TEXT DEFAULT '',
			state        TEXT NOT NULL DEFAULT 'active',
			schema_id    TEXT DEFAULT '',
			data         TEXT DEFAULT '{}',
			created_at   TEXT NOT NULL DEFAULT (datetime('now')),
			updated_at   TEXT NOT NULL DEFAULT (datetime('now'))
		);
		CREATE UNIQUE INDEX idx_entities_identifier ON entities(org_id, identifier);

		CREATE TABLE unique_fields (
			scope_id         TEXT NOT NULL DEFAULT '',
			field_name       TEXT NOT NULL,
			normalized_value TEXT NOT NULL,
			user_id        TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
			UNIQUE(scope_id, field_name, normalized_value)
		);
		CREATE INDEX idx_unique_fields_entity ON unique_fields(user_id);
		CREATE INDEX idx_unique_fields_lookup ON unique_fields(normalized_value, field_name);
	`)
	if err != nil {
		t.Fatalf("create schema: %v", err)
	}

	t.Cleanup(func() { db.Close() })
	return db
}

// commitTx commits the transaction, failing the test on error.
func commitTx(tb testing.TB, tx *sql.Tx) {
	tb.Helper()
	if err := tx.Commit(); err != nil {
		tb.Fatalf("tx.Commit: %v", err)
	}
}

func insertEntity(t *testing.T, db *sql.DB, id, orgID, identifier string) {
	t.Helper()
	_, err := db.Exec(
		`INSERT INTO users (id, org_id, identifier, display_name, state)
		 VALUES (?, ?, ?, ?, 'active')`,
		id, orgID, identifier, identifier,
	)
	if err != nil {
		t.Fatalf("insert entity %s: %v", id, err)
	}
}

// --- ExtractConstraints ---

func TestExtractConstraints(t *testing.T) {
	schema := `{
		"properties": {
			"email": { "type": "string", "x-unique": "instance" },
			"username": { "type": "string", "x-unique": "org" },
			"phone": { "type": "string", "x-identifier": true },
			"notes": { "type": "string" }
		}
	}`

	constraints := ExtractConstraints(schema)
	if len(constraints) != 2 {
		t.Fatalf("expected 2 constraints, got %d", len(constraints))
	}

	byName := map[string]FieldConstraint{}
	for _, c := range constraints {
		byName[c.FieldName] = c
	}

	if c, ok := byName["email"]; !ok || c.Scope != ScopeInstance {
		t.Errorf("email: expected instance scope, got %+v", byName["email"])
	}
	if c, ok := byName["username"]; !ok || c.Scope != ScopeOrg {
		t.Errorf("username: expected org scope, got %+v", byName["username"])
	}
	if _, ok := byName["phone"]; ok {
		t.Error("phone should not have a uniqueness constraint")
	}
}

func TestExtractConstraints_FalseValue(t *testing.T) {
	schema := `{
		"properties": {
			"email": { "type": "string", "x-unique": false }
		}
	}`

	constraints := ExtractConstraints(schema)
	if len(constraints) != 0 {
		t.Fatalf("expected 0 constraints, got %d", len(constraints))
	}
}

func TestExtractConstraints_InvalidJSON(t *testing.T) {
	constraints := ExtractConstraints(`{invalid}`)
	if constraints != nil {
		t.Fatalf("expected nil, got %v", constraints)
	}
}

// --- ExtractIdentifiers ---

func TestExtractIdentifiers(t *testing.T) {
	schema := `{
		"properties": {
			"email": { "x-identifier": true, "x-unique": "instance" },
			"phone": { "x-identifier": true },
			"notes": { "type": "string" }
		}
	}`

	ids := ExtractIdentifiers(schema)
	if len(ids) != 2 {
		t.Fatalf("expected 2 identifiers, got %d: %v", len(ids), ids)
	}
}

// --- Normalize ---

func TestNormalize(t *testing.T) {
	tests := []struct {
		input, want string
	}{
		{"Alice@Example.COM", "alice@example.com"},
		{"  bob  ", "bob"},
		{"", ""},
		{"UPPER", "upper"},
	}
	for _, tt := range tests {
		got := Normalize(tt.input)
		if got != tt.want {
			t.Errorf("Normalize(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

// --- Enforce ---

func TestEnforce_InstanceScope(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice@test.com")
	insertEntity(t, db, "e2", "org2", "bob@test.com")

	constraints := []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
	}

	// First insert: OK.
	tx, _ := db.BeginTx(ctx, nil)
	err := Enforce(ctx, tx, "e1", "org1", constraints, map[string]any{"email": "alice@test.com"})
	if err != nil {
		t.Fatalf("first enforce failed: %v", err)
	}
	commitTx(t, tx)

	// Second insert with same email in different org: FAIL (instance-scoped).
	tx2, _ := db.BeginTx(ctx, nil)
	err = Enforce(ctx, tx2, "e2", "org2", constraints, map[string]any{"email": "alice@test.com"})
	if err == nil {
		t.Fatal("expected uniqueness violation, got nil")
	}
	violation, ok := err.(*ViolationError)
	if !ok {
		t.Fatalf("expected *ViolationError, got %T", err)
	}
	if violation.Field != "email" {
		t.Errorf("field = %q, want email", violation.Field)
	}
	if violation.Scope != "instance" {
		t.Errorf("scope = %q, want instance", violation.Scope)
	}
	tx2.Rollback()
}

func TestEnforce_OrgScope(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice")
	insertEntity(t, db, "e2", "org2", "alice-org2")
	insertEntity(t, db, "e3", "org1", "bob")

	constraints := []FieldConstraint{
		{FieldName: "username", Scope: ScopeOrg},
	}

	// User "alice" in org1.
	tx, _ := db.BeginTx(ctx, nil)
	err := Enforce(ctx, tx, "e1", "org1", constraints, map[string]any{"username": "alice"})
	if err != nil {
		t.Fatalf("first enforce failed: %v", err)
	}
	commitTx(t, tx)

	// "alice" in org2: OK (different org, org-scoped).
	tx2, _ := db.BeginTx(ctx, nil)
	err = Enforce(ctx, tx2, "e2", "org2", constraints, map[string]any{"username": "alice"})
	if err != nil {
		t.Fatalf("cross-org should succeed: %v", err)
	}
	commitTx(t, tx2)

	// Another "alice" in org1: FAIL (same org).
	tx3, _ := db.BeginTx(ctx, nil)
	err = Enforce(ctx, tx3, "e3", "org1", constraints, map[string]any{"username": "alice"})
	if err == nil {
		t.Fatal("expected uniqueness violation within same org, got nil")
	}
	tx3.Rollback()
}

func TestEnforce_CaseInsensitive(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice@test.com")
	insertEntity(t, db, "e2", "org1", "bob")

	constraints := []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
	}

	tx, _ := db.BeginTx(ctx, nil)
	err := Enforce(ctx, tx, "e1", "org1", constraints, map[string]any{"email": "Alice@Test.COM"})
	if err != nil {
		t.Fatalf("first enforce failed: %v", err)
	}
	commitTx(t, tx)

	tx2, _ := db.BeginTx(ctx, nil)
	err = Enforce(ctx, tx2, "e2", "org1", constraints, map[string]any{"email": "ALICE@TEST.COM"})
	if err == nil {
		t.Fatal("case-insensitive uniqueness should fail")
	}
	tx2.Rollback()
}

func TestEnforce_EmptyValue_Skipped(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "test")

	constraints := []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
	}

	tx, _ := db.BeginTx(ctx, nil)
	err := Enforce(ctx, tx, "e1", "org1", constraints, map[string]any{"email": ""})
	if err != nil {
		t.Fatalf("empty value should be skipped: %v", err)
	}
	commitTx(t, tx)

	// nil value also skipped.
	insertEntity(t, db, "e2", "org1", "test2")
	tx2, _ := db.BeginTx(ctx, nil)
	err = Enforce(ctx, tx2, "e2", "org1", constraints, map[string]any{})
	if err != nil {
		t.Fatalf("missing value should be skipped: %v", err)
	}
	commitTx(t, tx2)
}

// --- Release ---

func TestRelease(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice")
	insertEntity(t, db, "e2", "org1", "bob")

	constraints := []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
	}

	// Insert unique field for e1.
	tx, _ := db.BeginTx(ctx, nil)
	if err := Enforce(ctx, tx, "e1", "org1", constraints, map[string]any{"email": "alice@test.com"}); err != nil {
		t.Fatalf("enforce: %v", err)
	}
	commitTx(t, tx)

	// Release e1 unique fields.
	tx2, _ := db.BeginTx(ctx, nil)
	err := Release(ctx, tx2, "e1")
	if err != nil {
		t.Fatalf("release failed: %v", err)
	}
	commitTx(t, tx2)

	// Now e2 can claim that email.
	tx3, _ := db.BeginTx(ctx, nil)
	err = Enforce(ctx, tx3, "e2", "org1", constraints, map[string]any{"email": "alice@test.com"})
	if err != nil {
		t.Fatalf("after release, should succeed: %v", err)
	}
	commitTx(t, tx3)
}

// --- ResolveIdentifier ---

func TestResolveIdentifier_InstanceScope(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice@test.com")

	// Insert instance-scoped unique field.
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id)
	         VALUES ('', 'email', 'alice@test.com', 'e1')`)

	result, err := ResolveIdentifier(ctx, db, "Alice@Test.COM", "")
	if err != nil {
		t.Fatalf("resolve failed: %v", err)
	}
	if result == nil {
		t.Fatal("expected result, got nil")
	}
	if result.UserID != "e1" {
		t.Errorf("user_id = %q, want e1", result.UserID)
	}
}

func TestResolveIdentifier_OrgScope(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice")
	insertEntity(t, db, "e2", "org2", "alice")

	// Insert org-scoped unique fields.
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id)
	         VALUES ('org1', 'username', 'alice', 'e1')`)
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id)
	         VALUES ('org2', 'username', 'alice', 'e2')`)

	// Resolve with org1 context.
	result, err := ResolveIdentifier(ctx, db, "alice", "org1")
	if err != nil {
		t.Fatalf("resolve failed: %v", err)
	}
	if result == nil || result.UserID != "e1" {
		t.Fatalf("expected e1, got %+v", result)
	}

	// Resolve with org2 context.
	result, err = ResolveIdentifier(ctx, db, "alice", "org2")
	if err != nil {
		t.Fatalf("resolve failed: %v", err)
	}
	if result == nil || result.UserID != "e2" {
		t.Fatalf("expected e2, got %+v", result)
	}
}

func TestResolveIdentifier_NotFound(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	result, err := ResolveIdentifier(ctx, db, "nonexistent@test.com", "")
	if !errors.Is(err, ErrIdentityNotFound) {
		t.Fatalf("expected ErrIdentityNotFound, got err=%v result=%+v", err, result)
	}
}

func TestResolveIdentifier_LegacyFallback(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	// Entity exists in entities table but NOT in unique_fields (legacy entity).
	insertEntity(t, db, "e1", "1", "admin")

	result, err := ResolveIdentifier(ctx, db, "admin", "1")
	if err != nil {
		t.Fatalf("resolve failed: %v", err)
	}
	if result == nil || result.UserID != "e1" {
		t.Fatalf("expected e1 via legacy fallback, got %+v", result)
	}
}

func TestResolveIdentifier_InstanceBeforeOrg(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e-global", "org1", "alice@global.com")
	insertEntity(t, db, "e-org", "org2", "alice-org")

	// Instance-scoped email.
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id)
	         VALUES ('', 'email', 'alice@global.com', 'e-global')`)
	// Org-scoped username with same value (unusual but possible).
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id)
	         VALUES ('org2', 'email', 'alice@global.com', 'e-org')`)

	// Instance match should win even when org is provided.
	result, err := ResolveIdentifier(ctx, db, "alice@global.com", "org2")
	if err != nil {
		t.Fatalf("resolve failed: %v", err)
	}
	if result == nil || result.UserID != "e-global" {
		t.Fatalf("instance scope should take priority, got %+v", result)
	}
}

// --- EnforceFromIdentifier ---

func TestEnforceFromIdentifier(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "admin")
	insertEntity(t, db, "e2", "org1", "admin2")

	tx, _ := db.BeginTx(ctx, nil)
	err := EnforceFromIdentifier(ctx, tx, "e1", "org1", "admin")
	if err != nil {
		t.Fatalf("first enforce failed: %v", err)
	}
	commitTx(t, tx)

	tx2, _ := db.BeginTx(ctx, nil)
	err = EnforceFromIdentifier(ctx, tx2, "e2", "org1", "admin")
	if err == nil {
		t.Fatal("expected uniqueness violation")
	}
	tx2.Rollback()
}

// --- Multi-field constraints ---

func TestEnforce_MultipleFields(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice")
	insertEntity(t, db, "e2", "org1", "bob")

	constraints := []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
		{FieldName: "username", Scope: ScopeOrg},
	}

	// e1: email + username.
	tx, _ := db.BeginTx(ctx, nil)
	err := Enforce(ctx, tx, "e1", "org1", constraints, map[string]any{
		"email":    "alice@test.com",
		"username": "alice",
	})
	if err != nil {
		t.Fatalf("first enforce failed: %v", err)
	}
	commitTx(t, tx)

	// e2: different email, same username in same org → FAIL on username.
	tx2, _ := db.BeginTx(ctx, nil)
	err = Enforce(ctx, tx2, "e2", "org1", constraints, map[string]any{
		"email":    "bob@test.com",
		"username": "alice",
	})
	if err == nil {
		t.Fatal("expected violation on username")
	}
	v := err.(*ViolationError)
	if v.Field != "username" {
		t.Errorf("expected violation on 'username', got %q", v.Field)
	}
	tx2.Rollback()

	// e2: same email, different username → FAIL on email.
	tx3, _ := db.BeginTx(ctx, nil)
	err = Enforce(ctx, tx3, "e2", "org1", constraints, map[string]any{
		"email":    "alice@test.com",
		"username": "bob",
	})
	if err == nil {
		t.Fatal("expected violation on email")
	}
	v = err.(*ViolationError)
	if v.Field != "email" {
		t.Errorf("expected violation on 'email', got %q", v.Field)
	}
	tx3.Rollback()
}

// --- Cross-type uniqueness (ADR-016 §2) ---

func TestEnforce_CrossTypeUniqueness(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	// Two entities of different "schema types" but sharing the same namespace.
	insertEntity(t, db, "human1", "org1", "human-alice")
	insertEntity(t, db, "svc1", "org1", "svc-alice")

	constraints := []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
	}

	// human_user claims alice@test.com.
	tx, _ := db.BeginTx(ctx, nil)
	err := Enforce(ctx, tx, "human1", "org1", constraints, map[string]any{"email": "alice@test.com"})
	if err != nil {
		t.Fatalf("human enforce failed: %v", err)
	}
	commitTx(t, tx)

	// service_user tries to claim same email → FAIL (cross-type).
	tx2, _ := db.BeginTx(ctx, nil)
	err = Enforce(ctx, tx2, "svc1", "org1", constraints, map[string]any{"email": "alice@test.com"})
	if err == nil {
		t.Fatal("cross-type uniqueness should be enforced")
	}
	tx2.Rollback()
}

// --- Update cycle: Release → Re-enforce ---

func TestRelease_ReEnforce_Cycle(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice")

	constraints := []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
		{FieldName: "username", Scope: ScopeOrg},
	}

	// Initial enforce.
	tx, _ := db.BeginTx(ctx, nil)
	err := Enforce(ctx, tx, "e1", "org1", constraints, map[string]any{
		"email":    "alice@test.com",
		"username": "alice",
	})
	if err != nil {
		t.Fatalf("initial enforce failed: %v", err)
	}
	commitTx(t, tx)

	// Simulate update: release old values, enforce new ones.
	tx2, _ := db.BeginTx(ctx, nil)
	err = Release(ctx, tx2, "e1")
	if err != nil {
		t.Fatalf("release failed: %v", err)
	}
	err = Enforce(ctx, tx2, "e1", "org1", constraints, map[string]any{
		"email":    "newalice@test.com", // changed email
		"username": "alice",             // same username
	})
	if err != nil {
		t.Fatalf("re-enforce failed: %v", err)
	}
	commitTx(t, tx2)

	// Verify old email is freed.
	var count int
	db.QueryRow(`SELECT COUNT(*) FROM unique_fields WHERE normalized_value = 'alice@test.com'`).Scan(&count)
	if count != 0 {
		t.Errorf("old email should be released, found %d rows", count)
	}

	// Verify new email is claimed.
	db.QueryRow(`SELECT COUNT(*) FROM unique_fields WHERE normalized_value = 'newalice@test.com'`).Scan(&count)
	if count != 1 {
		t.Errorf("new email should be claimed, found %d rows", count)
	}
}

// --- Violation error message ---

func TestViolationError_Error(t *testing.T) {
	v := &ViolationError{Field: "email", Value: "test@x.com", Scope: "instance"}
	msg := v.Error()
	if msg == "" {
		t.Fatal("violation error should not be empty")
	}
	if !contains(msg, "email") || !contains(msg, "test@x.com") || !contains(msg, "instance") {
		t.Errorf("violation error should contain field, value, scope: %q", msg)
	}
}

func contains(s, sub string) bool {
	return len(s) >= len(sub) && (s == sub || len(s) > 0 && containsStr(s, sub))
}

func containsStr(s, sub string) bool {
	for i := 0; i <= len(s)-len(sub); i++ {
		if s[i:i+len(sub)] == sub {
			return true
		}
	}
	return false
}

// --- Resolve: inactive entities skipped ---

func TestResolveIdentifier_SkipsInactive(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	// Insert an inactive entity.
	db.Exec(`INSERT INTO users (id, org_id, identifier, display_name, state) VALUES ('e1', 'org1', 'alice@test.com', 'Alice', 'deactivated')`)
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id) VALUES ('', 'email', 'alice@test.com', 'e1')`)

	result, err := ResolveIdentifier(ctx, db, "alice@test.com", "")
	if !errors.Is(err, ErrIdentityNotFound) {
		t.Fatalf("inactive entity should not resolve, got result=%+v err=%v", result, err)
	}
}

// --- Resolve: legacy fallback is case-insensitive ---

func TestResolveIdentifier_LegacyCaseInsensitive(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "1", "Admin")

	result, err := ResolveIdentifier(ctx, db, "admin", "1")
	if err != nil {
		t.Fatalf("resolve failed: %v", err)
	}
	if result == nil || result.UserID != "e1" {
		t.Fatalf("legacy fallback should be case-insensitive, got %+v", result)
	}
}

// --- EnforceFromIdentifier: empty identifier ---

func TestEnforceFromIdentifier_Empty(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "test")

	tx, _ := db.BeginTx(ctx, nil)
	err := EnforceFromIdentifier(ctx, tx, "e1", "org1", "")
	if err != nil {
		t.Fatalf("empty identifier should be a no-op: %v", err)
	}
	commitTx(t, tx)

	var count int
	db.QueryRow(`SELECT COUNT(*) FROM unique_fields WHERE user_id = 'e1'`).Scan(&count)
	if count != 0 {
		t.Errorf("no unique_fields should be created for empty identifier, got %d", count)
	}
}

// --- Extract from real human_user schema ---

func TestExtractConstraints_HumanUserSchema(t *testing.T) {
	schema := `{
		"properties": {
			"display_name": { "type": "string", "x-editable": true },
			"email": { "type": "string", "format": "email", "x-identifier": true, "x-unique": "instance" },
			"username": { "type": "string", "x-identifier": true, "x-unique": "org" },
			"phone": { "type": "string", "x-identifier": true, "x-mfa": "sms" },
			"locale": { "type": "string" },
			"timezone": { "type": "string" },
			"avatar_url": { "type": "string" },
			"metadata": { "type": "object", "x-hidden": true }
		}
	}`

	constraints := ExtractConstraints(schema)
	identifiers := ExtractIdentifiers(schema)

	// Exactly 2 unique constraints: email (instance) + username (org).
	if len(constraints) != 2 {
		t.Fatalf("expected 2 constraints, got %d: %+v", len(constraints), constraints)
	}

	// 3 identifiers: email, username, phone.
	if len(identifiers) != 3 {
		t.Fatalf("expected 3 identifiers, got %d: %v", len(identifiers), identifiers)
	}

	// Verify phone is NOT in constraints (no x-unique).
	for _, c := range constraints {
		if c.FieldName == "phone" {
			t.Error("phone should not have a uniqueness constraint")
		}
	}

	// Verify metadata/locale/timezone/avatar are not identifiers.
	for _, id := range identifiers {
		if id == "locale" || id == "timezone" || id == "avatar_url" || id == "metadata" || id == "display_name" {
			t.Errorf("unexpected identifier: %s", id)
		}
	}
}

// --- Whitespace-only values treated as empty ---

func TestEnforce_WhitespaceOnlyValue(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "test")

	constraints := []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
	}

	tx, _ := db.BeginTx(ctx, nil)
	err := Enforce(ctx, tx, "e1", "org1", constraints, map[string]any{"email": "   "})
	if err != nil {
		t.Fatalf("whitespace-only should be treated as empty (skipped): %v", err)
	}
	commitTx(t, tx)

	var count int
	db.QueryRow(`SELECT COUNT(*) FROM unique_fields WHERE user_id = 'e1'`).Scan(&count)
	if count != 0 {
		t.Errorf("whitespace-only should not create unique_fields row, got %d", count)
	}
}

// --- Same entity can re-claim its own value after release ---

func TestEnforce_SameEntityReEnforce(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice")

	constraints := []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
	}

	// Initial enforce.
	tx, _ := db.BeginTx(ctx, nil)
	if err := Enforce(ctx, tx, "e1", "org1", constraints, map[string]any{"email": "alice@test.com"}); err != nil {
		t.Fatalf("enforce: %v", err)
	}
	commitTx(t, tx)

	// Release.
	tx2, _ := db.BeginTx(ctx, nil)
	if err := Release(ctx, tx2, "e1"); err != nil {
		t.Fatalf("release: %v", err)
	}
	commitTx(t, tx2)

	// Re-enforce same value for same entity: should succeed.
	tx3, _ := db.BeginTx(ctx, nil)
	err := Enforce(ctx, tx3, "e1", "org1", constraints, map[string]any{"email": "alice@test.com"})
	if err != nil {
		t.Fatalf("re-enforce same value for same entity should succeed: %v", err)
	}
	commitTx(t, tx3)
}

// --- ValidateSchemaChange ---

func TestValidateSchemaChange_NoDuplicates(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice")
	insertEntity(t, db, "e2", "org1", "bob")

	// Insert distinct values.
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id) VALUES ('', 'email', 'alice@test.com', 'e1')`)
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id) VALUES ('', 'email', 'bob@test.com', 'e2')`)

	violations, err := ValidateSchemaChange(ctx, db, []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
	})
	if err != nil {
		t.Fatalf("validation failed: %v", err)
	}
	if len(violations) != 0 {
		t.Fatalf("expected 0 violations, got %d: %v", len(violations), violations)
	}
}

func TestValidateSchemaChange_WithDuplicates(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "alice")
	insertEntity(t, db, "e2", "org2", "alice2")

	// Insert DUPLICATE values (simulating pre-existing data).
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id) VALUES ('', 'email', 'shared@test.com', 'e1')`)
	// Manually insert a second row with same value but different scope_id to bypass UNIQUE constraint
	// In real scenario, these would exist from before the constraint was added.
	// For testing, insert into org-scoped first, then check instance-scoped validation.
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id) VALUES ('org1', 'email', 'shared@test.com', 'e1')`)
	db.Exec(`INSERT INTO unique_fields (scope_id, field_name, normalized_value, user_id) VALUES ('org2', 'email', 'shared@test.com', 'e2')`)

	// Check org-scoped: no duplicates (different org_ids).
	violations, err := ValidateSchemaChange(ctx, db, []FieldConstraint{
		{FieldName: "email", Scope: ScopeOrg},
	})
	if err != nil {
		t.Fatalf("validation failed: %v", err)
	}
	if len(violations) != 0 {
		t.Fatalf("org-scoped should have 0 violations (different orgs), got %d", len(violations))
	}
}

// --- Resolve: org context without unique_fields falls back to legacy with org filter ---

func TestResolveIdentifier_LegacyOrgScoped(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	// Two entities with same identifier in different orgs (legacy, no unique_fields).
	insertEntity(t, db, "e1", "org1", "alice")
	// Can't insert same (org_id, identifier) due to UNIQUE index, so test different orgs.
	db.Exec(`INSERT INTO users (id, org_id, identifier, display_name, state) VALUES ('e2', 'org2', 'alice', 'Alice Org2', 'active')`)

	// Without unique_fields, legacy fallback should respect org context.
	result, err := ResolveIdentifier(ctx, db, "alice", "org1")
	if err != nil {
		t.Fatalf("resolve failed: %v", err)
	}
	if result == nil || result.UserID != "e1" {
		t.Fatalf("expected e1 for org1, got %+v", result)
	}

	result, err = ResolveIdentifier(ctx, db, "alice", "org2")
	if err != nil {
		t.Fatalf("resolve failed: %v", err)
	}
	if result == nil || result.UserID != "e2" {
		t.Fatalf("expected e2 for org2, got %+v", result)
	}
}

// --- Extract: invalid x-unique scope is ignored ---

func TestExtractConstraints_InvalidScope(t *testing.T) {
	schema := `{
		"properties": {
			"email": { "type": "string", "x-unique": "global" },
			"phone": { "type": "string", "x-unique": 42 }
		}
	}`

	constraints := ExtractConstraints(schema)
	if len(constraints) != 0 {
		t.Fatalf("invalid scopes should be ignored, got %d constraints: %+v", len(constraints), constraints)
	}
}

// --- Enforce nil data map ---

func TestEnforce_NilData(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	insertEntity(t, db, "e1", "org1", "test")

	constraints := []FieldConstraint{
		{FieldName: "email", Scope: ScopeInstance},
	}

	tx, _ := db.BeginTx(ctx, nil)
	err := Enforce(ctx, tx, "e1", "org1", constraints, nil)
	if err != nil {
		t.Fatalf("nil data map should be safely handled: %v", err)
	}
	commitTx(t, tx)
}

// --- Delete entity cascades unique_fields (via ON DELETE CASCADE) ---

func TestCascadeDelete(t *testing.T) {
	db := setupTestDB(t)
	ctx := context.Background()

	// SQLite needs PRAGMA foreign_keys = ON for CASCADE to work.
	db.Exec(`PRAGMA foreign_keys = ON`)

	insertEntity(t, db, "e1", "org1", "alice")

	tx, _ := db.BeginTx(ctx, nil)
	if err := Enforce(ctx, tx, "e1", "org1",
		[]FieldConstraint{{FieldName: "email", Scope: ScopeInstance}},
		map[string]any{"email": "alice@test.com"}); err != nil {
		t.Fatalf("enforce: %v", err)
	}
	commitTx(t, tx)

	// Verify unique_fields row exists.
	var count int
	db.QueryRow(`SELECT COUNT(*) FROM unique_fields WHERE user_id = 'e1'`).Scan(&count)
	if count != 1 {
		t.Fatalf("expected 1 unique_fields row, got %d", count)
	}

	// Delete entity.
	db.ExecContext(ctx, `DELETE FROM users WHERE id = 'e1'`)

	// unique_fields should be cascaded.
	db.QueryRow(`SELECT COUNT(*) FROM unique_fields WHERE user_id = 'e1'`).Scan(&count)
	if count != 0 {
		t.Errorf("unique_fields should be 0 after cascade delete, got %d", count)
	}
}
