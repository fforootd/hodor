// Package uniqueness provides schema-driven uniqueness enforcement (ADR-016).
//
// It extracts x-unique annotations from entity schemas and enforces them
// via the unique_fields table. Uniqueness is cross-type: an email unique at
// instance scope is unique regardless of whether the entity is a human_user
// or service_user.
package uniqueness

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"fmt"
	"strings"

	"github.com/zitadel/zitadel/internal/httputil"
)

// Scope defines the uniqueness scope for a field.
type Scope string

const (
	ScopeInstance Scope = "instance" // Globally unique across all orgs.
	ScopeOrg      Scope = "org"      // Unique within an org.
)

// FieldConstraint represents a single x-unique annotation extracted from a schema.
type FieldConstraint struct {
	FieldName string
	Scope     Scope
}

// ViolationError is returned when a uniqueness check fails.
type ViolationError struct {
	Field string `json:"field"`
	Value string `json:"value"`
	Scope string `json:"scope"`
}

func (v *ViolationError) Error() string {
	return fmt.Sprintf("uniqueness violation: field %q value %q already exists (scope: %s)", v.Field, v.Value, v.Scope)
}

// ErrIdentityNotFound is returned when ResolveIdentifier finds no matching entity.
var ErrIdentityNotFound = errors.New("identity not found")

// ExtractConstraints parses a JSON schema string and returns all x-unique field constraints.
func ExtractConstraints(schemaJSON string) []FieldConstraint {
	var raw struct {
		Properties map[string]map[string]any `json:"properties"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &raw); err != nil {
		return nil
	}

	var constraints []FieldConstraint
	for name, def := range raw.Properties {
		v, ok := def["x-unique"]
		if !ok {
			continue
		}

		switch val := v.(type) {
		case string:
			scope := Scope(val)
			if scope == ScopeInstance || scope == ScopeOrg {
				constraints = append(constraints, FieldConstraint{FieldName: name, Scope: scope})
			}
		case bool:
			// x-unique: false — explicitly no uniqueness.
		}
	}
	return constraints
}

// ExtractIdentifiers returns field names marked with x-identifier: true.
func ExtractIdentifiers(schemaJSON string) []string {
	var raw struct {
		Properties map[string]map[string]any `json:"properties"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &raw); err != nil {
		return nil
	}

	var identifiers []string
	for name, def := range raw.Properties {
		if v, ok := def["x-identifier"].(bool); ok && v {
			identifiers = append(identifiers, name)
		}
	}
	return identifiers
}

// Normalize lowercases and trims a value for uniqueness comparison.
func Normalize(value string) string {
	return strings.ToLower(strings.TrimSpace(value))
}

// Enforce inserts unique_fields rows for an entity within a transaction.
// It reads the entity's data JSON, extracts values for x-unique fields,
// normalizes them, and inserts into unique_fields.
// Returns a *Violation error if a constraint is violated.
func Enforce(ctx context.Context, tx *sql.Tx, userID, orgID string, constraints []FieldConstraint, data map[string]any) error {
	instanceID := httputil.InstanceIDFromContext(ctx)
	for _, c := range constraints {
		rawVal, ok := data[c.FieldName]
		if !ok || rawVal == nil {
			continue
		}

		value := Normalize(fmt.Sprint(rawVal))
		if value == "" {
			continue
		}

		scopeID := "" // instance scope
		if c.Scope == ScopeOrg {
			scopeID = orgID
		}

		_, err := tx.ExecContext(ctx,
			`INSERT INTO unique_fields (instance_id, scope_id, field_name, normalized_value, user_id)
			 VALUES (?, ?, ?, ?, ?)`,
			instanceID, scopeID, c.FieldName, value, userID,
		)
		if err != nil {
			return &ViolationError{
				Field: c.FieldName,
				Value: fmt.Sprint(rawVal),
				Scope: string(c.Scope),
			}
		}
	}
	return nil
}

// EnforceFromIdentifier is a convenience for entities that use the legacy
// identifier column as their primary unique field. It writes an "identifier"
// entry into unique_fields at instance scope.
func EnforceFromIdentifier(ctx context.Context, tx *sql.Tx, userID, orgID, identifier string) error {
	if identifier == "" {
		return nil
	}
	instanceID := httputil.InstanceIDFromContext(ctx)
	normalized := Normalize(identifier)
	_, err := tx.ExecContext(ctx,
		`INSERT INTO unique_fields (instance_id, scope_id, field_name, normalized_value, user_id)
		 VALUES (?, '', 'identifier', ?, ?)`,
		instanceID, normalized, userID,
	)
	if err != nil {
		return &ViolationError{
			Field: "identifier",
			Value: identifier,
			Scope: "instance",
		}
	}
	return nil
}

// Release removes all unique_fields rows for an entity (used before re-enforcement on update).
func Release(ctx context.Context, tx *sql.Tx, userID string) error {
	instanceID := httputil.InstanceIDFromContext(ctx)
	_, err := tx.ExecContext(ctx, `DELETE FROM unique_fields WHERE instance_id = ? AND user_id = ?`, instanceID, userID)
	return err
}

// ResolvedEntity is the result of an identifier resolution.
type ResolvedEntity struct {
	UserID      string
	DisplayName string
	OrgID       string
}

// ResolveIdentifier looks up an entity by a unique field value.
// It tries instance-scoped matches first, then org-scoped matches if orgID is provided.
func ResolveIdentifier(ctx context.Context, db *sql.DB, identifier, orgID string) (*ResolvedEntity, error) {
	normalized := Normalize(identifier)
	instanceID := httputil.InstanceIDFromContext(ctx)

	// Phase 1: Instance-scoped match (globally unique identifiers).
	var result ResolvedEntity
	err := db.QueryRowContext(ctx,
		`SELECT uf.user_id, COALESCE(u.display_name, u.identifier), u.org_id
		 FROM unique_fields uf
		 JOIN users u ON u.id = uf.user_id
		 WHERE uf.instance_id = ?
		   AND uf.normalized_value = ?
		   AND uf.scope_id = ''
		   AND u.instance_id = ?
		   AND u.state = 'active'
		 LIMIT 1`, instanceID, normalized, instanceID,
	).Scan(&result.UserID, &result.DisplayName, &result.OrgID)
	if err == nil {
		return &result, nil
	}
	if err != sql.ErrNoRows {
		return nil, fmt.Errorf("resolve identifier (instance): %w", err)
	}

	// Phase 2: Org-scoped match (if org context is available).
	if orgID != "" {
		err = db.QueryRowContext(ctx,
			`SELECT uf.user_id, COALESCE(u.display_name, u.identifier), u.org_id
			 FROM unique_fields uf
			 JOIN users u ON u.id = uf.user_id
			 WHERE uf.instance_id = ?
			   AND uf.normalized_value = ?
			   AND uf.scope_id = ?
			   AND u.instance_id = ?
			   AND u.state = 'active'
			 LIMIT 1`, instanceID, normalized, orgID, instanceID,
		).Scan(&result.UserID, &result.DisplayName, &result.OrgID)
		if err == nil {
			return &result, nil
		}
		if err != sql.ErrNoRows {
			return nil, fmt.Errorf("resolve identifier (org): %w", err)
		}
	}

	// Phase 3: Fall back to legacy entities.identifier column for backward compat.
	query := `SELECT id, COALESCE(display_name, identifier), org_id
	          FROM users WHERE instance_id = ? AND LOWER(identifier) = ? AND state = 'active'`
	args := []any{instanceID, normalized}
	if orgID != "" {
		query += ` AND org_id = ?`
		args = append(args, orgID)
	}
	query += ` LIMIT 1`

	err = db.QueryRowContext(ctx, query, args...).Scan(&result.UserID, &result.DisplayName, &result.OrgID)
	if err == nil {
		return &result, nil
	}
	if err == sql.ErrNoRows {
		return nil, ErrIdentityNotFound
	}
	return nil, fmt.Errorf("resolve identifier (legacy): %w", err)
}

// ValidateSchemaChange checks whether tightening uniqueness constraints on a
// schema type would cause violations against existing data.
// Returns a list of violations (duplicate values that would conflict).
func ValidateSchemaChange(ctx context.Context, db *sql.DB, constraints []FieldConstraint) ([]map[string]any, error) {
	var violations []map[string]any
	instanceID := httputil.InstanceIDFromContext(ctx)

	for _, c := range constraints {
		var query string
		switch c.Scope {
		case ScopeInstance:
			query = `SELECT normalized_value, COUNT(*) as cnt
			         FROM unique_fields
			         WHERE instance_id = ? AND field_name = ?
			         GROUP BY normalized_value
			         HAVING cnt > 1`
		case ScopeOrg:
			query = `SELECT scope_id, normalized_value, COUNT(*) as cnt
			         FROM unique_fields
			         WHERE instance_id = ? AND field_name = ?
			         GROUP BY scope_id, normalized_value
			         HAVING cnt > 1`
		}

		rows, err := db.QueryContext(ctx, query, instanceID, c.FieldName)
		if err != nil {
			return nil, fmt.Errorf("check duplicates for %s: %w", c.FieldName, err)
		}
		defer rows.Close()

		for rows.Next() {
			v := map[string]any{"field": c.FieldName, "scope": string(c.Scope)}
			switch c.Scope {
			case ScopeInstance:
				var value string
				var cnt int
				if rows.Scan(&value, &cnt) == nil {
					v["value"] = value
					v["count"] = cnt
					violations = append(violations, v)
				}
			case ScopeOrg:
				var scopeID, value string
				var cnt int
				if rows.Scan(&scopeID, &value, &cnt) == nil {
					v["value"] = value
					v["org_id"] = scopeID
					v["count"] = cnt
					violations = append(violations, v)
				}
			}
		}
		if err := rows.Err(); err != nil {
			return nil, fmt.Errorf("iterate duplicates: %w", err)
		}
	}

	return violations, nil
}
