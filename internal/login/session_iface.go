package login

import "context"

// SessionCreator abstracts session creation, breaking the import cycle
// between login and api packages.
type SessionCreator interface {
	CreateSessionForLogin(ctx context.Context, userID string, userAgent, ipAddress string, signals *ClientSignals) (*CreateSessionResponse, error)
	EmitAuthEvent(ctx context.Context, eventType string, actorID string, payload map[string]any)
}

// ClientSignals contains client-side signals collected during a login flow.
// Mirrored from api.ClientSignals to avoid import cycle.
type ClientSignals struct {
	CaptchaProvider string  `json:"captcha_provider,omitempty"`
	CaptchaVerified bool    `json:"captcha_verified,omitempty"`
	CaptchaScore    float64 `json:"captcha_score,omitempty"`
	PoWCompleted    bool    `json:"pow_completed,omitempty"`
	PoWDurationMs   float64 `json:"pow_duration_ms,omitempty"`
	VisitorID       string  `json:"visitor_id,omitempty"`
	FingerprintHash string  `json:"fingerprint_hash,omitempty"`
	TraceID         string  `json:"trace_id,omitempty"`
	DocumentLoadMs  float64 `json:"document_load_ms,omitempty"`
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
