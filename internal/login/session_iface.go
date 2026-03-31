package login

import (
	"context"

	"github.com/zitadel/zitadel/internal/risk"
)

// SessionCreator abstracts session creation, breaking the import cycle
// between login and api packages.
type SessionCreator interface {
	CreateSessionForLogin(ctx context.Context, userID string, userAgent, ipAddress string, signals *risk.Signals, provenance *SessionProvenance) (*CreateSessionResponse, error)
	EmitAuthEvent(ctx context.Context, eventType string, actorID string, payload map[string]any)
	EmitEvent(ctx context.Context, eventType, actorID, aggregateID, aggregateType string, payload map[string]any)
}

type SessionProvenance struct {
	AuthMethod   string         `json:"auth_method,omitempty"`
	ProviderID   string         `json:"provider_id,omitempty"`
	ProviderKind string         `json:"provider_kind,omitempty"`
	LoginFlowID  string         `json:"login_flow_id,omitempty"`
	AuthContext  map[string]any `json:"auth_context,omitempty"`
}

// CreateSessionResponse mirrors api.CreateSessionResponse to avoid import cycle.
type CreateSessionResponse struct {
	Session SessionInfo `json:"session"`
	Token   string      `json:"token"`
}

// SessionInfo mirrors the session ID/details needed after creation.
type SessionInfo struct {
	ID string `json:"id"`
}
