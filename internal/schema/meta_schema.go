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

// CatalogEntry represents a single entry in the x-catalog of the meta schema.
type CatalogEntry struct {
	SchemaFile string   `json:"schema_file,omitempty"`
	Group      string   `json:"group"`
	Alias      string   `json:"alias"`
	Singular   string   `json:"singular"`
	Path       string   `json:"path"`
	Icon       string   `json:"icon"`
	SortOrder  int      `json:"sort_order"`
	Required   bool     `json:"required,omitempty"`
	Storage    string   `json:"storage"`
	Route      string   `json:"route,omitempty"`
	Nav        string   `json:"nav,omitempty"`
	Components []string `json:"components,omitempty"`
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
