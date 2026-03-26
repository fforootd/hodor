package lake

import (
	"testing"
)

func TestFlattenPayload_PromotesFields(t *testing.T) {
	payload := `{"identifier":"user@test.com","ip_address":"1.2.3.4","user_agent":"Chrome","auth_method":"password","reason":"login","extra_field":"value"}`

	fp := flattenPayload(payload)

	if fp.Identifier != "user@test.com" {
		t.Errorf("Identifier = %q, want user@test.com", fp.Identifier)
	}
	if fp.IPAddress != "1.2.3.4" {
		t.Errorf("IPAddress = %q, want 1.2.3.4", fp.IPAddress)
	}
	if fp.UserAgent != "Chrome" {
		t.Errorf("UserAgent = %q, want Chrome", fp.UserAgent)
	}
	if fp.AuthMethod != "password" {
		t.Errorf("AuthMethod = %q, want password", fp.AuthMethod)
	}
	if fp.Reason != "login" {
		t.Errorf("Reason = %q, want login", fp.Reason)
	}
	// extra_field should be in the Extra JSON.
	if fp.Extra == "" {
		t.Fatal("expected non-empty Extra for remaining fields")
	}
	if fp.Extra != `{"extra_field":"value"}` {
		t.Errorf("Extra = %q, want remaining JSON", fp.Extra)
	}
}

func TestFlattenPayload_EmptyPayload(t *testing.T) {
	fp := flattenPayload("")
	if fp.Identifier != "" || fp.Extra != "" {
		t.Error("expected all empty for empty payload")
	}

	fp2 := flattenPayload("{}")
	if fp2.Identifier != "" || fp2.Extra != "" {
		t.Error("expected all empty for {} payload")
	}
}

func TestFlattenPayload_InvalidJSON(t *testing.T) {
	fp := flattenPayload("not json")
	if fp.Extra != "not json" {
		t.Errorf("Extra = %q, want raw input as fallback", fp.Extra)
	}
}

func TestFlattenPayload_NoPromotedFields(t *testing.T) {
	fp := flattenPayload(`{"custom":"value","other":123}`)
	if fp.Identifier != "" {
		t.Error("expected empty Identifier")
	}
	if fp.Extra == "" {
		t.Error("expected non-empty Extra")
	}
}

func TestEventSchema_HasExpectedFields(t *testing.T) {
	// Verify the Arrow schema has the expected number of fields.
	if eventSchema.NumFields() != 16 {
		t.Errorf("eventSchema has %d fields, want 16", eventSchema.NumFields())
	}

	// Verify key field names.
	names := make(map[string]bool)
	for i := 0; i < eventSchema.NumFields(); i++ {
		names[eventSchema.Field(i).Name] = true
	}

	required := []string{"event_id", "event_type", "org_id", "actor_id", "aggregate_id",
		"trace_id", "identifier", "ip_address", "payload_extra", "created_at"}
	for _, name := range required {
		if !names[name] {
			t.Errorf("eventSchema missing field %q", name)
		}
	}
}
