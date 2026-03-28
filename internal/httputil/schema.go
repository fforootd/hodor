package httputil

import "encoding/json"

// SchemaFieldInfo describes a parsed property from a JSON Schema.
type SchemaFieldInfo struct {
	Name        string   `json:"name"`
	Type        string   `json:"type"`
	Format      string   `json:"format,omitempty"`
	Description string   `json:"description,omitempty"`
	Enum        []string `json:"enum,omitempty"`
	Required    bool     `json:"required"`
}

// ParseSchemaFields extracts property metadata from a JSON Schema definition.
// Handles "properties", "required", "type", "format", "description", and "enum".
func ParseSchemaFields(schemaJSON string) []SchemaFieldInfo {
	var schemaDef map[string]any
	if json.Unmarshal([]byte(schemaJSON), &schemaDef) != nil {
		return nil
	}

	requiredSet := map[string]bool{}
	if reqArr, ok := schemaDef["required"].([]any); ok {
		for _, r := range reqArr {
			if s, ok := r.(string); ok {
				requiredSet[s] = true
			}
		}
	}

	props, ok := schemaDef["properties"].(map[string]any)
	if !ok {
		return nil
	}

	fields := make([]SchemaFieldInfo, 0, len(props))
	for name, def := range props {
		f := SchemaFieldInfo{
			Name:     name,
			Type:     "any",
			Required: requiredSet[name],
		}
		if defMap, ok := def.(map[string]any); ok {
			if t, ok := defMap["type"].(string); ok {
				f.Type = t
			}
			if _, hasEnum := defMap["enum"]; hasEnum {
				f.Type = "enum"
			}
			if fmt, ok := defMap["format"].(string); ok {
				f.Format = fmt
			}
			if desc, ok := defMap["description"].(string); ok {
				f.Description = desc
			}
			if enums, ok := defMap["enum"].([]any); ok {
				for _, e := range enums {
					if s, ok := e.(string); ok {
						f.Enum = append(f.Enum, s)
					}
				}
			}
		}
		fields = append(fields, f)
	}
	return fields
}
