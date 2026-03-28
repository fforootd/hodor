// Package settings implements hierarchical settings resolution (ADR-009).
// Settings cascade: instance → org → app, using deep merge.
// Each scope level can override individual fields without replacing the full config.
package settings

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"

	"github.com/google/uuid"
)

// scopeOrder defines the cascade hierarchy from broadest to most specific.
var scopeOrder = []string{"instance", "org", "app"}

// Resolve returns the effective settings for a type by deep-merging the
// scope chain: instance ← org ← app. Only fields explicitly set at a
// lower scope override the parent.
func Resolve(ctx context.Context, db *sql.DB, settingsType string, orgID string, appID string) (map[string]any, error) {
	chain := []struct {
		scope   string
		scopeID string
	}{
		{"instance", ""},
	}
	if orgID != "" {
		chain = append(chain, struct {
			scope   string
			scopeID string
		}{"org", orgID})
	}
	if appID != "" {
		chain = append(chain, struct {
			scope   string
			scopeID string
		}{"app", appID})
	}

	result := make(map[string]any)
	for _, s := range chain {
		data, err := Get(ctx, db, settingsType, s.scope, s.scopeID)
		if err != nil {
			return nil, fmt.Errorf("get settings %s/%s/%s: %w", settingsType, s.scope, s.scopeID, err)
		}
		if data != nil {
			deepMerge(result, data)
		}
	}

	return result, nil
}

// Get reads the raw (unmerged) settings override at a specific scope.
// Returns nil, nil if no override exists at that scope.
func Get(ctx context.Context, db *sql.DB, settingsType, scope, scopeID string) (map[string]any, error) {
	var dataJSON string
	err := db.QueryRowContext(ctx,
		`SELECT data FROM settings WHERE type = ? AND scope = ? AND scope_id = ?`,
		settingsType, scope, scopeID,
	).Scan(&dataJSON)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	if err != nil {
		return nil, fmt.Errorf("query: %w", err)
	}

	var data map[string]any
	if err := json.Unmarshal([]byte(dataJSON), &data); err != nil {
		return nil, fmt.Errorf("unmarshal: %w", err)
	}
	return data, nil
}

// Put upserts a settings override at a specific scope.
// The data should contain only the fields being overridden, not the full config.
func Put(ctx context.Context, db *sql.DB, settingsType, scope, scopeID string, data map[string]any) error {
	dataJSON, err := json.Marshal(data)
	if err != nil {
		return fmt.Errorf("marshal: %w", err)
	}

	now := time.Now().UTC().Format(time.RFC3339)
	id := uuid.New().String()

	// UPSERT: insert or update on conflict.
	_, err = db.ExecContext(ctx,
		`INSERT INTO settings (id, type, scope, scope_id, data, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(type, scope, scope_id) DO UPDATE SET data = ?, updated_at = ?`,
		id, settingsType, scope, scopeID, string(dataJSON), now, now,
		string(dataJSON), now,
	)
	if err != nil {
		return fmt.Errorf("upsert: %w", err)
	}

	return nil
}

// Delete removes a settings override at a specific scope.
// The scope inherits from its parent after deletion.
func Delete(ctx context.Context, db *sql.DB, settingsType, scope, scopeID string) error {
	_, err := db.ExecContext(ctx,
		`DELETE FROM settings WHERE type = ? AND scope = ? AND scope_id = ?`,
		settingsType, scope, scopeID,
	)
	return err
}

// deepMerge merges src into dst. For nested maps, fields are merged recursively.
// For all other types, src values overwrite dst values.
func deepMerge(dst, src map[string]any) {
	for k, srcVal := range src {
		if srcMap, ok := srcVal.(map[string]any); ok {
			if dstMap, ok := dst[k].(map[string]any); ok {
				deepMerge(dstMap, srcMap)
				continue
			}
		}
		dst[k] = srcVal
	}
}
