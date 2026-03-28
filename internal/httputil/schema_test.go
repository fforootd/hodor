package httputil

import "testing"

func TestParseSchemaFields_BasicProperties(t *testing.T) {
	schema := `{
		"type": "object",
		"required": ["username", "email"],
		"properties": {
			"username": {"type": "string", "description": "Login name"},
			"email": {"type": "string", "format": "email"},
			"age": {"type": "integer"},
			"role": {"type": "string", "enum": ["admin", "user", "viewer"]}
		}
	}`

	fields := ParseSchemaFields(schema)
	if fields == nil {
		t.Fatal("expected non-nil fields")
	}

	fieldMap := map[string]SchemaFieldInfo{}
	for _, f := range fields {
		fieldMap[f.Name] = f
	}

	// username: required, string
	u := fieldMap["username"]
	if u.Type != "string" {
		t.Errorf("username type = %q, want string", u.Type)
	}
	if !u.Required {
		t.Error("username should be required")
	}
	if u.Description != "Login name" {
		t.Errorf("username description = %q, want 'Login name'", u.Description)
	}

	// email: required, string, format email
	e := fieldMap["email"]
	if e.Type != "string" {
		t.Errorf("email type = %q, want string", e.Type)
	}
	if !e.Required {
		t.Error("email should be required")
	}
	if e.Format != "email" {
		t.Errorf("email format = %q, want email", e.Format)
	}

	// age: not required, integer
	a := fieldMap["age"]
	if a.Type != "integer" {
		t.Errorf("age type = %q, want integer", a.Type)
	}
	if a.Required {
		t.Error("age should NOT be required")
	}

	// role: enum
	r := fieldMap["role"]
	if r.Type != "enum" {
		t.Errorf("role type = %q, want enum", r.Type)
	}
	if len(r.Enum) != 3 {
		t.Errorf("role enum len = %d, want 3", len(r.Enum))
	}
}

func TestParseSchemaFields_EmptySchema(t *testing.T) {
	fields := ParseSchemaFields(`{}`)
	if fields != nil {
		t.Errorf("expected nil for schema without properties, got %v", fields)
	}
}

func TestParseSchemaFields_InvalidJSON(t *testing.T) {
	fields := ParseSchemaFields(`not json`)
	if fields != nil {
		t.Errorf("expected nil for invalid JSON, got %v", fields)
	}
}

func TestParseSchemaFields_NoRequired(t *testing.T) {
	schema := `{
		"type": "object",
		"properties": {
			"name": {"type": "string"}
		}
	}`
	fields := ParseSchemaFields(schema)
	if len(fields) != 1 {
		t.Fatalf("expected 1 field, got %d", len(fields))
	}
	if fields[0].Required {
		t.Error("name should not be required when no required array")
	}
}

func TestParseSchemaFields_PropertyWithoutType(t *testing.T) {
	schema := `{
		"type": "object",
		"properties": {
			"custom": {}
		}
	}`
	fields := ParseSchemaFields(schema)
	if len(fields) != 1 {
		t.Fatalf("expected 1 field, got %d", len(fields))
	}
	if fields[0].Type != "any" {
		t.Errorf("type = %q, want 'any' for untyped property", fields[0].Type)
	}
}
