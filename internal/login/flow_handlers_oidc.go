package login

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"net/http"
	"net/url"

	"github.com/zitadel/zitadel/internal/httputil"
)

type oidcAuthRequestContext struct {
	RedirectURI string
	State       string
	Prompt      []string
	LoginHint   string
}

func (h *Handler) lookupOIDCAuthRequest(ctx context.Context, requestID string) (*oidcAuthRequestContext, error) {
	var authReq oidcAuthRequestContext
	var dataJSON string
	err := h.db.SQL().QueryRowContext(ctx,
		`SELECT redirect_uri, state, COALESCE(data, '{}')
		 FROM auth_states
		 WHERE id = ?
			   AND type = 'oidc_auth'
			   AND expires_at > datetime('now')`,
		requestID,
	).Scan(&authReq.RedirectURI, &authReq.State, &dataJSON)
	if err != nil {
		if errors.Is(err, sql.ErrNoRows) {
			return nil, err
		}
		return nil, err
	}
	if dataJSON != "" && dataJSON != "{}" {
		var data map[string]any
		if err := json.Unmarshal([]byte(dataJSON), &data); err == nil {
			authReq.Prompt = stringSliceFromAny(data["prompt"])
			if loginHint, ok := data["login_hint"].(string); ok {
				authReq.LoginHint = loginHint
			}
		}
	}
	return &authReq, nil
}

func (h *Handler) completeOIDCAuthRequest(ctx context.Context, requestID, userID string) error {
	result, err := h.db.SQL().ExecContext(ctx,
		`UPDATE auth_states
		 SET user_id = ?, done = 1, auth_time = datetime('now')
		 WHERE id = ? AND type = 'oidc_auth'`,
		userID, requestID,
	)
	if err != nil {
		return err
	}
	rowsAffected, _ := result.RowsAffected()
	if rowsAffected == 0 {
		return sql.ErrNoRows
	}
	return nil
}

func (h *Handler) oidcAuthorizeCallbackURL(requestID string) string {
	return "/authorize/callback?id=" + url.QueryEscape(requestID)
}

func stringSliceFromAny(value any) []string {
	values, ok := value.([]any)
	if !ok {
		return nil
	}
	result := make([]string, 0, len(values))
	for _, value := range values {
		if s, ok := value.(string); ok {
			result = append(result, s)
		}
	}
	return result
}

func hasOIDCPrompt(prompts []string, want string) bool {
	for _, prompt := range prompts {
		if prompt == want {
			return true
		}
	}
	return false
}

func allowTrustedSessionReuse(prompts []string) bool {
	return !hasOIDCPrompt(prompts, "login") && !hasOIDCPrompt(prompts, "select_account")
}

func requireSilentTrustedSession(prompts []string) bool {
	return hasOIDCPrompt(prompts, "none")
}

func (h *Handler) loadTrustedIdentitySummary(ctx context.Context, userID string) (identifier, displayName string) {
	_ = h.db.SQL().QueryRowContext(ctx,
		`SELECT COALESCE(identifier, ''), COALESCE(display_name, '')
		 FROM users
		 WHERE id = ?`,
		userID,
	).Scan(&identifier, &displayName)
	return identifier, displayName
}

func (h *Handler) completeFlowWithTrustedSession(w http.ResponseWriter, flow *Flow) {
	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"flow_id":      flow.ID,
		"step":         "complete",
		"session_id":   "",
		"redirect_uri": h.oidcAuthorizeCallbackURL(flow.AuthRequestID),
	})
}
