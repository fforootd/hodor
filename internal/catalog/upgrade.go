package catalog

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
)

// UpgradeReport is the result of a schema upgrade preview.
type UpgradeReport struct {
	SchemaType    string         `json:"schema_type"`
	TotalEntities int            `json:"total_entities"`
	Sampled       int            `json:"sampled"`
	Impact        ImpactSummary  `json:"impact"`
	FieldChanges  []FieldChange  `json:"field_changes"`
	SampleResults []EntityResult `json:"sample_entities"`
}

// ImpactSummary summarizes how many entities would be valid, warned, or broken.
type ImpactSummary struct {
	Valid    int `json:"valid"`
	Warnings int `json:"warnings"`
	Breaking int `json:"breaking"`
}

// FieldChange describes a structural difference between old and new schemas.
type FieldChange struct {
	Path             string `json:"path"`
	Change           string `json:"change"` // field_added, field_removed, type_changed, required_added, required_removed
	Description      string `json:"description"`
	Severity         string `json:"severity"` // info, warning, breaking
	AffectedEstimate int    `json:"affected_estimate,omitempty"`
}

// EntityResult shows how a specific entity would be affected.
type EntityResult struct {
	ID          string         `json:"id"`
	DisplayName string         `json:"display_name"`
	Status      string         `json:"status"` // valid, warning, breaking
	Changes     []EntityChange `json:"changes,omitempty"`
}

// EntityChange describes a specific issue with one entity under the new schema.
type EntityChange struct {
	Path         string `json:"path"`
	Issue        string `json:"issue"`
	CurrentValue any    `json:"current_value"`
	Suggestion   string `json:"suggestion,omitempty"`
}

// PreviewUpgrade analyzes the impact of changing a schema on existing entities.
// It samples entities and validates them against the proposed new schema.
func PreviewUpgrade(ctx context.Context, db *sql.DB, schemaType string, newSchema map[string]any, sampleSize int) (*UpgradeReport, error) {
	if sampleSize <= 0 {
		sampleSize = 10
	}
	if sampleSize > 100 {
		sampleSize = 100
	}

	// Count total entities of this type.
	var totalEntities int
	err := db.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM users i
		  JOIN schemas s ON i.schema_id = s.id
		 WHERE s.type = ?`, schemaType,
	).Scan(&totalEntities)
	if err != nil {
		return nil, fmt.Errorf("count entities: %w", err)
	}

	// Get the current schema for comparison.
	var currentSchemaJSON string
	err = db.QueryRowContext(ctx,
		`SELECT schema FROM schemas WHERE type = ? ORDER BY is_default DESC, version DESC LIMIT 1`, schemaType,
	).Scan(&currentSchemaJSON)

	var currentSchema map[string]any
	if err == nil {
		json.Unmarshal([]byte(currentSchemaJSON), &currentSchema)
	}

	// Compute structural field changes between old and new schema.
	fieldChanges := diffSchemas(currentSchema, newSchema)

	// Sample entities.
	rows, err := db.QueryContext(ctx,
		`SELECT i.id, i.display_name, i.metadata FROM users i
		  JOIN schemas s ON i.schema_id = s.id
		 WHERE s.type = ? ORDER BY RANDOM() LIMIT ?`,
		schemaType, sampleSize,
	)
	if err != nil {
		return nil, fmt.Errorf("sample entities: %w", err)
	}
	defer rows.Close()

	var results []EntityResult
	impact := ImpactSummary{}

	for rows.Next() {
		var id, displayName, dataJSON string
		if err := rows.Scan(&id, &displayName, &dataJSON); err != nil {
			continue
		}

		var entityData map[string]any
		if err := json.Unmarshal([]byte(dataJSON), &entityData); err != nil {
			continue
		}

		result := validateEntityAgainstSchema(id, displayName, entityData, newSchema, fieldChanges)
		switch result.Status {
		case "valid":
			impact.Valid++
		case "warning":
			impact.Warnings++
		case "breaking":
			impact.Breaking++
		}
		results = append(results, result)
	}
	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate entities: %w", err)
	}

	// Estimate affected counts for field changes.
	for i := range fieldChanges {
		fieldChanges[i].AffectedEstimate = estimateAffected(ctx, db, schemaType, fieldChanges[i], totalEntities)
	}

	return &UpgradeReport{
		SchemaType:    schemaType,
		TotalEntities: totalEntities,
		Sampled:       len(results),
		Impact:        impact,
		FieldChanges:  fieldChanges,
		SampleResults: results,
	}, nil
}

// diffSchemas compares old and new JSON schemas to find structural changes.
func diffSchemas(oldSchema, newSchema map[string]any) []FieldChange {
	var changes []FieldChange

	oldProps := extractProperties(oldSchema)
	newProps := extractProperties(newSchema)
	oldRequired := extractRequired(oldSchema)
	newRequired := extractRequired(newSchema)

	// Find added fields.
	for name, prop := range newProps {
		if _, exists := oldProps[name]; !exists {
			severity := "info"
			desc := fmt.Sprintf("New optional field '%s'", name)

			if newRequired[name] {
				severity = "breaking"
				desc = fmt.Sprintf("New required field '%s' added — existing entities without this field will be invalid", name)
			}

			propMap, _ := prop.(map[string]any)
			if defaultVal, ok := propMap["default"]; ok {
				desc += fmt.Sprintf(" (default: %v)", defaultVal)
				if severity == "breaking" {
					severity = "warning"
					desc = fmt.Sprintf("New required field '%s' with default '%v' — will apply automatically", name, defaultVal)
				}
			}

			changes = append(changes, FieldChange{
				Path:        "properties." + name,
				Change:      "field_added",
				Description: desc,
				Severity:    severity,
			})
		}
	}

	// Find removed fields.
	for name := range oldProps {
		if _, exists := newProps[name]; !exists {
			changes = append(changes, FieldChange{
				Path:        "properties." + name,
				Change:      "field_removed",
				Description: fmt.Sprintf("Field '%s' removed — existing data for this field will be orphaned", name),
				Severity:    "warning",
			})
		}
	}

	// Find type changes and newly required fields.
	for name, newProp := range newProps {
		oldProp, exists := oldProps[name]
		if !exists {
			continue
		}

		oldPropMap, _ := oldProp.(map[string]any)
		newPropMap, _ := newProp.(map[string]any)

		// Type change.
		oldType, _ := oldPropMap["type"].(string)
		newType, _ := newPropMap["type"].(string)
		if oldType != "" && newType != "" && oldType != newType {
			changes = append(changes, FieldChange{
				Path:        "properties." + name,
				Change:      "type_changed",
				Description: fmt.Sprintf("Field '%s' type changed from '%s' to '%s'", name, oldType, newType),
				Severity:    "breaking",
			})
		}

		// Newly required.
		if newRequired[name] && !oldRequired[name] {
			changes = append(changes, FieldChange{
				Path:        "properties." + name,
				Change:      "required_added",
				Description: fmt.Sprintf("Field '%s' is now required but was optional", name),
				Severity:    "breaking",
			})
		}

		// Newly optional.
		if !newRequired[name] && oldRequired[name] {
			changes = append(changes, FieldChange{
				Path:        "properties." + name,
				Change:      "required_removed",
				Description: fmt.Sprintf("Field '%s' is now optional (was required)", name),
				Severity:    "info",
			})
		}
	}

	return changes
}

// validateEntityAgainstSchema checks a single entity against the proposed schema.
func validateEntityAgainstSchema(id, displayName string, data, schema map[string]any, fieldChanges []FieldChange) EntityResult {
	result := EntityResult{
		ID:          id,
		DisplayName: displayName,
		Status:      "valid",
	}

	required := extractRequired(schema)
	props := extractProperties(schema)

	// Check required fields.
	for name := range required {
		val, exists := data[name]
		if !exists || val == nil || val == "" {
			result.Changes = append(result.Changes, EntityChange{
				Path:         name,
				Issue:        "required field missing",
				CurrentValue: val,
				Suggestion:   "Set a default value or make the field optional",
			})
			result.Status = "breaking"
		}
	}

	// Check type compatibility for existing fields.
	for name, propDef := range props {
		val, exists := data[name]
		if !exists || val == nil {
			continue
		}

		propMap, _ := propDef.(map[string]any)
		expectedType, _ := propMap["type"].(string)
		if expectedType == "" {
			continue
		}

		if !isTypeCompatible(val, expectedType) {
			result.Changes = append(result.Changes, EntityChange{
				Path:         name,
				Issue:        fmt.Sprintf("type mismatch: expected %s", expectedType),
				CurrentValue: val,
				Suggestion:   fmt.Sprintf("Convert value to %s or update schema", expectedType),
			})
			if result.Status != "breaking" {
				result.Status = "warning"
			}
		}
	}

	return result
}

// estimateAffected estimates how many entities are affected by a specific field change.
func estimateAffected(ctx context.Context, db *sql.DB, schemaType string, fc FieldChange, total int) int {
	fieldName := strings.TrimPrefix(fc.Path, "properties.")

	switch fc.Change {
	case "required_added", "field_added":
		if fc.Severity != "breaking" {
			return 0
		}
		// Count entities missing this field.
		var count int
		err := db.QueryRowContext(ctx,
			`SELECT COUNT(*) FROM users i
			  JOIN schemas s ON i.schema_id = s.id
			 WHERE s.type = ? AND (
				json_extract(i.metadata, '$.' || ?) IS NULL OR json_extract(i.metadata, '$.' || ?) = ''
			)`, schemaType, fieldName, fieldName,
		).Scan(&count)
		if err != nil {
			// fallback: assume all are affected
			return total
		}
		return count

	case "field_removed":
		// Count entities that have this field.
		var count int
		err := db.QueryRowContext(ctx,
			`SELECT COUNT(*) FROM users i
			  JOIN schemas s ON i.schema_id = s.id
			 WHERE s.type = ? AND json_extract(i.metadata, '$.' || ?) IS NOT NULL`,
			schemaType, fieldName,
		).Scan(&count)
		if err != nil {
			return 0
		}
		return count

	default:
		return total
	}
}

// extractProperties gets the properties map from a JSON schema.
func extractProperties(schema map[string]any) map[string]any {
	if schema == nil {
		return nil
	}
	props, _ := schema["properties"].(map[string]any)
	return props
}

// extractRequired gets the required fields as a set.
func extractRequired(schema map[string]any) map[string]bool {
	result := make(map[string]bool)
	if schema == nil {
		return result
	}
	required, _ := schema["required"].([]any)
	for _, r := range required {
		if name, ok := r.(string); ok {
			result[name] = true
		}
	}
	return result
}

// isTypeCompatible checks if a value matches the expected JSON schema type.
func isTypeCompatible(val any, expectedType string) bool {
	switch expectedType {
	case "string":
		_, ok := val.(string)
		return ok
	case "integer":
		switch v := val.(type) {
		case float64:
			return v == float64(int64(v))
		case int, int64:
			return true
		case json.Number:
			_, err := v.Int64()
			return err == nil
		}
		return false
	case "number":
		switch val.(type) {
		case float64, float32, int, int64, json.Number:
			return true
		}
		return false
	case "boolean":
		_, ok := val.(bool)
		return ok
	case "object":
		_, ok := val.(map[string]any)
		return ok
	case "array":
		_, ok := val.([]any)
		return ok
	default:
		return true
	}
}
