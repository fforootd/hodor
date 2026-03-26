package redact

import "testing"

const testSchema = `{
  "type": "object",
  "properties": {
    "email":        {"type": "string", "x-user-editable": true},
    "phone":        {"type": "string", "x-sensitive": true, "x-user-editable": true},
    "ssn":          {"type": "string", "x-sensitive": true, "x-hidden": true},
    "display_name": {"type": "string", "x-user-editable": true},
    "metadata":     {"type": "object", "x-user-editable": false, "x-source": "admin"}
  }
}`

func TestSensitiveFields(t *testing.T) {
	fields := SensitiveFields(testSchema)
	if !fields["phone"] {
		t.Error("expected phone to be sensitive")
	}
	if !fields["ssn"] {
		t.Error("expected ssn to be sensitive")
	}
	if fields["email"] {
		t.Error("email should not be sensitive")
	}
	if fields["display_name"] {
		t.Error("display_name should not be sensitive")
	}
}

func TestSensitiveFields_InvalidJSON(t *testing.T) {
	fields := SensitiveFields("not json")
	if fields != nil {
		t.Error("expected nil for invalid JSON")
	}
}

func TestHiddenFields(t *testing.T) {
	fields := HiddenFields(testSchema)
	if !fields["ssn"] {
		t.Error("expected ssn to be hidden")
	}
	if fields["email"] {
		t.Error("email should not be hidden")
	}
}

func TestUserEditableFields(t *testing.T) {
	fields := UserEditableFields(testSchema, false)
	if !fields["email"] {
		t.Error("expected email to be editable")
	}
	if !fields["phone"] {
		t.Error("expected phone to be editable")
	}
	if fields["metadata"] {
		t.Error("metadata should not be editable")
	}
}

func TestUserEditableFields_DefaultTrue(t *testing.T) {
	// Schema with no x-user-editable annotation — default should be true.
	schema := `{"type":"object","properties":{"name":{"type":"string"}}}`
	fields := UserEditableFields(schema, true)
	if !fields["name"] {
		t.Error("expected name to be editable (default true)")
	}
}

func TestFieldSource(t *testing.T) {
	if s := FieldSource(map[string]any{"x-source": "admin"}); s != "admin" {
		t.Errorf("FieldSource = %q, want admin", s)
	}
	if s := FieldSource(map[string]any{}); s != "user" {
		t.Errorf("FieldSource = %q, want user (default)", s)
	}
}

func TestPayload_RedactsSensitive(t *testing.T) {
	payload := map[string]any{
		"email": "user@test.com",
		"phone": "+1-555-1234",
		"ssn":   "123-45-6789",
	}

	redacted := Payload(testSchema, payload)

	if redacted["email"] != "user@test.com" {
		t.Error("email should not be redacted")
	}
	if redacted["phone"] != RedactedValue {
		t.Errorf("phone = %v, want %v", redacted["phone"], RedactedValue)
	}
	if redacted["ssn"] != RedactedValue {
		t.Errorf("ssn = %v, want %v", redacted["ssn"], RedactedValue)
	}
}

func TestPayload_NoSensitiveFields(t *testing.T) {
	schema := `{"type":"object","properties":{"name":{"type":"string"}}}`
	payload := map[string]any{"name": "test"}
	result := Payload(schema, payload)
	// Should return the original payload (no copy needed).
	if result["name"] != "test" {
		t.Error("expected unchanged payload")
	}
}

func TestFieldPermissions(t *testing.T) {
	perms := FieldPermissions(testSchema, false)
	if perms == nil {
		t.Fatal("expected non-nil permissions map")
	}

	phonePerm := perms["phone"]
	if phonePerm["sensitive"] != true {
		t.Error("phone should be sensitive")
	}
	if phonePerm["editable"] != true {
		t.Error("phone should be editable")
	}

	metaPerm := perms["metadata"]
	if metaPerm["source"] != "admin" {
		t.Errorf("metadata source = %v, want admin", metaPerm["source"])
	}
}
