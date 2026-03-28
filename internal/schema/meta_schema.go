// Package schema provides meta-schema validation for identity schema annotations.
package schema

import (
	"embed"
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
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
	Ref             string                   `json:"$ref,omitempty"`
	Version         string                   `json:"version,omitempty"`
	Group           string                   `json:"group"`
	Alias           string                   `json:"alias"`
	Singular        string                   `json:"singular"`
	Path            string                   `json:"path"`
	Icon            string                   `json:"icon"`
	SortOrder       int                      `json:"sort_order"`
	Required        bool                     `json:"required,omitempty"`
	Storage         string                   `json:"storage"`
	Route           string                   `json:"route,omitempty"`
	Nav             string                   `json:"nav,omitempty"`
	Countable       bool                     `json:"countable,omitempty"`
	Aggregates      []string                 `json:"aggregates,omitempty"`
	SeparatorBefore bool                     `json:"separator_before,omitempty"`
	Engines         map[string]EngineBinding `json:"engines,omitempty"`
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

		// 6. Group validation removed — flat nav, all items use group: "nav".
	}

	return errs
}

// schemaToTable maps JSON schema filenames to SQL table names.
var schemaToTable = map[string]string{
	"schemas/event.json":   "events",
	"schemas/session.json": "sessions",
}

// ValidateAgainstDDL cross-references JSON schema properties against the
// column definitions in a SQL DDL string (typically the initial migration).
// It reports drift in both directions:
//   - Column in DDL but missing from JSON schema → warning
//   - Property in JSON schema but missing from DDL → warning
//
// This is designed to be called from CI tests with the embedded migration SQL.
func ValidateAgainstDDL(ddl string) []LintError {
	// Parse all CREATE TABLE statements from the DDL.
	tableColumns := parseDDLColumns(ddl)
	if len(tableColumns) == 0 {
		return []LintError{{Level: "error", Message: "no CREATE TABLE statements found in DDL"}}
	}

	var errs []LintError

	for schemaFile, tableName := range schemaToTable {
		// Load JSON schema.
		schemaJSON, err := LoadSchemaFile(schemaFile)
		if err != nil {
			errs = append(errs, LintError{
				File: schemaFile, Level: "error",
				Message: fmt.Sprintf("cannot load schema file: %v", err),
			})
			continue
		}

		var schema struct {
			Properties map[string]any `json:"properties"`
		}
		if err := json.Unmarshal([]byte(schemaJSON), &schema); err != nil {
			errs = append(errs, LintError{
				File: schemaFile, Level: "error",
				Message: fmt.Sprintf("invalid JSON: %v", err),
			})
			continue
		}

		ddlCols, ok := tableColumns[tableName]
		if !ok {
			errs = append(errs, LintError{
				File: schemaFile, Level: "error",
				Message: fmt.Sprintf("table %q not found in DDL", tableName),
			})
			continue
		}

		// Build sets.
		schemaProps := make(map[string]bool)
		for prop := range schema.Properties {
			schemaProps[prop] = true
		}
		ddlSet := make(map[string]bool)
		for _, col := range ddlCols {
			ddlSet[col] = true
		}

		// Columns in DDL but not in JSON schema.
		for _, col := range ddlCols {
			if !schemaProps[col] {
				errs = append(errs, LintError{
					Type: tableName, File: schemaFile, Level: "warning",
					Message: fmt.Sprintf("column %q in DDL but missing from JSON schema", col),
				})
			}
		}

		// Properties in JSON schema but not in DDL.
		for prop := range schema.Properties {
			if !ddlSet[prop] {
				errs = append(errs, LintError{
					Type: tableName, File: schemaFile, Level: "warning",
					Message: fmt.Sprintf("property %q in JSON schema but missing from DDL", prop),
				})
			}
		}
	}

	return errs
}

// createTableRe matches "CREATE TABLE [IF NOT EXISTS] tablename ("
var createTableRe = regexp.MustCompile(`(?i)CREATE\s+TABLE\s+(?:IF\s+NOT\s+EXISTS\s+)?(\w+)\s*\(`)

// parseDDLColumns parses CREATE TABLE statements and returns a map of table name to column names.
func parseDDLColumns(ddl string) map[string][]string {
	result := make(map[string][]string)

	matches := createTableRe.FindAllStringSubmatchIndex(ddl, -1)
	for _, match := range matches {
		tableName := ddl[match[2]:match[3]]
		// Find the matching closing paren by counting nesting.
		start := match[1] // right after the opening (
		depth := 1
		end := start
		for end < len(ddl) && depth > 0 {
			switch ddl[end] {
			case '(':
				depth++
			case ')':
				depth--
			}
			if depth > 0 {
				end++
			}
		}
		if depth != 0 {
			continue
		}

		body := ddl[start:end]
		columns := parseColumnNames(body)
		result[tableName] = columns
	}

	return result
}

// parseColumnNames extracts column names from a CREATE TABLE body (between parens).
func parseColumnNames(body string) []string {
	var cols []string
	// Split by commas, but be aware of nested parens (e.g. DEFAULT (...)).
	lines := splitTopLevel(body)
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		// Skip constraints: PRIMARY KEY, FOREIGN KEY, UNIQUE, CHECK, INDEX
		upper := strings.ToUpper(line)
		if strings.HasPrefix(upper, "PRIMARY KEY") ||
			strings.HasPrefix(upper, "FOREIGN KEY") ||
			strings.HasPrefix(upper, "UNIQUE") ||
			strings.HasPrefix(upper, "CHECK") ||
			strings.HasPrefix(upper, "CONSTRAINT") {
			continue
		}
		// First word is the column name.
		fields := strings.Fields(line)
		if len(fields) >= 2 {
			cols = append(cols, fields[0])
		}
	}
	return cols
}

// splitTopLevel splits a string by commas, ignoring commas inside parentheses.
func splitTopLevel(s string) []string {
	var parts []string
	depth := 0
	start := 0
	for i := 0; i < len(s); i++ {
		switch s[i] {
		case '(':
			depth++
		case ')':
			depth--
		case ',':
			if depth == 0 {
				parts = append(parts, s[start:i])
				start = i + 1
			}
		}
	}
	if start < len(s) {
		parts = append(parts, s[start:])
	}
	return parts
}
