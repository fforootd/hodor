// Package loginflow provides login flow resolution and audience targeting.
// Separated from internal/login to avoid import cycles (login ↔ api).
package loginflow

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"sort"

	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/logging"
)

// FlowState represents the lifecycle state of a login flow.
type FlowState string

const (
	FlowStateDraft    FlowState = "draft"
	FlowStateTesting  FlowState = "testing"
	FlowStateActive   FlowState = "active"
	FlowStateArchived FlowState = "archived"
)

// Audience defines who sees a particular login flow.
type Audience struct {
	SchemaIDs      []string          `json:"schema_ids,omitempty"`
	UserIDs        []string          `json:"user_ids,omitempty"`
	OrgIDs         []string          `json:"org_ids,omitempty"`
	MetadataFilter map[string]string `json:"metadata_filter,omitempty"`
}

// LoginFlow represents a login flow entity with audience targeting.
type LoginFlow struct {
	ID          string          `json:"id"`
	OrgID       string          `json:"org_id"`
	Name        string          `json:"name"`
	Strategy    string          `json:"strategy"`
	Config      json.RawMessage `json:"config"`
	IsDefault   bool            `json:"is_default"`
	Enabled     bool            `json:"enabled"`
	State       FlowState       `json:"state"`
	Priority    int             `json:"priority"`
	Audience    Audience        `json:"audience"`
	AuthMethods json.RawMessage `json:"auth_methods"`
}

// Resolver resolves the best login flow for a given user context.
// Uses most-precise-match-wins (proxy routing pattern):
//
//	Level 4: explicit user_id match   (+1000)
//	Level 3: schema_id match          (+100)
//	Level 2: org_id match             (+10)
//	Level 1: is_default fallback      (+0)
type Resolver struct {
	db *database.DB
}

// NewResolver creates a new flow resolver.
func NewResolver(db *database.DB) *Resolver {
	return &Resolver{db: db}
}

// UserContext holds the user's identity attributes for flow resolution.
type UserContext struct {
	UserID   string
	OrgID    string
	SchemaID string
	Metadata map[string]string
}

// scoredFlow wraps a LoginFlow with a computed specificity score.
type scoredFlow struct {
	flow  *LoginFlow
	score int
}

// Resolve picks the best login flow for a given user context.
// Returns nil if no flow matches (should not happen if a default exists).
func (r *Resolver) Resolve(ctx context.Context, uc UserContext) (*LoginFlow, error) {
	flows, err := r.loadActiveFlows(ctx, uc.OrgID)
	if err != nil {
		return nil, fmt.Errorf("load active flows: %w", err)
	}

	if len(flows) == 0 {
		return nil, fmt.Errorf("no login flows found for org %s", uc.OrgID)
	}

	var candidates []scoredFlow
	for i := range flows {
		f := &flows[i]
		score, ok := r.matchFlow(f, uc)
		if ok {
			candidates = append(candidates, scoredFlow{flow: f, score: score})
		}
	}

	if len(candidates) == 0 {
		// Fallback to default flow.
		for i := range flows {
			if flows[i].IsDefault {
				return &flows[i], nil
			}
		}
		return nil, fmt.Errorf("no matching login flow and no default configured")
	}

	// Sort by score DESC, then by flow priority DESC.
	sort.Slice(candidates, func(i, j int) bool {
		if candidates[i].score != candidates[j].score {
			return candidates[i].score > candidates[j].score
		}
		return candidates[i].flow.Priority > candidates[j].flow.Priority
	})

	return candidates[0].flow, nil
}

// matchFlow checks if a flow matches the user context and returns a specificity score.
// Returns (score, true) if the flow matches, (0, false) otherwise.
func (r *Resolver) matchFlow(f *LoginFlow, uc UserContext) (int, bool) {
	// Flows in 'testing' state only serve to explicit user_ids.
	if f.State == FlowStateTesting {
		if contains(f.Audience.UserIDs, uc.UserID) {
			return f.Priority + 1000, true
		}
		return 0, false
	}

	// Default flow matches everyone at base priority.
	if f.IsDefault && len(f.Audience.UserIDs) == 0 && len(f.Audience.SchemaIDs) == 0 && len(f.Audience.OrgIDs) == 0 {
		return f.Priority, true
	}

	score := f.Priority
	matched := false

	// Level 4: Explicit user_id match (highest specificity).
	if len(f.Audience.UserIDs) > 0 {
		if contains(f.Audience.UserIDs, uc.UserID) {
			score += 1000
			matched = true
		} else if len(f.Audience.SchemaIDs) == 0 && len(f.Audience.OrgIDs) == 0 {
			// Flow is user_id-only and we don't match.
			return 0, false
		}
	}

	// Level 3: Schema_id match.
	if len(f.Audience.SchemaIDs) > 0 {
		if contains(f.Audience.SchemaIDs, uc.SchemaID) {
			score += 100
			matched = true
		} else if !matched {
			return 0, false
		}
	}

	// Level 2: Org_id match.
	if len(f.Audience.OrgIDs) > 0 {
		if contains(f.Audience.OrgIDs, uc.OrgID) {
			score += 10
			matched = true
		} else if !matched {
			return 0, false
		}
	}

	// Metadata filter (AND logic — all keys must match).
	if len(f.Audience.MetadataFilter) > 0 {
		for k, v := range f.Audience.MetadataFilter {
			if uc.Metadata[k] != v {
				if !matched {
					return 0, false
				}
				// If we already matched on a higher level, metadata mismatch
				// doesn't disqualify but reduces specificity.
				score -= 1
			}
		}
		matched = true
	}

	// If no audience rules are set, the flow matches everyone.
	if !matched && len(f.Audience.UserIDs) == 0 && len(f.Audience.SchemaIDs) == 0 && len(f.Audience.OrgIDs) == 0 && len(f.Audience.MetadataFilter) == 0 {
		matched = true
	}

	return score, matched
}

// loadActiveFlows loads all non-draft/non-archived flows for the given org
// (plus instance-level flows where org_id is NULL).
func (r *Resolver) loadActiveFlows(ctx context.Context, orgID string) ([]LoginFlow, error) {
	scoped := r.db.Scoped(ctx)
	rows, err := scoped.QueryContext(ctx, scoped.Rebind(
		`SELECT id, COALESCE(org_id,''), name, strategy, config, COALESCE(is_default,0), COALESCE(enabled,1), state, priority,
		        COALESCE(audience,'{}'), COALESCE(auth_methods,'{}')
		 FROM login_flows
		 WHERE instance_id = ?
		   AND COALESCE(enabled,1) = 1 AND state IN ('active','testing')
		   AND (org_id IS NULL OR org_id = '' OR org_id = ?)
		 ORDER BY priority DESC`),
		scoped.InstanceID(), orgID,
	)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var flows []LoginFlow
	for rows.Next() {
		var f LoginFlow
		var configJSON, audienceJSON, authMethodsJSON string
		var isDefault, enabled int
		err := rows.Scan(&f.ID, &f.OrgID, &f.Name, &f.Strategy, &configJSON,
			&isDefault, &enabled, &f.State, &f.Priority,
			&audienceJSON, &authMethodsJSON)
		if err != nil {
			logging.Printf("[flow_resolver] scan error: %v", err)
			continue
		}
		f.IsDefault = isDefault == 1 || isDefault != 0
		f.Enabled = enabled == 1 || enabled != 0
		f.Config = json.RawMessage(configJSON)
		_ = json.Unmarshal([]byte(audienceJSON), &f.Audience)
		f.AuthMethods = json.RawMessage(authMethodsJSON)
		flows = append(flows, f)
	}
	return flows, rows.Err()
}

// TestAudience runs the audience rules against a sample of real users and returns
// which users would match this flow.
func (r *Resolver) TestAudience(ctx context.Context, flowID string, limit int) (*AudienceTestResult, error) {
	scoped := r.db.Scoped(ctx)
	// Load the flow.
	var f LoginFlow
	var configJSON, audienceJSON, authMethodsJSON string
	var isDefault, enabled int
	err := scoped.QueryRowContext(ctx, scoped.Rebind(
		`SELECT id, COALESCE(org_id,''), name, strategy, config, COALESCE(is_default,0), COALESCE(enabled,1), state, priority,
		        COALESCE(audience,'{}'), COALESCE(auth_methods,'{}')
		 FROM login_flows WHERE instance_id = ? AND id = ?`), scoped.InstanceID(), flowID,
	).Scan(&f.ID, &f.OrgID, &f.Name, &f.Strategy, &configJSON,
		&isDefault, &enabled, &f.State, &f.Priority,
		&audienceJSON, &authMethodsJSON)
	if err != nil {
		if err == sql.ErrNoRows {
			return nil, fmt.Errorf("flow not found: %s", flowID)
		}
		return nil, err
	}
	f.IsDefault = isDefault == 1
	f.Enabled = enabled == 1
	f.Config = json.RawMessage(configJSON)
	f.AuthMethods = json.RawMessage(authMethodsJSON)
	_ = json.Unmarshal([]byte(audienceJSON), &f.Audience)

	// Fetch sample of users.
	if limit <= 0 || limit > 100 {
		limit = 20
	}
	userRows, err := scoped.QueryContext(ctx, scoped.Rebind(
		`SELECT id, COALESCE(org_id,''), COALESCE(schema_id,''), COALESCE(display_name,''), COALESCE(identifier,''), COALESCE(metadata,'{}')
		 FROM users WHERE instance_id = ? AND state = 'active' ORDER BY created_at DESC LIMIT ?`), scoped.InstanceID(), limit*3, // overfetch to get enough matches
	)
	if err != nil {
		return nil, err
	}
	defer userRows.Close()

	result := &AudienceTestResult{
		FlowID:  flowID,
		Matches: []AudienceMatch{},
	}

	var totalUsers int
	for userRows.Next() {
		totalUsers++
		var uid, orgID, schemaID, displayName, identifier, metaJSON string
		if err := userRows.Scan(&uid, &orgID, &schemaID, &displayName, &identifier, &metaJSON); err != nil {
			continue
		}
		var meta map[string]string
		_ = json.Unmarshal([]byte(metaJSON), &meta)

		uc := UserContext{
			UserID:   uid,
			OrgID:    orgID,
			SchemaID: schemaID,
			Metadata: meta,
		}

		score, ok := r.matchFlow(&f, uc)
		if ok && len(result.Matches) < limit {
			reason := classifyMatch(score, f.Priority)
			result.Matches = append(result.Matches, AudienceMatch{
				UserID:      uid,
				DisplayName: displayName,
				Identifier:  identifier,
				Score:       score,
				Reason:      reason,
			})
		}
	}
	if err := userRows.Err(); err != nil {
		return nil, fmt.Errorf("user rows: %w", err)
	}

	result.TotalUsers = totalUsers
	result.MatchingUsers = len(result.Matches)
	if totalUsers > 0 {
		result.MatchPct = float64(result.MatchingUsers) / float64(totalUsers) * 100
	}
	return result, nil
}

// AudienceTestResult is returned by the test endpoint.
type AudienceTestResult struct {
	FlowID        string          `json:"flow_id"`
	TotalUsers    int             `json:"total_users"`
	MatchingUsers int             `json:"matching_users"`
	MatchPct      float64         `json:"matching_percentage"`
	Matches       []AudienceMatch `json:"sample"`
}

// AudienceMatch describes why a specific user matches a flow.
type AudienceMatch struct {
	UserID      string `json:"user_id"`
	DisplayName string `json:"display_name"`
	Identifier  string `json:"identifier"`
	Score       int    `json:"score"`
	Reason      string `json:"reason"`
}

func classifyMatch(score, basePriority int) string {
	adjusted := score - basePriority
	switch {
	case adjusted >= 1000:
		return "user_id (explicit allowlist)"
	case adjusted >= 100:
		return "schema_id match"
	case adjusted >= 10:
		return "org_id match"
	default:
		return "default flow"
	}
}

func contains(haystack []string, needle string) bool {
	for _, s := range haystack {
		if s == needle {
			return true
		}
	}
	return false
}
