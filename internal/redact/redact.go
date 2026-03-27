// Package redact provides sensitive field redaction for event payloads.
// It reads x-sensitive annotations from entity schemas and masks
// marked fields with "***REDACTED***" before event emission.
package redact

import "encoding/json"

const RedactedValue = "***REDACTED***"

// SchemaProperties extracts the "properties" map from a JSON schema string.
func SchemaProperties(schemaJSON string) map[string]map[string]any {
	var schema struct {
		Properties map[string]map[string]any `json:"properties"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &schema); err != nil {
		return nil
	}
	return schema.Properties
}

// SensitiveFields returns the set of field names marked x-sensitive: true.
func SensitiveFields(schemaJSON string) map[string]bool {
	props := SchemaProperties(schemaJSON)
	if props == nil {
		return nil
	}
	result := make(map[string]bool)
	for name, def := range props {
		if sensitive, ok := def["x-sensitive"]; ok {
			if b, ok := sensitive.(bool); ok && b {
				result[name] = true
			}
		}
	}
	return result
}

// UserEditableFields returns the set of field names marked x-user-editable: true.
// If the annotation is absent, defaultEditable determines the default.
func UserEditableFields(schemaJSON string, defaultEditable bool) map[string]bool {
	props := SchemaProperties(schemaJSON)
	if props == nil {
		return nil
	}
	result := make(map[string]bool)
	for name, def := range props {
		if editable, ok := def["x-user-editable"]; ok {
			if b, ok := editable.(bool); ok {
				result[name] = b
				continue
			}
		}
		result[name] = defaultEditable
	}
	return result
}

// HiddenFields returns the set of field names marked x-hidden: true.
func HiddenFields(schemaJSON string) map[string]bool {
	props := SchemaProperties(schemaJSON)
	if props == nil {
		return nil
	}
	result := make(map[string]bool)
	for name, def := range props {
		if hidden, ok := def["x-hidden"]; ok {
			if b, ok := hidden.(bool); ok && b {
				result[name] = true
			}
		}
	}
	return result
}

// FieldSource returns the x-source annotation for a field, or "user" if absent.
func FieldSource(fieldDef map[string]any) string {
	if src, ok := fieldDef["x-source"]; ok {
		if s, ok := src.(string); ok {
			return s
		}
	}
	return "user"
}

// Payload redacts sensitive fields in a profile payload.
// Returns a new map with sensitive values replaced by RedactedValue.
func Payload(schemaJSON string, payload map[string]any) map[string]any {
	sensitive := SensitiveFields(schemaJSON)
	if len(sensitive) == 0 {
		return payload
	}

	redacted := make(map[string]any, len(payload))
	for k, v := range payload {
		if sensitive[k] {
			redacted[k] = RedactedValue
		} else {
			redacted[k] = v
		}
	}
	return redacted
}

// FieldPermissions builds a map of field name → {editable, sensitive, hidden, source}
// for use in the /v1/account/profile response.
func FieldPermissions(schemaJSON string, defaultEditable bool) map[string]map[string]any {
	props := SchemaProperties(schemaJSON)
	if props == nil {
		return nil
	}

	result := make(map[string]map[string]any, len(props))
	editable := UserEditableFields(schemaJSON, defaultEditable)
	sensitive := SensitiveFields(schemaJSON)
	hidden := HiddenFields(schemaJSON)

	for name, def := range props {
		result[name] = map[string]any{
			"editable":  editable[name],
			"sensitive": sensitive[name],
			"hidden":    hidden[name],
			"source":    FieldSource(def),
		}
	}
	return result
}
