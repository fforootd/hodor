package login

import (
	"testing"
)

func TestClaimMappings_ExtractsFromSchema(t *testing.T) {
	schema := `{
		"type": "object",
		"properties": {
			"email": {"type": "string", "x-claim": "claims.email"},
			"name":  {"type": "string", "x-claim": "claims.name ?? claims.preferred_username"},
			"phone": {"type": "string"}
		}
	}`

	mappings := ClaimMappings(schema)
	if len(mappings) != 2 {
		t.Fatalf("expected 2 mappings, got %d", len(mappings))
	}
	if mappings["email"] != "claims.email" {
		t.Errorf("email mapping: got %q", mappings["email"])
	}
	if mappings["name"] != "claims.name ?? claims.preferred_username" {
		t.Errorf("name mapping: got %q", mappings["name"])
	}
	if _, ok := mappings["phone"]; ok {
		t.Error("phone should not have a mapping")
	}
}

func TestClaimMappings_EmptySchema(t *testing.T) {
	mappings := ClaimMappings("{}")
	if mappings == nil {
		t.Fatal("expected non-nil map")
	}
	if len(mappings) != 0 {
		t.Fatalf("expected 0 mappings, got %d", len(mappings))
	}
}

func TestClaimMappings_InvalidJSON(t *testing.T) {
	mappings := ClaimMappings("not json")
	if mappings != nil {
		t.Fatal("expected nil for invalid JSON")
	}
}

func TestMapClaims_SchemaDefaultsOnly(t *testing.T) {
	schema := `{
		"type": "object",
		"properties": {
			"email": {"type": "string", "x-claim": "claims.email"},
			"name":  {"type": "string", "x-claim": "claims.name"}
		}
	}`

	claims := map[string]any{
		"email": "jane@example.com",
		"name":  "Jane Doe",
	}

	profile, err := MapClaims(schema, nil, claims)
	if err != nil {
		t.Fatal(err)
	}
	if profile["email"] != "jane@example.com" {
		t.Errorf("email: got %q", profile["email"])
	}
	if profile["name"] != "Jane Doe" {
		t.Errorf("name: got %q", profile["name"])
	}
}

func TestMapClaims_ProviderOverridesWin(t *testing.T) {
	schema := `{
		"type": "object",
		"properties": {
			"email": {"type": "string", "x-claim": "claims.email"}
		}
	}`

	overrides := map[string]string{
		"email": "claims.preferred_username",
	}

	claims := map[string]any{
		"email":              "jane@example.com",
		"preferred_username": "jane@corp.com",
	}

	profile, err := MapClaims(schema, overrides, claims)
	if err != nil {
		t.Fatal(err)
	}
	if profile["email"] != "jane@corp.com" {
		t.Errorf("expected override to win, got %q", profile["email"])
	}
}

func TestMapClaims_FallbackOperator(t *testing.T) {
	schema := `{
		"type": "object",
		"properties": {
			"name": {"type": "string", "x-claim": "claims.name ?? (claims.given_name + ' ' + claims.family_name)"}
		}
	}`

	// Case 1: "name" claim present.
	profile, _ := MapClaims(schema, nil, map[string]any{"name": "Jane Doe"})
	if profile["name"] != "Jane Doe" {
		t.Errorf("case 1: got %q", profile["name"])
	}

	// Case 2: "name" missing, fallback to given + family.
	profile, _ = MapClaims(schema, nil, map[string]any{
		"given_name":  "Jane",
		"family_name": "Doe",
	})
	if profile["name"] != "Jane Doe" {
		t.Errorf("case 2: got %q", profile["name"])
	}
}

func TestMapClaims_FailingExprSkipped(t *testing.T) {
	schema := `{
		"type": "object",
		"properties": {
			"email": {"type": "string", "x-claim": "claims.email"},
			"bad":   {"type": "string", "x-claim": "claims.nonexistent.nested.deep"}
		}
	}`

	profile, err := MapClaims(schema, nil, map[string]any{"email": "ok@test.com"})
	if err != nil {
		t.Fatal(err)
	}
	if profile["email"] != "ok@test.com" {
		t.Error("email should still be mapped")
	}
	if _, ok := profile["bad"]; ok {
		t.Error("bad field should have been skipped")
	}
}

func TestMapClaims_EmptyClaims(t *testing.T) {
	schema := `{
		"type": "object",
		"properties": {
			"email": {"type": "string", "x-claim": "claims.email ?? ''"}
		}
	}`

	profile, err := MapClaims(schema, nil, map[string]any{})
	if err != nil {
		t.Fatal(err)
	}
	// Empty string result should be excluded from profile.
	if _, ok := profile["email"]; ok {
		t.Error("empty string should be excluded from profile")
	}
}

func TestParseIDTokenClaims_Valid(t *testing.T) {
	// Manually construct a JWT with known payload. Header and sig don't matter for parsing.
	// payload: {"sub":"123","email":"test@test.com"}
	// base64url: eyJzdWIiOiIxMjMiLCJlbWFpbCI6InRlc3RAdGVzdC5jb20ifQ
	token := "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMiLCJlbWFpbCI6InRlc3RAdGVzdC5jb20ifQ.fakesig"

	claims, err := parseIDTokenClaims(token)
	if err != nil {
		t.Fatal(err)
	}
	if claims["sub"] != "123" {
		t.Errorf("sub: got %q", claims["sub"])
	}
	if claims["email"] != "test@test.com" {
		t.Errorf("email: got %q", claims["email"])
	}
}

func TestParseIDTokenClaims_InvalidJWT(t *testing.T) {
	_, err := parseIDTokenClaims("not.a.valid")
	if err == nil {
		t.Error("expected error for invalid JWT")
	}

	_, err = parseIDTokenClaims("only-one-part")
	if err == nil {
		t.Error("expected error for single-part token")
	}
}

func TestRandomString(t *testing.T) {
	a := randomString(32)
	b := randomString(32)
	if len(a) != 32 {
		t.Errorf("expected length 32, got %d", len(a))
	}
	if a == b {
		t.Error("two random strings should differ")
	}
}

func TestSha256URLSafe(t *testing.T) {
	result := sha256URLSafe("hello")
	if result == "" {
		t.Error("expected non-empty result")
	}
	// Should be URL-safe base64 (no +, /, =).
	for _, c := range result {
		if c == '+' || c == '/' || c == '=' {
			t.Errorf("non-URL-safe character: %c", c)
		}
	}
}
