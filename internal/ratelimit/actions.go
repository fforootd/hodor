package ratelimit

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"sync"
	"time"

	"github.com/expr-lang/expr"
	"github.com/expr-lang/expr/vm"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"
)

// ActionEngine evaluates expr actions at pipeline hooks (ADR-015).
// Actions are loaded from the entities table (schema_type = 'action')
// and compiled expr programs are cached.
type ActionEngine struct {
	db       *sql.DB
	cache    map[string]*compiledAction
	cacheMu  sync.RWMutex
	ttl      time.Duration
	loadedAt time.Time
}

type compiledAction struct {
	ID         string
	Name       string
	Hook       string // pipeline hook (on_request, pre_auth, etc.)
	Priority   int
	ActionType string // rate_limit, webhook, expr, etc.
	Trigger    *vm.Program
	Config     map[string]any
	FailOpen   bool
	TimeoutMs  int
}

// ActionResult is the outcome of evaluating a single action.
type ActionResult struct {
	ActionID   string
	Matched    bool
	ActionType string // engine type that should run
	Config     map[string]any
	Error      error
}

// RequestEnv is the expr environment available to on_request actions.
type RequestEnv struct {
	Method  string            `expr:"method"`
	Path    string            `expr:"path"`
	Headers map[string]string `expr:"headers"`
	IP      string            `expr:"ip"`
	OrgID   string            `expr:"org_id"`
}

// NewActionEngine creates a new action engine with a default 30-second cache TTL.
func NewActionEngine(db *sql.DB) *ActionEngine {
	return &ActionEngine{
		db:    db,
		cache: make(map[string]*compiledAction),
		ttl:   30 * time.Second,
	}
}

// EvaluateHook loads actions for the given hook, evaluates their triggers,
// and returns results for actions whose triggers evaluated to true.
func (e *ActionEngine) EvaluateHook(ctx context.Context, hook string, env *RequestEnv) ([]ActionResult, error) {
	actions, err := e.loadActions(ctx, hook)
	if err != nil {
		return nil, fmt.Errorf("load actions: %w", err)
	}

	var results []ActionResult
	exprEnv := map[string]any{
		"request": map[string]any{
			"method":  env.Method,
			"path":    env.Path,
			"headers": env.Headers,
			"ip":      env.IP,
			"org_id":  env.OrgID,
		},
	}

	for _, action := range actions {
		result := ActionResult{
			ActionID:   action.ID,
			ActionType: action.ActionType,
			Config:     action.Config,
		}

		// Evaluate trigger.
		output, err := vm.Run(action.Trigger, exprEnv)
		if err != nil {
			result.Error = err
			if !action.FailOpen {
				result.Matched = true // fail closed = treat as match (block)
			}
			results = append(results, result)
			continue
		}

		matched, ok := output.(bool)
		if !ok {
			result.Error = fmt.Errorf("trigger did not return bool: %T", output)
			results = append(results, result)
			continue
		}

		result.Matched = matched
		if matched {
			results = append(results, result)
		}
	}

	return results, nil
}

// loadActions loads and compiles actions for a hook, using the TTL cache.
func (e *ActionEngine) loadActions(ctx context.Context, hook string) ([]*compiledAction, error) {
	e.cacheMu.RLock()
	if time.Since(e.loadedAt) < e.ttl && len(e.cache) > 0 {
		var actions []*compiledAction
		for _, a := range e.cache {
			if a.Hook == hook {
				actions = append(actions, a)
			}
		}
		e.cacheMu.RUnlock()
		return actions, nil
	}
	e.cacheMu.RUnlock()

	return e.refreshActions(ctx, hook)
}

// refreshActions queries the database for all enabled actions and recompiles them.
func (e *ActionEngine) refreshActions(ctx context.Context, hook string) ([]*compiledAction, error) {
	instanceID := httputil.InstanceIDFromContext(ctx)
	rows, err := e.db.QueryContext(ctx,
		`SELECT id, name, hook, action_type, COALESCE(trigger_expr, 'true'),
		        priority, config, fail_open, timeout_ms, enabled
		 FROM actions
		 WHERE instance_id = ?`,
		instanceID,
	)
	if err != nil {
		return nil, fmt.Errorf("query actions: %w", err)
	}
	defer rows.Close()

	e.cacheMu.Lock()
	defer e.cacheMu.Unlock()

	// Clear cache.
	e.cache = make(map[string]*compiledAction)
	e.loadedAt = time.Now()

	var hookActions []*compiledAction

	for rows.Next() {
		var actionID, name, actionHook, actionType, trigger, configStr string
		var priority, timeoutMs int
		var failOpen, enabled bool
		if err := rows.Scan(&actionID, &name, &actionHook, &actionType, &trigger,
			&priority, &configStr, &failOpen, &timeoutMs, &enabled); err != nil {
			logging.Printf("[actions] scan action: %v", err)
			continue
		}

		if !enabled {
			continue
		}

		var config map[string]any
		_ = json.Unmarshal([]byte(configStr), &config)

		// Compile the trigger expression.
		env := map[string]any{
			"request": map[string]any{
				"method":  "",
				"path":    "",
				"headers": map[string]string{},
				"ip":      "",
				"org_id":  "",
			},
		}

		program, err := expr.Compile(trigger, expr.Env(env), expr.AsBool())
		if err != nil {
			logging.Printf("[actions] compile action %s trigger %q: %v", actionID, trigger, err)
			continue
		}

		ca := &compiledAction{
			ID:         actionID,
			Name:       name,
			Hook:       actionHook,
			Priority:   priority,
			ActionType: actionType,
			Trigger:    program,
			Config:     config,
			FailOpen:   failOpen,
			TimeoutMs:  timeoutMs,
		}

		e.cache[actionID] = ca
		if actionHook == hook {
			hookActions = append(hookActions, ca)
		}
	}

	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("iterate actions: %w", err)
	}

	return hookActions, nil
}

// InvalidateCache forces a cache refresh on the next action evaluation.
func (e *ActionEngine) InvalidateCache() {
	e.cacheMu.Lock()
	e.loadedAt = time.Time{}
	e.cacheMu.Unlock()
}
