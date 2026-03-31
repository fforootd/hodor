package api

import (
	"context"

	"github.com/zitadel/zitadel/internal/login"
	"github.com/zitadel/zitadel/internal/risk"
)

// CreateSessionInternal adapts the login.SessionCreator interface.
// This satisfies the interface without creating an import cycle.
func (a *API) CreateSessionForLogin(ctx context.Context, userID string, userAgent, ipAddress string, signals *risk.Signals, provenance *login.SessionProvenance) (*login.CreateSessionResponse, error) {
	resp, err := a.CreateSessionInternal(ctx, userID, userAgent, ipAddress, signals, provenance)
	if err != nil {
		return nil, err
	}
	return &login.CreateSessionResponse{
		Session: login.SessionInfo{ID: resp.Session.ID},
		Token:   resp.Token,
	}, nil
}
