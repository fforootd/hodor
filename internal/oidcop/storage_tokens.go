package oidcop

import (
	"context"
	"fmt"
	"strings"
	"time"

	jose "github.com/go-jose/go-jose/v4"
	"github.com/google/uuid"
	"github.com/zitadel/oidc/v3/pkg/oidc"
	"github.com/zitadel/oidc/v3/pkg/op"
)

func (s *Storage) CreateAccessToken(ctx context.Context, request op.TokenRequest) (string, time.Time, error) {
	var applicationID string
	if authReq, ok := request.(*AuthRequest); ok {
		applicationID = authReq.ClientID
	}

	tokenID := uuid.NewString()
	expiration := time.Now().Add(5 * time.Minute)

	tokenHash := tokenID
	_, err := s.db.SQL().ExecContext(ctx,
		`INSERT INTO tokens (id, type, token_hash, user_id, application_id, audience, scopes, expires_at)
		 VALUES (?, 'oidc_access', ?, ?, ?, ?, ?, ?)`,
		tokenID, tokenHash, request.GetSubject(), applicationID,
		strings.Join(request.GetAudience(), " "),
		strings.Join(request.GetScopes(), " "),
		expiration.Format(time.RFC3339),
	)
	if err != nil {
		return "", time.Time{}, err
	}
	return tokenID, expiration, nil
}

func (s *Storage) CreateAccessAndRefreshTokens(ctx context.Context, request op.TokenRequest, currentRefreshToken string) (string, string, time.Time, error) {
	var applicationID string
	if authReq, ok := request.(*AuthRequest); ok {
		applicationID = authReq.ClientID
	}

	tokenID := uuid.NewString()
	refreshTokenID := uuid.NewString()
	expiration := time.Now().Add(5 * time.Minute)

	tokenHash := tokenID
	_, err := s.db.SQL().ExecContext(ctx,
		`INSERT INTO tokens (id, type, token_hash, user_id, application_id, audience, scopes, refresh_token_id, expires_at)
		 VALUES (?, 'oidc_access', ?, ?, ?, ?, ?, ?, ?)`,
		tokenID, tokenHash, request.GetSubject(), applicationID,
		strings.Join(request.GetAudience(), " "),
		strings.Join(request.GetScopes(), " "),
		refreshTokenID, expiration.Format(time.RFC3339),
	)
	if err != nil {
		return "", "", time.Time{}, err
	}

	if currentRefreshToken != "" {
		_, _ = s.db.SQL().ExecContext(ctx, `DELETE FROM tokens WHERE token_hash = ? AND type = 'oidc_refresh'`, currentRefreshToken)
	}

	refreshExpiration := time.Now().Add(24 * time.Hour)
	var authTimeStr string
	if ar, ok := request.(*AuthRequest); ok {
		authTimeStr = ar.AuthTime.Format(time.RFC3339)
	} else if rr, ok := request.(*RefreshTokenRequest); ok {
		authTimeStr = rr.AuthTime.Format(time.RFC3339)
	} else {
		authTimeStr = time.Now().Format(time.RFC3339)
	}

	_, err = s.db.SQL().ExecContext(ctx,
		`INSERT INTO tokens (id, type, token_hash, user_id, application_id, audience, scopes, auth_time, refresh_token_id, expires_at)
		 VALUES (?, 'oidc_refresh', ?, ?, ?, ?, ?, ?, ?, ?)`,
		refreshTokenID, refreshTokenID, request.GetSubject(), applicationID,
		strings.Join(request.GetAudience(), " "),
		strings.Join(request.GetScopes(), " "),
		authTimeStr,
		tokenID, refreshExpiration.Format(time.RFC3339),
	)
	if err != nil {
		return "", "", time.Time{}, err
	}

	return tokenID, refreshTokenID, expiration, nil
}

func (s *Storage) TokenRequestByRefreshToken(ctx context.Context, refreshToken string) (op.RefreshTokenRequest, error) {
	var (
		id, applicationID, userID, audienceStr, scopesStr string
		authTimeStr, expirationStr                        string
	)
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT id, application_id, user_id, audience, scopes, auth_time, expires_at
		 FROM tokens WHERE token_hash = ? AND type = 'oidc_refresh'`, refreshToken,
	).Scan(&id, &applicationID, &userID, &audienceStr, &scopesStr, &authTimeStr, &expirationStr)
	if err != nil {
		return nil, fmt.Errorf("invalid refresh token")
	}

	authTime, _ := time.Parse(time.RFC3339, authTimeStr)
	expiration, _ := time.Parse(time.RFC3339, expirationStr)

	if expiration.Before(time.Now()) {
		return nil, fmt.Errorf("expired refresh token")
	}

	return &RefreshTokenRequest{
		ID:            id,
		ApplicationID: applicationID,
		UserID:        userID,
		Audience:      strings.Split(audienceStr, " "),
		Scopes:        strings.Split(scopesStr, " "),
		AuthTime:      authTime,
	}, nil
}

// RefreshTokenRequest implements op.RefreshTokenRequest.
type RefreshTokenRequest struct {
	ID            string
	ApplicationID string
	UserID        string
	Audience      []string
	Scopes        []string
	AuthTime      time.Time
}

func (r *RefreshTokenRequest) GetAMR() []string                 { return nil }
func (r *RefreshTokenRequest) GetAudience() []string            { return r.Audience }
func (r *RefreshTokenRequest) GetAuthTime() time.Time           { return r.AuthTime }
func (r *RefreshTokenRequest) GetClientID() string              { return r.ApplicationID }
func (r *RefreshTokenRequest) GetScopes() []string              { return r.Scopes }
func (r *RefreshTokenRequest) GetSubject() string               { return r.UserID }
func (r *RefreshTokenRequest) SetCurrentScopes(scopes []string) { r.Scopes = scopes }

func (s *Storage) RevokeToken(ctx context.Context, tokenIDOrToken string, userID string, clientID string) *oidc.Error {
	_, _ = s.db.SQL().ExecContext(ctx, `DELETE FROM tokens WHERE id = ? AND application_id = ? AND type = 'oidc_access'`, tokenIDOrToken, clientID)
	_, _ = s.db.SQL().ExecContext(ctx, `DELETE FROM tokens WHERE token_hash = ? AND application_id = ? AND type = 'oidc_refresh'`, tokenIDOrToken, clientID)
	return nil
}

func (s *Storage) GetRefreshTokenInfo(ctx context.Context, clientID string, token string) (string, string, error) {
	var userID, id string
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT user_id, id FROM tokens WHERE token_hash = ? AND type = 'oidc_refresh'`, token,
	).Scan(&userID, &id)
	if err != nil {
		return "", "", op.ErrInvalidRefreshToken
	}
	return userID, id, nil
}

func (s *Storage) TerminateSession(ctx context.Context, userID string, clientID string) error {
	_, _ = s.db.SQL().ExecContext(ctx,
		`DELETE FROM tokens WHERE user_id = ? AND application_id = ? AND type = 'oidc_access'`, userID, clientID)
	_, _ = s.db.SQL().ExecContext(ctx,
		`DELETE FROM tokens WHERE user_id = ? AND application_id = ? AND type = 'oidc_refresh'`, userID, clientID)
	return nil
}

func (s *Storage) GetKeyByIDAndClientID(ctx context.Context, keyID, clientID string) (*jose.JSONWebKey, error) {
	return nil, fmt.Errorf("JWT profile not supported")
}

func (s *Storage) ValidateJWTProfileScopes(ctx context.Context, userID string, scopes []string) ([]string, error) {
	return scopes, nil
}
