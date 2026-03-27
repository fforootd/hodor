// Package schema provides meta-schema validation for identity schema annotations.
package schema

import _ "embed"

// MetaSchema is the JSON Schema that validates x-* annotations on entity schemas.
// It defines the allowed structure of x-auth-methods, x-login, x-branding, and
// per-field x-auth annotations.
//
//go:embed meta_schema.json
var MetaSchema string
