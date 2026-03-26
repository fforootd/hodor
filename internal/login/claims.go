// Package login provides claim mapping between OIDC ID token claims
// and identity profile fields using expr expressions.
//
// Mapping resolution order:
//  1. Schema x-claim-mapping annotations (defaults)
//  2. Provider claim_overrides (takes priority)
package login

import (
	"encoding/json"
	"fmt"
	"log"

	"github.com/expr-lang/expr"
)

// ClaimMappings extracts x-claim-mapping annotations from a JSON schema string.
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
		if mapping, ok := def["x-claim-mapping"].(string); ok && mapping != "" {
			result[field] = mapping
		}
	}
	return result
}

// MapClaims evaluates claim mapping expressions against raw OIDC claims.
// Schema-level x-claim-mapping provides defaults; provider-level overrides take priority.
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
			log.Printf("[claims] expr eval error for field %q: %v (expr: %s)", field, err, exprStr)
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
