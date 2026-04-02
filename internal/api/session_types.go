package api

import (
	"net/http"

	sessionsvc "github.com/zitadel/zitadel/internal/session"
)

type SessionResponse struct {
	ID           string         `json:"id"`
	IduserID     string         `json:"user_id"`
	OrgID        string         `json:"org_id"`
	AuthMethod   string         `json:"auth_method,omitempty"`
	ProviderID   string         `json:"provider_id,omitempty"`
	ProviderKind string         `json:"provider_kind,omitempty"`
	LoginFlowID  string         `json:"login_flow_id,omitempty"`
	Metadata     map[string]any `json:"metadata,omitempty"`
	UserAgent    string         `json:"user_agent,omitempty"`
	IPAddress    string         `json:"ip_address,omitempty"`
	CreatedAt    string         `json:"created_at"`
	ExpiresAt    string         `json:"expires_at"`
	RevokedAt    *string        `json:"revoked_at,omitempty"`
}

type CreateSessionRequest struct {
	IduserID  string `json:"user_id"`
	UserAgent string `json:"user_agent,omitempty"`
	IPAddress string `json:"ip_address,omitempty"`
}

type CreateSessionResponse struct {
	Session SessionResponse `json:"session"`
	Token   string          `json:"token"`
}

func (a *API) RegisterSessionRoutes(mux *http.ServeMux, requireAdmin func(http.HandlerFunc) http.HandlerFunc) {
	mux.HandleFunc("POST /v1/sessions", requireAdmin(a.createSession))
	mux.HandleFunc("GET /v1/sessions", requireAdmin(a.listSessions))
	mux.HandleFunc("GET /v1/sessions/{id}", requireAdmin(a.getSession))
	mux.HandleFunc("POST /v1/sessions/{id}/revoke", requireAdmin(a.revokeSession))
}

func sessionResponseFromRecord(record sessionsvc.Record) SessionResponse {
	return SessionResponse{
		ID:           record.ID,
		IduserID:     record.UserID,
		OrgID:        record.OrgID,
		AuthMethod:   record.AuthMethod,
		ProviderID:   record.ProviderID,
		ProviderKind: record.ProviderKind,
		LoginFlowID:  record.LoginFlowID,
		Metadata:     record.Metadata,
		UserAgent:    record.UserAgent,
		IPAddress:    record.IPAddress,
		CreatedAt:    record.CreatedAt,
		ExpiresAt:    record.ExpiresAt,
		RevokedAt:    record.RevokedAt,
	}
}
