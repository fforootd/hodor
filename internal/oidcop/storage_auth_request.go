package oidcop

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/google/uuid"
	"github.com/zitadel/oidc/v3/pkg/oidc"
	"github.com/zitadel/oidc/v3/pkg/op"
)

func (s *Storage) CreateAuthRequest(ctx context.Context, authReq *oidc.AuthRequest, userID string) (op.AuthRequest, error) {
	id := uuid.NewString()

	var cc, ccm string
	if authReq.CodeChallenge != "" {
		cc = authReq.CodeChallenge
		ccm = string(authReq.CodeChallengeMethod)
	}
	dataJSON := encodeAuthRequestData(authReq)

	scoped := s.scoped(ctx)
	expiresAt := time.Now().UTC().Add(10 * time.Minute).Format(time.RFC3339)
	_, err := scoped.ExecContext(ctx, scoped.Rebind(
		`INSERT INTO auth_states (id, instance_id, type, client_id, redirect_uri, scopes, state, nonce, response_type, code_challenge, code_challenge_method, user_id, data, expires_at)
		 VALUES (?, ?, 'oidc_auth', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`),
		id, scoped.InstanceID(), authReq.ClientID, authReq.RedirectURI,
		strings.Join(authReq.Scopes, " "),
		authReq.State, authReq.Nonce,
		string(authReq.ResponseType),
		cc, ccm, userID, dataJSON, expiresAt,
	)
	if err != nil {
		return nil, fmt.Errorf("create auth request: %w", err)
	}

	return s.authRequestFromRow(ctx, id)
}

func encodeAuthRequestData(authReq *oidc.AuthRequest) string {
	data := map[string]any{}
	if len(authReq.Prompt) > 0 {
		data["prompt"] = []string(authReq.Prompt)
	}
	if authReq.LoginHint != "" {
		data["login_hint"] = authReq.LoginHint
	}
	if authReq.MaxAge != nil {
		data["max_age"] = *authReq.MaxAge
	}

	encoded, err := json.Marshal(data)
	if err != nil {
		return "{}"
	}
	return string(encoded)
}

func (s *Storage) AuthRequestByID(ctx context.Context, id string) (op.AuthRequest, error) {
	return s.authRequestFromRow(ctx, id)
}

func (s *Storage) AuthRequestByCode(ctx context.Context, code string) (op.AuthRequest, error) {
	scoped := s.scoped(ctx)
	var requestID string
	err := scoped.QueryRowContext(ctx,
		scoped.Rebind(`SELECT id FROM auth_states WHERE code = ? AND type = 'oidc_auth' AND instance_id = ?`), code, scoped.InstanceID(),
	).Scan(&requestID)
	if err != nil {
		return nil, fmt.Errorf("code invalid or expired")
	}
	return s.authRequestFromRow(ctx, requestID)
}

func (s *Storage) SaveAuthCode(ctx context.Context, id string, code string) error {
	scoped := s.scoped(ctx)
	_, err := scoped.ExecContext(ctx,
		scoped.Rebind(`UPDATE auth_states SET code = ? WHERE id = ? AND instance_id = ?`), code, id, scoped.InstanceID(),
	)
	return err
}

func (s *Storage) DeleteAuthRequest(ctx context.Context, id string) error {
	scoped := s.scoped(ctx)
	_, _ = scoped.ExecContext(ctx, scoped.Rebind(`DELETE FROM auth_states WHERE id = ? AND instance_id = ?`), id, scoped.InstanceID())
	return nil
}

// CompleteAuthRequest is called by the login flow after successful authentication.
func (s *Storage) CompleteAuthRequest(ctx context.Context, requestID, userID string) error {
	scoped := s.scoped(ctx)
	_, err := scoped.ExecContext(ctx, scoped.Rebind(
		`UPDATE auth_states SET user_id = ?, done = 1, auth_time = ? WHERE id = ? AND instance_id = ?`),
		userID, time.Now().UTC().Format(time.RFC3339), requestID, scoped.InstanceID(),
	)
	return err
}

func (s *Storage) authRequestFromRow(ctx context.Context, id string) (*AuthRequest, error) {
	var (
		clientID, redirectURI, scopesStr, state, nonce string
		responseType, cc, ccm, userID                  string
		authTimeStr                                    sql.NullString
		done                                           int
	)

	scoped := s.scoped(ctx)
	err := scoped.QueryRowContext(ctx, scoped.Rebind(
		`SELECT client_id, redirect_uri, scopes, state, nonce, response_type,
		        code_challenge, code_challenge_method, user_id, auth_time, done
		 FROM auth_states WHERE id = ? AND instance_id = ?`), id, scoped.InstanceID(),
	).Scan(&clientID, &redirectURI, &scopesStr, &state, &nonce, &responseType,
		&cc, &ccm, &userID, &authTimeStr, &done)
	if err != nil {
		return nil, fmt.Errorf("auth request not found: %w", err)
	}

	req := &AuthRequest{
		ID:           id,
		ClientID:     clientID,
		RedirectURI:  redirectURI,
		Scopes:       strings.Split(scopesStr, " "),
		State:        state,
		Nonce:        nonce,
		ResponseType: oidc.ResponseType(responseType),
		UserID:       userID,
		IsDone:       done == 1,
	}

	if cc != "" {
		req.CodeChallenge = &oidc.CodeChallenge{
			Challenge: cc,
			Method:    oidc.CodeChallengeMethod(ccm),
		}
	}

	if authTimeStr.Valid && authTimeStr.String != "" {
		if t, ok := parseStoredTimestamp(authTimeStr.String); ok {
			req.AuthTime = t
		}
	}

	return req, nil
}
