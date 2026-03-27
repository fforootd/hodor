// Package schema provides meta-schema validation for identity schema annotations.
package schema

import (
	"embed"
	"encoding/json"
	"fmt"
)

// MetaSchema is the JSON Schema that validates x-* annotations on entity schemas.
//
//go:embed meta_schema.json
var MetaSchema string

// SchemaFiles embeds all standalone schema JSON files.
//
//go:embed schemas/*.json
var SchemaFiles embed.FS

// EngineBinding describes how an engine processes entities of this type.
type EngineBinding struct {
	Model     string   `json:"model"`
	Relations []string `json:"relations,omitempty"`
	Direction string   `json:"direction,omitempty"`
}

// CatalogEntry represents a single entry in the x-catalog of the meta schema.
type CatalogEntry struct {
	Ref       string                   `json:"$ref,omitempty"`
	Version   string                   `json:"version,omitempty"`
	Group     string                   `json:"group"`
	Alias     string                   `json:"alias"`
	Singular  string                   `json:"singular"`
	Path      string                   `json:"path"`
	Icon      string                   `json:"icon"`
	SortOrder int                      `json:"sort_order"`
	Required  bool                     `json:"required,omitempty"`
	Storage   string                   `json:"storage"`
	Route     string                   `json:"route,omitempty"`
	Nav       string                   `json:"nav,omitempty"`
	Engines   map[string]EngineBinding `json:"engines,omitempty"`
}

// GroupEntry represents a nav group definition from x-groups.
type GroupEntry struct {
	Label     string `json:"label"`
	Icon      string `json:"icon"`
	SortOrder int    `json:"sort_order"`
	Nav       string `json:"nav,omitempty"`
}

// Catalog returns the parsed x-catalog from the meta schema.
func Catalog() (map[string]CatalogEntry, error) {
	var meta struct {
		Catalog map[string]CatalogEntry `json:"x-catalog"`
	}
	if err := json.Unmarshal([]byte(MetaSchema), &meta); err != nil {
		return nil, fmt.Errorf("parse meta schema catalog: %w", err)
	}
	return meta.Catalog, nil
}

// Groups returns the parsed x-groups from the meta schema.
func Groups() (map[string]GroupEntry, error) {
	var meta struct {
		Groups map[string]GroupEntry `json:"x-groups"`
	}
	if err := json.Unmarshal([]byte(MetaSchema), &meta); err != nil {
		return nil, fmt.Errorf("parse meta schema groups: %w", err)
	}
	return meta.Groups, nil
}

// LoadSchemaFile reads a schema JSON file from the embedded filesystem.
func LoadSchemaFile(path string) (string, error) {
	data, err := SchemaFiles.ReadFile(path)
	if err != nil {
		return "", fmt.Errorf("read schema file %s: %w", path, err)
	}
	return string(data), nil
}

// LintError represents a single schema lint issue.
type LintError struct {
	Type    string `json:"type"`    // catalog type name
	File    string `json:"file"`    // schema file path
	Level   string `json:"level"`   // "error" or "warning"
	Message string `json:"message"` // description of the issue
}

func (e LintError) Error() string {
	return fmt.Sprintf("[%s] %s: %s (%s)", e.Level, e.Type, e.Message, e.File)
}

// ValidateCatalog lints all catalog entries and their referenced schema files.
// Checks:
//   - Every $ref resolves to a real embedded file
//   - Every schema has a $version that matches the catalog version
//   - Every schema has a $schema reference
//   - Entity-storage schemas have required fields
func ValidateCatalog() []LintError {
	catalog, err := Catalog()
	if err != nil {
		return []LintError{{Level: "error", Message: fmt.Sprintf("cannot parse catalog: %v", err)}}
	}

	var errs []LintError

	for typeName, entry := range catalog {
		// Skip entries without $ref (system views like "schema").
		if entry.Ref == "" {
			continue
		}

		// 1. Check $ref resolves.
		schemaJSON, err := LoadSchemaFile(entry.Ref)
		if err != nil {
			errs = append(errs, LintError{
				Type: typeName, File: entry.Ref, Level: "error",
				Message: fmt.Sprintf("$ref does not resolve: %v", err),
			})
			continue
		}

		// 2. Parse schema.
		var schema map[string]any
		if err := json.Unmarshal([]byte(schemaJSON), &schema); err != nil {
			errs = append(errs, LintError{
				Type: typeName, File: entry.Ref, Level: "error",
				Message: fmt.Sprintf("invalid JSON: %v", err),
			})
			continue
		}

		// 3. Check $schema reference.
		if _, ok := schema["$schema"]; !ok {
			errs = append(errs, LintError{
				Type: typeName, File: entry.Ref, Level: "warning",
				Message: "missing $schema reference",
			})
		}

		// 4. Check $version matches catalog.
		fileVersion, _ := schema["$version"].(string)
		if fileVersion == "" {
			errs = append(errs, LintError{
				Type: typeName, File: entry.Ref, Level: "warning",
				Message: "missing $version",
			})
		} else if entry.Version != "" && fileVersion != entry.Version {
			errs = append(errs, LintError{
				Type: typeName, File: entry.Ref, Level: "error",
				Message: fmt.Sprintf("version mismatch: file=%s catalog=%s", fileVersion, entry.Version),
			})
		}

		// 5. Entity schemas should have "properties".
		if entry.Storage == "entities" {
			if _, ok := schema["properties"]; !ok {
				errs = append(errs, LintError{
					Type: typeName, File: entry.Ref, Level: "warning",
					Message: "entity schema missing 'properties'",
				})
			}
		}

		// 6. Check group is valid.
		groups, _ := Groups()
		if groups != nil {
			if _, ok := groups[entry.Group]; !ok {
				errs = append(errs, LintError{
					Type: typeName, File: entry.Ref, Level: "error",
					Message: fmt.Sprintf("group %q not defined in x-groups", entry.Group),
				})
			}
		}
	}

	return errs
}

