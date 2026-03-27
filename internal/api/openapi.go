package api

import (
	"encoding/json"
	"net/http"
)

// openAPISpec serves a dynamically generated OpenAPI 3.1 spec.
// The Identity data schema is composed from the schemas table at runtime.
func (a *API) openAPISpec(w http.ResponseWriter, r *http.Request) {
	// Load entity schemas from the registry.
	var dataSchema any
	var schemaStr string
	err := a.db.SQL().QueryRowContext(r.Context(),
		`SELECT schema FROM schemas WHERE type = 'identity' ORDER BY version DESC LIMIT 1`,
	).Scan(&schemaStr)
	if err == nil {
		json.Unmarshal([]byte(schemaStr), &dataSchema)
	}

	// If no schema registered, use a permissive schema.
	if dataSchema == nil {
		dataSchema = map[string]any{
			"type":                 "object",
			"additionalProperties": true,
			"description":          "Arbitrary identity data. Register a schema via POST /v1/schemas to define structure.",
		}
	}

	spec := map[string]any{
		"openapi": "3.1.0",
		"info": map[string]any{
			"title":       "Zitadel v2 API",
			"version":     "0.1.0",
			"description": "Identity and access management API with flexible schemas.",
		},
		"servers": []map[string]any{
			{"url": "/", "description": "This instance"},
		},
		"paths": map[string]any{
			"/v1/entities": map[string]any{
				"get": map[string]any{
					"summary":     "List entities",
					"operationId": "listIdentities",
					"parameters": []map[string]any{
						{"name": "cursor", "in": "query", "schema": map[string]any{"type": "string"}},
						{"name": "limit", "in": "query", "schema": map[string]any{"type": "integer", "default": 50}},
					},
					"responses": map[string]any{
						"200": map[string]any{
							"description": "List of entities",
							"content": map[string]any{
								"application/json": map[string]any{
									"schema": map[string]any{"$ref": "#/components/schemas/ListIdentitiesResponse"},
								},
							},
						},
					},
				},
				"post": map[string]any{
					"summary":     "Create an identity",
					"operationId": "createEntity",
					"requestBody": map[string]any{
						"required": true,
						"content": map[string]any{
							"application/json": map[string]any{
								"schema": map[string]any{"$ref": "#/components/schemas/CreateIdentityRequest"},
							},
						},
					},
					"responses": map[string]any{
						"201": map[string]any{
							"description": "Identity created",
							"content": map[string]any{
								"application/json": map[string]any{
									"schema": map[string]any{"$ref": "#/components/schemas/Entity"},
								},
							},
						},
					},
				},
			},
			"/v1/entities/{id}": map[string]any{
				"get": map[string]any{
					"summary":     "Get an identity",
					"operationId": "getEntity",
					"parameters": []map[string]any{
						{"name": "id", "in": "path", "required": true, "schema": map[string]any{"type": "integer"}},
					},
					"responses": map[string]any{
						"200": map[string]any{
							"description": "Identity found",
							"content": map[string]any{
								"application/json": map[string]any{
									"schema": map[string]any{"$ref": "#/components/schemas/Entity"},
								},
							},
						},
					},
				},
				"patch": map[string]any{
					"summary":     "Update an identity",
					"operationId": "updateEntity",
					"parameters": []map[string]any{
						{"name": "id", "in": "path", "required": true, "schema": map[string]any{"type": "integer"}},
					},
					"requestBody": map[string]any{
						"content": map[string]any{
							"application/json": map[string]any{
								"schema": map[string]any{"$ref": "#/components/schemas/UpdateIdentityRequest"},
							},
						},
					},
					"responses": map[string]any{
						"200": map[string]any{
							"description": "Identity updated",
							"content": map[string]any{
								"application/json": map[string]any{
									"schema": map[string]any{"$ref": "#/components/schemas/Entity"},
								},
							},
						},
					},
				},
				"delete": map[string]any{
					"summary":     "Delete an identity",
					"operationId": "deleteEntity",
					"parameters": []map[string]any{
						{"name": "id", "in": "path", "required": true, "schema": map[string]any{"type": "integer"}},
					},
					"responses": map[string]any{
						"204": map[string]any{"description": "Identity deleted"},
					},
				},
			},
			"/v1/schemas": map[string]any{
				"get": map[string]any{
					"summary":     "List schemas",
					"operationId": "listSchemas",
					"parameters": []map[string]any{
						{"name": "type", "in": "query", "schema": map[string]any{"type": "string"}},
					},
					"responses": map[string]any{
						"200": map[string]any{
							"description": "List of schemas",
							"content": map[string]any{
								"application/json": map[string]any{
									"schema": map[string]any{"$ref": "#/components/schemas/ListSchemasResponse"},
								},
							},
						},
					},
				},
				"post": map[string]any{
					"summary":     "Create or update a schema",
					"operationId": "createSchema",
					"requestBody": map[string]any{
						"required": true,
						"content": map[string]any{
							"application/json": map[string]any{
								"schema": map[string]any{"$ref": "#/components/schemas/CreateSchemaRequest"},
							},
						},
					},
					"responses": map[string]any{
						"201": map[string]any{
							"description": "Schema created",
							"content": map[string]any{
								"application/json": map[string]any{
									"schema": map[string]any{"$ref": "#/components/schemas/SchemaObject"},
								},
							},
						},
					},
				},
			},
			"/v1/schemas/{id}": map[string]any{
				"get": map[string]any{
					"summary":     "Get a schema",
					"operationId": "getSchema",
					"parameters": []map[string]any{
						{"name": "id", "in": "path", "required": true, "schema": map[string]any{"type": "string"}},
					},
					"responses": map[string]any{
						"200": map[string]any{
							"description": "Schema found",
							"content": map[string]any{
								"application/json": map[string]any{
									"schema": map[string]any{"$ref": "#/components/schemas/SchemaObject"},
								},
							},
						},
					},
				},
			},
		},
		"components": map[string]any{
			"schemas": map[string]any{
				"Entity": map[string]any{
					"type":     "object",
					"required": []string{"id", "identifier", "state"},
					"properties": map[string]any{
						"id":           map[string]any{"type": "integer", "format": "int64"},
						"org_id":       map[string]any{"type": "integer", "format": "int64"},
						"identifier":   map[string]any{"type": "string"},
						"state":        map[string]any{"type": "string", "enum": []string{"active", "deactivated", "locked"}},
						"data":         dataSchema,
						"schema_id":    map[string]any{"type": "string"},
						"capabilities": map[string]any{"type": "array", "items": map[string]any{"type": "string"}},
						"created_at":   map[string]any{"type": "string", "format": "date-time"},
						"updated_at":   map[string]any{"type": "string", "format": "date-time"},
					},
				},
				"CreateIdentityRequest": map[string]any{
					"type":     "object",
					"required": []string{"identifier"},
					"properties": map[string]any{
						"identifier":   map[string]any{"type": "string"},
						"data":         dataSchema,
						"schema_id":    map[string]any{"type": "string"},
						"capabilities": map[string]any{"type": "array", "items": map[string]any{"type": "string"}},
					},
				},
				"UpdateIdentityRequest": map[string]any{
					"type": "object",
					"properties": map[string]any{
						"state":     map[string]any{"type": "string"},
						"data":      dataSchema,
						"schema_id": map[string]any{"type": "string"},
					},
				},
				"ListIdentitiesResponse": map[string]any{
					"type": "object",
					"properties": map[string]any{
						"items":       map[string]any{"type": "array", "items": map[string]any{"$ref": "#/components/schemas/Entity"}},
						"next_cursor": map[string]any{"type": "string"},
					},
				},
				"CreateSchemaRequest": map[string]any{
					"type":     "object",
					"required": []string{"id", "type", "schema"},
					"properties": map[string]any{
						"id":     map[string]any{"type": "string"},
						"type":   map[string]any{"type": "string"},
						"org_id": map[string]any{"type": "integer"},
						"schema": map[string]any{"type": "object", "description": "A JSON Schema document"},
					},
				},
				"SchemaObject": map[string]any{
					"type": "object",
					"properties": map[string]any{
						"id":         map[string]any{"type": "string"},
						"type":       map[string]any{"type": "string"},
						"org_id":     map[string]any{"type": "integer"},
						"schema":     map[string]any{"type": "object"},
						"version":    map[string]any{"type": "integer"},
						"created_at": map[string]any{"type": "string", "format": "date-time"},
					},
				},
				"ListSchemasResponse": map[string]any{
					"type": "object",
					"properties": map[string]any{
						"items": map[string]any{"type": "array", "items": map[string]any{"$ref": "#/components/schemas/SchemaObject"}},
					},
				},
				"Error": map[string]any{
					"type": "object",
					"properties": map[string]any{
						"error":   map[string]any{"type": "string"},
						"code":    map[string]any{"type": "integer"},
						"details": map[string]any{"type": "string"},
					},
				},
			},
		},
	}

	writeJSON(w, http.StatusOK, spec)
}
