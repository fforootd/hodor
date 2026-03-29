package api

import (
	"encoding/json"
	"reflect"
	"sort"
	"strings"
)

// OpenAPIRegistry collects API operations and generates an OpenAPI 3.1 spec.
type OpenAPIRegistry struct {
	operations []OpenAPIOperation
}

// OpenAPIOperation describes a single API endpoint.
type OpenAPIOperation struct {
	Method      string // "GET", "POST", etc.
	Path        string // "/v1/entities/{id}"
	ID          string // operationId
	Summary     string
	Description string
	Tags        []string
	Request     any // Go struct → JSON Schema (nil for no body)
	Response    any // Go struct → JSON Schema (nil for 204s)
	StatusCode  int // Primary success status (200, 201, 204)
	PathParams  []OpenAPIParam
	QueryParams []OpenAPIParam
	Security    bool // true = requires auth
}

// OpenAPIParam describes a single path or query parameter.
type OpenAPIParam struct {
	Name        string
	Type        string // "string", "integer", "boolean"
	Required    bool
	Description string
}

// Add registers an operation.
func (r *OpenAPIRegistry) Add(op OpenAPIOperation) {
	if op.StatusCode == 0 {
		op.StatusCode = 200
	}
	r.operations = append(r.operations, op)
}

// Spec generates the complete OpenAPI 3.1 JSON document.
func (r *OpenAPIRegistry) Spec() map[string]any {
	paths := map[string]any{}
	schemas := map[string]any{}

	for _, op := range r.operations {
		pathItem, ok := paths[op.Path].(map[string]any)
		if !ok {
			pathItem = map[string]any{}
			paths[op.Path] = pathItem
		}

		operation := map[string]any{
			"operationId": op.ID,
			"summary":     op.Summary,
		}
		if op.Description != "" {
			operation["description"] = op.Description
		}
		if len(op.Tags) > 0 {
			operation["tags"] = op.Tags
		}

		// Parameters
		params := make([]map[string]any, 0, len(op.PathParams)+len(op.QueryParams))
		for _, p := range op.PathParams {
			params = append(params, map[string]any{
				"name":        p.Name,
				"in":          "path",
				"required":    true,
				"description": p.Description,
				"schema":      map[string]any{"type": p.Type},
			})
		}
		for _, p := range op.QueryParams {
			param := map[string]any{
				"name":   p.Name,
				"in":     "query",
				"schema": map[string]any{"type": p.Type},
			}
			if p.Required {
				param["required"] = true
			}
			if p.Description != "" {
				param["description"] = p.Description
			}
			params = append(params, param)
		}
		if len(params) > 0 {
			operation["parameters"] = params
		}

		// Request body
		if op.Request != nil {
			refName := structToSchema(op.Request, schemas)
			operation["requestBody"] = map[string]any{
				"required": true,
				"content": map[string]any{
					"application/json": map[string]any{
						"schema": map[string]any{"$ref": "#/components/schemas/" + refName},
					},
				},
			}
		}

		// Response
		responses := map[string]any{}
		statusStr := statusCodeStr(op.StatusCode)
		if op.Response != nil {
			refName := structToSchema(op.Response, schemas)
			responses[statusStr] = map[string]any{
				"description": op.Summary,
				"content": map[string]any{
					"application/json": map[string]any{
						"schema": map[string]any{"$ref": "#/components/schemas/" + refName},
					},
				},
			}
		} else {
			responses[statusStr] = map[string]any{
				"description": op.Summary,
			}
		}
		operation["responses"] = responses

		// Security
		if op.Security {
			operation["security"] = []map[string]any{
				{"bearerAuth": []string{}},
			}
		}

		pathItem[strings.ToLower(op.Method)] = operation
	}

	// Build tags list from unique tags.
	tagSet := map[string]bool{}
	for _, op := range r.operations {
		for _, t := range op.Tags {
			tagSet[t] = true
		}
	}
	tagNames := make([]string, 0, len(tagSet))
	for t := range tagSet {
		tagNames = append(tagNames, t)
	}
	sort.Strings(tagNames)
	tags := make([]map[string]any, 0, len(tagNames))
	for _, t := range tagNames {
		tags = append(tags, map[string]any{"name": t})
	}

	spec := map[string]any{
		"openapi": "3.1.0",
		"info": map[string]any{
			"title":       "Zitadel API",
			"version":     "0.1.0",
			"description": "Identity and access management API. Schema-driven, extensible, open source.",
		},
		"servers": []map[string]any{
			{"url": "/", "description": "This instance"},
		},
		"paths": paths,
		"tags":  tags,
		"components": map[string]any{
			"schemas": schemas,
			"securitySchemes": map[string]any{
				"bearerAuth": map[string]any{
					"type":         "http",
					"scheme":       "bearer",
					"bearerFormat": "PAT or session token",
				},
			},
		},
	}
	return spec
}

// SpecJSON returns the spec as indented JSON bytes.
func (r *OpenAPIRegistry) SpecJSON() ([]byte, error) {
	return json.MarshalIndent(r.Spec(), "", "  ")
}

// structToSchema converts a Go struct into a JSON Schema entry in the schemas
// map and returns the schema name (for $ref).
func structToSchema(v any, schemas map[string]any) string {
	t := reflect.TypeOf(v)
	if t.Kind() == reflect.Ptr {
		t = t.Elem()
	}

	name := t.Name()
	if name == "" {
		name = "InlineObject"
	}

	// Check if already registered.
	if _, exists := schemas[name]; exists {
		return name
	}

	// Handle map types (like CountsResponse).
	if t.Kind() == reflect.Map {
		schemas[name] = map[string]any{
			"type":                 "object",
			"additionalProperties": goTypeToSchema(t.Elem()),
		}
		return name
	}

	if t.Kind() != reflect.Struct {
		schemas[name] = map[string]any{"type": goKindToType(t.Kind())}
		return name
	}

	props := map[string]any{}
	var required []string

	for i := 0; i < t.NumField(); i++ {
		field := t.Field(i)
		if !field.IsExported() {
			continue
		}

		tag := field.Tag.Get("json")
		if tag == "-" {
			continue
		}

		jsonName, opts := parseJSONTag(tag)
		if jsonName == "" {
			jsonName = field.Name
		}

		prop := goTypeToSchema(field.Type)

		// Add format hints.
		if strings.HasSuffix(jsonName, "_at") {
			prop["format"] = "date-time"
		}

		props[jsonName] = prop

		// Fields without omitempty are required.
		if !opts.omitempty {
			required = append(required, jsonName)
		}
	}

	schema := map[string]any{
		"type":       "object",
		"properties": props,
	}
	if len(required) > 0 {
		schema["required"] = required
	}
	schemas[name] = schema
	return name
}

// goTypeToSchema converts a Go reflect.Type to a JSON Schema property.
func goTypeToSchema(t reflect.Type) map[string]any {
	switch t.Kind() {
	case reflect.String:
		return map[string]any{"type": "string"}
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return map[string]any{"type": "integer"}
	case reflect.Float32, reflect.Float64:
		return map[string]any{"type": "number"}
	case reflect.Bool:
		return map[string]any{"type": "boolean"}
	case reflect.Slice:
		return map[string]any{
			"type":  "array",
			"items": goTypeToSchema(t.Elem()),
		}
	case reflect.Map:
		return map[string]any{
			"type":                 "object",
			"additionalProperties": goTypeToSchema(t.Elem()),
		}
	case reflect.Ptr:
		return goTypeToSchema(t.Elem())
	case reflect.Interface:
		// any/interface{} → freeform object
		return map[string]any{}
	case reflect.Struct:
		// Nested struct — inline its properties.
		props := map[string]any{}
		for i := 0; i < t.NumField(); i++ {
			f := t.Field(i)
			if !f.IsExported() {
				continue
			}
			tag := f.Tag.Get("json")
			if tag == "-" {
				continue
			}
			name, _ := parseJSONTag(tag)
			if name == "" {
				name = f.Name
			}
			props[name] = goTypeToSchema(f.Type)
		}
		return map[string]any{
			"type":       "object",
			"properties": props,
		}
	default:
		return map[string]any{"type": "string"}
	}
}

func goKindToType(k reflect.Kind) string {
	switch k {
	case reflect.String:
		return "string"
	case reflect.Int, reflect.Int8, reflect.Int16, reflect.Int32, reflect.Int64:
		return "integer"
	case reflect.Float32, reflect.Float64:
		return "number"
	case reflect.Bool:
		return "boolean"
	default:
		return "string"
	}
}

type jsonTagOpts struct {
	omitempty bool
}

func parseJSONTag(tag string) (string, jsonTagOpts) {
	parts := strings.Split(tag, ",")
	name := parts[0]
	opts := jsonTagOpts{}
	for _, p := range parts[1:] {
		if p == "omitempty" {
			opts.omitempty = true
		}
	}
	return name, opts
}

func statusCodeStr(code int) string {
	switch code {
	case 200:
		return "200"
	case 201:
		return "201"
	case 204:
		return "204"
	default:
		return "200"
	}
}
