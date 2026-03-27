package oidcop

import (
	"context"
	"crypto/rand"
	"crypto/rsa"
	"crypto/x509"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	jose "github.com/go-jose/go-jose/v4"
	"github.com/google/uuid"
	"github.com/zitadel/oidc/v3/pkg/oidc"
	"github.com/zitadel/oidc/v3/pkg/op"

	"github.com/zitadel/zitadel/internal/database"
)

var (
	_ op.Storage                  = &Storage{}
	_ op.ClientCredentialsStorage = &Storage{}
)

// Storage implements op.Storage backed by the ZITADEL database.
type Storage struct {
	db *database.DB
}

// NewStorage creates a new OIDC Storage.
func NewStorage(db *database.DB) *Storage {
	return &Storage{db: db}
}

// ---------- Health ----------

func (s *Storage) Health(_ context.Context) error {
	return s.db.SQL().Ping()
}

// ---------- Client ----------

func (s *Storage) GetClientByClientID(ctx context.Context, clientID string) (op.Client, error) {
	var dataJSON, schemaJSON sql.NullString
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT i.data, COALESCE(sc.schema, '{}')
		 FROM identities i
		 LEFT JOIN schemas sc ON i.schema_id = sc.id
		 WHERE i.identifier = ? AND i.state = 'active'`,
		clientID,
	).Scan(&dataJSON, &schemaJSON)
	if err != nil {
		return nil, fmt.Errorf("client not found: %w", err)
	}

	return ClientFromIdentity(clientID, dataJSON.String, schemaJSON.String)
}

func (s *Storage) AuthorizeClientIDSecret(ctx context.Context, clientID, clientSecret string) error {
	// Look up the identity and its credential.
	var credData string
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT ic.credential_data FROM identity_credentials ic
		 JOIN identities i ON ic.identity_id = i.id
		 WHERE i.identifier = ? AND ic.credential_type = 'client_secret'`,
		clientID,
	).Scan(&credData)
	if err != nil {
		return fmt.Errorf("client not found or no secret configured")
	}

	// credData is JSON: {"hash": "<bcrypt hash>"}
	var cred struct{ Hash string `json:"hash"` }
	if err := json.Unmarshal([]byte(credData), &cred); err != nil {
		return fmt.Errorf("invalid credential data")
	}

	// For the POC, do a simple comparison. Production would use bcrypt.CompareHashAndPassword.
	if cred.Hash != clientSecret {
		return fmt.Errorf("invalid client secret")
	}
	return nil
}

// ClientCredentials implements op.ClientCredentialsStorage.
func (s *Storage) ClientCredentials(ctx context.Context, clientID, clientSecret string) (op.Client, error) {
	if err := s.AuthorizeClientIDSecret(ctx, clientID, clientSecret); err != nil {
		return nil, err
	}
	return s.GetClientByClientID(ctx, clientID)
}

// ClientCredentialsTokenRequest implements op.ClientCredentialsStorage.
func (s *Storage) ClientCredentialsTokenRequest(ctx context.Context, clientID string, scopes []string) (op.TokenRequest, error) {
	// For client_credentials, the client authenticates as itself.
	return &AuthRequest{
		ID:           uuid.NewString(),
		ClientID:     clientID,
		UserID:       clientID, // subject = client itself
		Scopes:       scopes,
		AuthTime:     time.Now(),
		IsDone:       true,
		ResponseType: oidc.ResponseTypeCode,
	}, nil
}

// ---------- Auth Request Lifecycle ----------

func (s *Storage) CreateAuthRequest(ctx context.Context, authReq *oidc.AuthRequest, userID string) (op.AuthRequest, error) {
	id := uuid.NewString()

	var cc, ccm string
	if authReq.CodeChallenge != "" {
		cc = authReq.CodeChallenge
		ccm = string(authReq.CodeChallengeMethod)
	}

	_, err := s.db.SQL().ExecContext(ctx,
		`INSERT INTO oidc_auth_requests (id, client_id, redirect_uri, scopes, state, nonce, response_type, code_challenge, code_challenge_method, user_id)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		id, authReq.ClientID, authReq.RedirectURI,
		strings.Join(authReq.Scopes, " "),
		authReq.State, authReq.Nonce,
		string(authReq.ResponseType),
		cc, ccm, userID,
	)
	if err != nil {
		return nil, fmt.Errorf("create auth request: %w", err)
	}

	return s.authRequestFromRow(ctx, id)
}

func (s *Storage) AuthRequestByID(ctx context.Context, id string) (op.AuthRequest, error) {
	return s.authRequestFromRow(ctx, id)
}

func (s *Storage) AuthRequestByCode(ctx context.Context, code string) (op.AuthRequest, error) {
	var requestID string
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT request_id FROM oidc_codes WHERE code = ?`, code,
	).Scan(&requestID)
	if err != nil {
		return nil, fmt.Errorf("code invalid or expired")
	}
	return s.authRequestFromRow(ctx, requestID)
}

func (s *Storage) SaveAuthCode(ctx context.Context, id string, code string) error {
	_, err := s.db.SQL().ExecContext(ctx,
		`INSERT INTO oidc_codes (code, request_id) VALUES (?, ?)`, code, id,
	)
	return err
}

func (s *Storage) DeleteAuthRequest(ctx context.Context, id string) error {
	_, _ = s.db.SQL().ExecContext(ctx, `DELETE FROM oidc_codes WHERE request_id = ?`, id)
	_, _ = s.db.SQL().ExecContext(ctx, `DELETE FROM oidc_auth_requests WHERE id = ?`, id)
	return nil
}

// CompleteAuthRequest is called by the login flow after successful authentication.
func (s *Storage) CompleteAuthRequest(ctx context.Context, requestID, userID string) error {
	_, err := s.db.SQL().ExecContext(ctx,
		`UPDATE oidc_auth_requests SET user_id = ?, done = 1, auth_time = datetime('now') WHERE id = ?`,
		userID, requestID,
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

	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT client_id, redirect_uri, scopes, state, nonce, response_type,
		        code_challenge, code_challenge_method, user_id, auth_time, done
		 FROM oidc_auth_requests WHERE id = ?`, id,
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
		if t, err := time.Parse("2006-01-02 15:04:05", authTimeStr.String); err == nil {
			req.AuthTime = t
		}
	}

	return req, nil
}

// ---------- Token Creation ----------

func (s *Storage) CreateAccessToken(ctx context.Context, request op.TokenRequest) (string, time.Time, error) {
	var applicationID string
	if authReq, ok := request.(*AuthRequest); ok {
		applicationID = authReq.ClientID
	}

	tokenID := uuid.NewString()
	expiration := time.Now().Add(5 * time.Minute)

	_, err := s.db.SQL().ExecContext(ctx,
		`INSERT INTO oidc_tokens (id, application_id, subject, audience, scopes, expiration)
		 VALUES (?, ?, ?, ?, ?, ?)`,
		tokenID, applicationID, request.GetSubject(),
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

	// Access token
	_, err := s.db.SQL().ExecContext(ctx,
		`INSERT INTO oidc_tokens (id, application_id, subject, audience, scopes, refresh_token_id, expiration)
		 VALUES (?, ?, ?, ?, ?, ?, ?)`,
		tokenID, applicationID, request.GetSubject(),
		strings.Join(request.GetAudience(), " "),
		strings.Join(request.GetScopes(), " "),
		refreshTokenID, expiration.Format(time.RFC3339),
	)
	if err != nil {
		return "", "", time.Time{}, err
	}

	// Refresh token (if new, or renew existing)
	if currentRefreshToken != "" {
		// Delete old refresh token
		_, _ = s.db.SQL().ExecContext(ctx, `DELETE FROM oidc_refresh_tokens WHERE token = ?`, currentRefreshToken)
	}

	refreshExpiration := time.Now().Add(24 * time.Hour)
	// Get auth time from request if available
	var authTimeStr string
	if ar, ok := request.(*AuthRequest); ok {
		authTimeStr = ar.AuthTime.Format(time.RFC3339)
	} else if rr, ok := request.(*RefreshTokenRequest); ok {
		authTimeStr = rr.AuthTime.Format(time.RFC3339)
	} else {
		authTimeStr = time.Now().Format(time.RFC3339)
	}

	_, err = s.db.SQL().ExecContext(ctx,
		`INSERT INTO oidc_refresh_tokens (id, token, application_id, user_id, audience, scopes, auth_time, access_token, expiration)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)`,
		refreshTokenID, refreshTokenID, applicationID, request.GetSubject(),
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

// ---------- Refresh Token ----------

func (s *Storage) TokenRequestByRefreshToken(ctx context.Context, refreshToken string) (op.RefreshTokenRequest, error) {
	var (
		id, applicationID, userID, audienceStr, scopesStr string
		authTimeStr, expirationStr                        string
	)
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT id, application_id, user_id, audience, scopes, auth_time, expiration
		 FROM oidc_refresh_tokens WHERE token = ?`, refreshToken,
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

func (r *RefreshTokenRequest) GetAMR() []string          { return nil }
func (r *RefreshTokenRequest) GetAudience() []string     { return r.Audience }
func (r *RefreshTokenRequest) GetAuthTime() time.Time    { return r.AuthTime }
func (r *RefreshTokenRequest) GetClientID() string       { return r.ApplicationID }
func (r *RefreshTokenRequest) GetScopes() []string       { return r.Scopes }
func (r *RefreshTokenRequest) GetSubject() string        { return r.UserID }
func (r *RefreshTokenRequest) SetCurrentScopes(scopes []string) { r.Scopes = scopes }

// ---------- Revocation + Session Termination ----------

func (s *Storage) RevokeToken(ctx context.Context, tokenIDOrToken string, userID string, clientID string) *oidc.Error {
	// Try as access token ID
	_, _ = s.db.SQL().ExecContext(ctx, `DELETE FROM oidc_tokens WHERE id = ? AND application_id = ?`, tokenIDOrToken, clientID)
	// Try as refresh token
	_, _ = s.db.SQL().ExecContext(ctx, `DELETE FROM oidc_refresh_tokens WHERE token = ? AND application_id = ?`, tokenIDOrToken, clientID)
	return nil
}

func (s *Storage) GetRefreshTokenInfo(ctx context.Context, clientID string, token string) (string, string, error) {
	var userID, id string
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT user_id, id FROM oidc_refresh_tokens WHERE token = ?`, token,
	).Scan(&userID, &id)
	if err != nil {
		return "", "", op.ErrInvalidRefreshToken
	}
	return userID, id, nil
}

func (s *Storage) TerminateSession(ctx context.Context, userID string, clientID string) error {
	_, _ = s.db.SQL().ExecContext(ctx,
		`DELETE FROM oidc_tokens WHERE subject = ? AND application_id = ?`, userID, clientID)
	_, _ = s.db.SQL().ExecContext(ctx,
		`DELETE FROM oidc_refresh_tokens WHERE user_id = ? AND application_id = ?`, userID, clientID)
	return nil
}

// ---------- Signing Keys ----------

func (s *Storage) SigningKey(ctx context.Context) (op.SigningKey, error) {
	key, err := s.getOrCreateSigningKey(ctx)
	if err != nil {
		return nil, err
	}
	return key, nil
}

func (s *Storage) SignatureAlgorithms(_ context.Context) ([]jose.SignatureAlgorithm, error) {
	return []jose.SignatureAlgorithm{jose.RS256}, nil
}

func (s *Storage) KeySet(ctx context.Context) ([]op.Key, error) {
	sk, err := s.getOrCreateSigningKey(ctx)
	if err != nil {
		return nil, err
	}
	return []op.Key{&publicKey{sk}}, nil
}

type signingKeyData struct {
	id  string
	key *rsa.PrivateKey
}

func (sk *signingKeyData) SignatureAlgorithm() jose.SignatureAlgorithm { return jose.RS256 }
func (sk *signingKeyData) Key() any                                    { return sk.key }
func (sk *signingKeyData) ID() string                                  { return sk.id }

type publicKey struct {
	*signingKeyData
}

func (pk *publicKey) Algorithm() jose.SignatureAlgorithm { return jose.RS256 }
func (pk *publicKey) Use() string                        { return "sig" }
func (pk *publicKey) Key() any                           { return &pk.key.PublicKey }

func (s *Storage) getOrCreateSigningKey(ctx context.Context) (*signingKeyData, error) {
	var id string
	var keyBytes []byte
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT id, private_key FROM oidc_signing_keys ORDER BY created_at DESC LIMIT 1`,
	).Scan(&id, &keyBytes)
	if err == nil {
		pk, err := x509.ParsePKCS1PrivateKey(keyBytes)
		if err != nil {
			return nil, fmt.Errorf("parse signing key: %w", err)
		}
		return &signingKeyData{id: id, key: pk}, nil
	}

	// Generate new key
	key, err := rsa.GenerateKey(rand.Reader, 2048)
	if err != nil {
		return nil, fmt.Errorf("generate signing key: %w", err)
	}
	id = uuid.NewString()
	keyDER := x509.MarshalPKCS1PrivateKey(key)

	_, err = s.db.SQL().ExecContext(ctx,
		`INSERT INTO oidc_signing_keys (id, algorithm, private_key) VALUES (?, 'RS256', ?)`,
		id, keyDER,
	)
	if err != nil {
		return nil, fmt.Errorf("store signing key: %w", err)
	}

	return &signingKeyData{id: id, key: key}, nil
}

// ---------- Userinfo ----------

func (s *Storage) SetUserinfoFromScopes(ctx context.Context, userinfo *oidc.UserInfo, userID, clientID string, scopes []string) error {
	return s.setUserinfo(ctx, userinfo, userID, scopes)
}

func (s *Storage) SetUserinfoFromRequest(ctx context.Context, userinfo *oidc.UserInfo, request op.IDTokenRequest, scopes []string) error {
	return s.setUserinfo(ctx, userinfo, request.GetSubject(), scopes)
}

func (s *Storage) SetUserinfoFromToken(ctx context.Context, userinfo *oidc.UserInfo, tokenID, subject, origin string) error {
	// Verify token exists and not expired
	var expirationStr string
	var scopesStr string
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT scopes, expiration FROM oidc_tokens WHERE id = ?`, tokenID,
	).Scan(&scopesStr, &expirationStr)
	if err != nil {
		return fmt.Errorf("token invalid")
	}
	exp, _ := time.Parse(time.RFC3339, expirationStr)
	if exp.Before(time.Now()) {
		return fmt.Errorf("token expired")
	}
	return s.setUserinfo(ctx, userinfo, subject, strings.Split(scopesStr, " "))
}

func (s *Storage) setUserinfo(ctx context.Context, userinfo *oidc.UserInfo, userID string, scopes []string) error {
	var identifier string
	var dataJSON sql.NullString

	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT identifier, data FROM identities WHERE identifier = ? OR CAST(id AS TEXT) = ?`,
		userID, userID,
	).Scan(&identifier, &dataJSON)
	if err != nil {
		return fmt.Errorf("user not found: %w", err)
	}

	var data map[string]any
	if dataJSON.Valid {
		_ = json.Unmarshal([]byte(dataJSON.String), &data)
	}

	for _, scope := range scopes {
		switch scope {
		case oidc.ScopeOpenID:
			userinfo.Subject = userID
		case oidc.ScopeEmail:
			if email, ok := data["email"].(string); ok {
				userinfo.Email = email
				userinfo.EmailVerified = oidc.Bool(true)
			} else {
				// Fall back to identifier if it looks like an email
				if strings.Contains(identifier, "@") {
					userinfo.Email = identifier
					userinfo.EmailVerified = oidc.Bool(true)
				}
			}
		case oidc.ScopeProfile:
			userinfo.PreferredUsername = identifier
			if dn, ok := data["display_name"].(string); ok {
				userinfo.Name = dn
			}
			if fn, ok := data["first_name"].(string); ok {
				userinfo.GivenName = fn
			}
			if ln, ok := data["last_name"].(string); ok {
				userinfo.FamilyName = ln
			}
		case oidc.ScopePhone:
			if phone, ok := data["phone"].(string); ok {
				userinfo.PhoneNumber = phone
				userinfo.PhoneNumberVerified = oidc.Bool(true)
			}
		}
	}
	return nil
}

// ---------- Introspection ----------

func (s *Storage) SetIntrospectionFromToken(ctx context.Context, introspection *oidc.IntrospectionResponse, tokenID, subject, clientID string) error {
	var expirationStr, scopesStr, applicationID string
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT scopes, expiration, application_id FROM oidc_tokens WHERE id = ?`, tokenID,
	).Scan(&scopesStr, &expirationStr, &applicationID)
	if err != nil {
		return fmt.Errorf("token invalid")
	}
	exp, _ := time.Parse(time.RFC3339, expirationStr)
	if exp.Before(time.Now()) {
		return fmt.Errorf("token expired")
	}

	introspection.Expiration = oidc.FromTime(exp)
	introspection.Scope = strings.Split(scopesStr, " ")
	introspection.ClientID = applicationID

	userinfo := new(oidc.UserInfo)
	if err := s.setUserinfo(ctx, userinfo, subject, introspection.Scope); err == nil {
		introspection.SetUserInfo(userinfo)
	}
	return nil
}

// ---------- Private Claims ----------

func (s *Storage) GetPrivateClaimsFromScopes(ctx context.Context, userID, clientID string, scopes []string) (map[string]any, error) {
	return nil, nil
}

// ---------- JWT Profile (not used in POC) ----------

func (s *Storage) GetKeyByIDAndClientID(ctx context.Context, keyID, clientID string) (*jose.JSONWebKey, error) {
	return nil, fmt.Errorf("JWT profile not supported")
}

func (s *Storage) ValidateJWTProfileScopes(ctx context.Context, userID string, scopes []string) ([]string, error) {
	return scopes, nil
}
