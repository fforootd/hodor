// Package login provides claim mapping between OIDC ID token claims
// and identity profile fields using expr expressions.
//
// Mapping resolution order:
//  1. Schema x-claim annotations (defaults)
//  2. Provider claim_overrides (takes priority)
package login

import (
	"encoding/json"
	"fmt"
	"github.com/zitadel/zitadel/internal/logging"
	"strings"

	"github.com/expr-lang/expr"
)

// ClaimMappings extracts x-claim annotations from a JSON schema string.
// Returns a map of field_name → expr_expression.
func ClaimMappings(schemaJSON string) map[string]string {
	var schema struct {
		Properties map[string]map[string]any `json:"properties"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &schema); err != nil {
		return nil
	}

	result := make(map[string]string)
	for field, def := range schema.Properties {
		if mapping, ok := def["x-claim"].(string); ok && mapping != "" {
			result[field] = mapping
		}
	}
	return result
}

// MapClaims evaluates claim mapping expressions against raw OIDC claims.
// Schema-level x-claim provides defaults; provider-level overrides take priority.
// Returns a profile map suitable for storing in the identity.
func MapClaims(schemaJSON string, providerOverrides map[string]string, rawClaims map[string]any) (map[string]any, error) {
	// 1. Extract default mappings from schema.
	defaults := ClaimMappings(schemaJSON)
	if defaults == nil {
		defaults = make(map[string]string)
	}

	// 2. Merge: provider overrides win.
	merged := make(map[string]string, len(defaults))
	for field, exprStr := range defaults {
		merged[field] = exprStr
	}
	for field, exprStr := range providerOverrides {
		merged[field] = exprStr
	}

	// 3. Evaluate each expression against the claims.
	env := map[string]any{
		"claims": rawClaims,
	}

	profile := make(map[string]any, len(merged))
	for field, exprStr := range merged {
		val, err := evalClaimExpr(exprStr, env)
		if err != nil {
			logging.Printf("[claims] expr eval error for field %q: %v (expr: %s)", field, err, exprStr)
			continue // skip fields that fail to evaluate
		}
		if val != nil && val != "" {
			profile[field] = val
		}
	}

	return profile, nil
}

// evalClaimExpr safely evaluates a single expr expression.
func evalClaimExpr(exprStr string, env map[string]any) (any, error) {
	program, err := expr.Compile(exprStr, expr.Env(env))
	if err != nil {
		return nil, fmt.Errorf("compile: %w", err)
	}

	output, err := expr.Run(program, env)
	if err != nil {
		return nil, fmt.Errorf("run: %w", err)
	}

	return output, nil
}

// DefaultGoogleOverrides returns claim overrides for Google (usually empty since
// Google follows standard OIDC claims).
func DefaultGoogleOverrides() map[string]string {
	return map[string]string{}
}

// DefaultEntraIDOverrides returns claim overrides for Microsoft Entra ID.
func DefaultEntraIDOverrides() map[string]string {
	return map[string]string{
		"email": "claims.preferred_username ?? claims.email ?? claims.upn",
	}
}

// ---------- Outbound: Identity → OIDC Userinfo Claims ----------

// standardOIDCClaims maps schema field names to their standard OIDC claim names.
// This provides a fallback when x-claim is not present.
var standardOIDCClaims = map[string]string{
	"email":        "email",
	"phone":        "phone_number",
	"display_name": "name",
	"first_name":   "given_name",
	"last_name":    "family_name",
	"locale":       "locale",
	"timezone":     "zoneinfo",
	"avatar_url":   "picture",
	"nickname":     "nickname",
}

// UserinfoClaims reads x-claim annotations from a JSON schema and maps
// identity data fields to standard OIDC claims. This is the outbound
// counterpart to MapClaims (inbound IDP claims).
//
// Resolution: for each schema property with x-claim, extract the target
// OIDC claim name from the expression, then emit data[field] under that claim name.
func UserinfoClaims(schemaJSON string, data map[string]any) map[string]any {
	var schema struct {
		Properties map[string]map[string]any `json:"properties"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &schema); err != nil {
		return nil
	}

	result := make(map[string]any)

	for field, def := range schema.Properties {
		val, ok := data[field]
		if !ok || val == nil || val == "" {
			continue
		}

		// Determine the OIDC claim name from x-claim.
		var claimName string
		if mapping, ok := def["x-claim"].(string); ok && mapping != "" {
			claimName = OIDCClaimName(mapping)
		}
		if claimName == "" {
			// Fallback: use the standardOIDCClaims table.
			claimName = standardOIDCClaims[field]
		}
		if claimName == "" {
			// No mapping known — skip (don't leak arbitrary fields).
			continue
		}

		result[claimName] = val
	}

	return result
}

// OIDCClaimName extracts the standard OIDC claim name from an x-claim
// expression. It handles common patterns:
//
//	"claims.email"                                    → "email"
//	"claims.name ?? (claims.given_name + ' ' + ...)"  → "name"
//	"claims.phone_number ?? ''"                       → "phone_number"
func OIDCClaimName(expr string) string {
	expr = strings.TrimSpace(expr)

	// Must start with "claims."
	if !strings.HasPrefix(expr, "claims.") {
		return ""
	}

	// Strip "claims." prefix.
	rest := expr[7:]

	// Take everything until a non-identifier character.
	// Identifier chars: a-z, A-Z, 0-9, _
	end := 0
	for end < len(rest) {
		c := rest[end]
		if (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_' {
			end++
		} else {
			break
		}
	}

	if end == 0 {
		return ""
	}
	return rest[:end]
}
