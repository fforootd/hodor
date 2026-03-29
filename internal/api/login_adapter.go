package api

import (
	"context"

	"github.com/zitadel/zitadel/internal/login"
)

// CreateSessionInternal adapts the login.SessionCreator interface.
// This converts login.ClientSignals → api.ClientSignals to satisfy the interface
// without creating an import cycle.
func (a *API) CreateSessionForLogin(ctx context.Context, userID string, userAgent, ipAddress string, signals *login.ClientSignals) (*login.CreateSessionResponse, error) {
	var apiSignals *ClientSignals
	if signals != nil {
		apiSignals = &ClientSignals{
			CaptchaProvider: signals.CaptchaProvider,
			CaptchaVerified: signals.CaptchaVerified,
			CaptchaScore:    signals.CaptchaScore,
			PoWCompleted:    signals.PoWCompleted,
			PoWDurationMs:   signals.PoWDurationMs,
			VisitorID:       signals.VisitorID,
			FingerprintHash: signals.FingerprintHash,
			RequestID:       signals.RequestID,
		}
	}
	resp, err := a.CreateSessionInternal(ctx, userID, userAgent, ipAddress, apiSignals)
	if err != nil {
		return nil, err
	}
	return &login.CreateSessionResponse{
		Session: login.SessionInfo{ID: resp.Session.ID},
		Token:   resp.Token,
	}, nil
}
