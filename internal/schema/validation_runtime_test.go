package schema

import "testing"

func TestValidateSchemaDocument_BuiltInHumanUser(t *testing.T) {
	humanUser, err := LoadSchemaFile("schemas/human_user.json")
	if err != nil {
		t.Fatalf("LoadSchemaFile() error = %v", err)
	}

	if err := ValidateSchemaDocument([]byte(humanUser)); err != nil {
		t.Fatalf("ValidateSchemaDocument() error = %v", err)
	}
}

func TestValidateSchemaDocument_InvalidAuthMethod(t *testing.T) {
	err := ValidateSchemaDocument([]byte(`{
		"type": "object",
		"x-auth-methods": {
			"oauth2": {"enabled": true, "interactive": true}
		},
		"properties": {}
	}`))
	if err == nil {
		t.Fatal("expected validation error for unsupported auth method")
	}
}

func TestMaterializeUserDataMapsIdentifierIntoSchemaFields(t *testing.T) {
	humanUser, err := LoadSchemaFile("schemas/human_user.json")
	if err != nil {
		t.Fatalf("LoadSchemaFile() error = %v", err)
	}

	payload := MaterializeUserData(humanUser, "alice@example.com", "Alice", map[string]any{
		"locale": "en-US",
	})
	if got := payload["email"]; got != "alice@example.com" {
		t.Fatalf("payload[email] = %v, want alice@example.com", got)
	}
	if got := payload["display_name"]; got != "Alice" {
		t.Fatalf("payload[display_name] = %v, want Alice", got)
	}
	if got := payload["locale"]; got != "en-US" {
		t.Fatalf("payload[locale] = %v, want en-US", got)
	}

	if err := ValidateData(humanUser, payload); err != nil {
		t.Fatalf("ValidateData() error = %v", err)
	}
}
