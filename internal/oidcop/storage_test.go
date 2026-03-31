package oidcop

import (
	"context"
	"encoding/json"
	"path/filepath"
	"testing"
	"time"

	"github.com/zitadel/oidc/v3/pkg/oidc"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/bootstrap"
	"github.com/zitadel/zitadel/internal/crypto"
	"github.com/zitadel/zitadel/internal/database"
	"github.com/zitadel/zitadel/internal/id"
)

func newStorageWithServer(t *testing.T) (*Storage, *database.DB) {
	t.Helper()

	dir := t.TempDir()
	db, err := database.Open("sqlite://" + filepath.Join(dir, "test.db"))
	if err != nil {
		t.Fatalf("database.Open() error = %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	if err := database.Migrate(db); err != nil {
		t.Fatalf("database.Migrate() error = %v", err)
	}
	if err := bootstrap.EnsureAdmin(t.Context(), db, ""); err != nil {
		t.Fatalf("EnsureAdmin() error = %v", err)
	}

	box, err := crypto.NewSecretBox("", nil)
	if err != nil {
		t.Fatalf("NewSecretBox() error = %v", err)
	}
	store := crypto.NewSecretStore(db.SQL(), box)
	return NewStorage(db, store), db
}

func insertUser(t *testing.T, db *database.DB, identifier string, metadata map[string]any) string {
	t.Helper()

	raw, _ := json.Marshal(metadata)
	userID := id.New()
	now := time.Now().UTC().Format("2006-01-02 15:04:05")
	_, err := db.SQL().Exec(
		`INSERT INTO users (id, org_id, identifier, display_name, user_type, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, '', ?, ?, 'human', 'active', 'human_user_v1', ?, ?, ?)`,
		userID, identifier, identifier, string(raw), now, now,
	)
	if err != nil {
		t.Fatalf("insert user: %v", err)
	}
	return userID
}

func insertApp(t *testing.T, db *database.DB, clientID, clientSecret string) {
	t.Helper()

	secretHash, err := auth.HashSecret(clientSecret)
	if err != nil {
		t.Fatalf("HashSecret() error = %v", err)
	}

	now := time.Now().UTC().Format("2006-01-02 15:04:05")
	_, err = db.SQL().Exec(
		`INSERT INTO apps (id, org_id, name, app_type, client_id, client_secret, redirect_uris, grant_types, response_types, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, '', 'Secret App', 'web', ?, ?, '[]', '[]', '[]', 'active', 'app_v1', '{}', ?, ?)`,
		id.New(), clientID, secretHash, now, now,
	)
	if err != nil {
		t.Fatalf("insert app: %v", err)
	}
}

func TestStorageAuthRequestLifecycle(t *testing.T) {
	storage, _ := newStorageWithServer(t)

	maxAge := uint(60)
	authReq := &oidc.AuthRequest{
		ClientID:            "console",
		RedirectURI:         "https://example.com/callback",
		Scopes:              []string{"openid", "email"},
		State:               "state-123",
		Nonce:               "nonce-123",
		ResponseType:        oidc.ResponseTypeCode,
		CodeChallenge:       "challenge-123",
		CodeChallengeMethod: oidc.CodeChallengeMethodS256,
		Prompt:              []string{"login"},
		LoginHint:           "hint@example.com",
		MaxAge:              &maxAge,
	}

	created, err := storage.CreateAuthRequest(t.Context(), authReq, "user-1")
	if err != nil {
		t.Fatalf("CreateAuthRequest() error = %v", err)
	}
	got := created.(*AuthRequest)
	if got.ClientID != "console" || got.RedirectURI != authReq.RedirectURI {
		t.Fatalf("created auth request = %#v", got)
	}

	if err := storage.SaveAuthCode(t.Context(), got.ID, "code-123"); err != nil {
		t.Fatalf("SaveAuthCode() error = %v", err)
	}
	byCode, err := storage.AuthRequestByCode(t.Context(), "code-123")
	if err != nil {
		t.Fatalf("AuthRequestByCode() error = %v", err)
	}
	if byCode.GetState() != "state-123" {
		t.Fatalf("AuthRequestByCode().GetState() = %q", byCode.GetState())
	}

	if err := storage.CompleteAuthRequest(t.Context(), got.ID, "user-1"); err != nil {
		t.Fatalf("CompleteAuthRequest() error = %v", err)
	}
	completed, err := storage.AuthRequestByID(t.Context(), got.ID)
	if err != nil {
		t.Fatalf("AuthRequestByID() error = %v", err)
	}
	if !completed.Done() {
		t.Fatal("expected completed auth request to be done")
	}

	if err := storage.DeleteAuthRequest(t.Context(), got.ID); err != nil {
		t.Fatalf("DeleteAuthRequest() error = %v", err)
	}
	if _, err := storage.AuthRequestByID(t.Context(), got.ID); err == nil {
		t.Fatal("expected deleted auth request lookup to fail")
	}
}

func TestStorageCreateAccessAndRefreshTokensLifecycle(t *testing.T) {
	storage, db := newStorageWithServer(t)
	userID := insertUser(t, db, "refresh-user@example.com", map[string]any{"email": "refresh-user@example.com"})

	req := &AuthRequest{
		ID:       "auth-1",
		ClientID: "console",
		UserID:   userID,
		Scopes:   []string{"openid", "email"},
		AuthTime: time.Now().UTC(),
	}

	accessToken, refreshToken, expiration, err := storage.CreateAccessAndRefreshTokens(t.Context(), req, "")
	if err != nil {
		t.Fatalf("CreateAccessAndRefreshTokens() error = %v", err)
	}
	if accessToken == "" || refreshToken == "" || expiration.IsZero() {
		t.Fatalf("token outputs are incomplete: %q %q %v", accessToken, refreshToken, expiration)
	}

	refreshReq, err := storage.TokenRequestByRefreshToken(t.Context(), refreshToken)
	if err != nil {
		t.Fatalf("TokenRequestByRefreshToken() error = %v", err)
	}
	if refreshReq.GetSubject() != userID {
		t.Fatalf("GetSubject() = %q, want %q", refreshReq.GetSubject(), userID)
	}

	gotUserID, refreshID, err := storage.GetRefreshTokenInfo(t.Context(), "console", refreshToken)
	if err != nil {
		t.Fatalf("GetRefreshTokenInfo() error = %v", err)
	}
	if gotUserID != userID || refreshID == "" {
		t.Fatalf("GetRefreshTokenInfo() = %q %q", gotUserID, refreshID)
	}

	storage.RevokeToken(t.Context(), refreshToken, userID, "console")
	if _, err := storage.TokenRequestByRefreshToken(t.Context(), refreshToken); err == nil {
		t.Fatal("expected revoked refresh token to fail lookup")
	}
}

func TestStorageSigningKeyIsStableAcrossReads(t *testing.T) {
	storage, _ := newStorageWithServer(t)

	first, err := storage.SigningKey(t.Context())
	if err != nil {
		t.Fatalf("SigningKey() error = %v", err)
	}
	second, err := storage.SigningKey(t.Context())
	if err != nil {
		t.Fatalf("SigningKey() second error = %v", err)
	}
	if first.ID() != second.ID() {
		t.Fatalf("signing key id changed: %q -> %q", first.ID(), second.ID())
	}

	keys, err := storage.KeySet(t.Context())
	if err != nil {
		t.Fatalf("KeySet() error = %v", err)
	}
	if len(keys) != 1 {
		t.Fatalf("KeySet() len = %d, want 1", len(keys))
	}
}

func TestStorageUserinfoAndIntrospection(t *testing.T) {
	storage, db := newStorageWithServer(t)

	metadata := map[string]any{
		"email":        "claims@example.com",
		"display_name": "Claims User",
		"first_name":   "Claims",
		"last_name":    "User",
		"phone":        "+15550001",
		"locale":       "en-US",
	}
	userID := insertUser(t, db, "claims@example.com", metadata)

	req := &AuthRequest{
		ClientID: "console",
		UserID:   userID,
		Scopes:   []string{"openid", "email", "profile", "phone"},
		AuthTime: time.Now().UTC(),
	}
	accessToken, _, _, err := storage.CreateAccessAndRefreshTokens(t.Context(), req, "")
	if err != nil {
		t.Fatalf("CreateAccessAndRefreshTokens() error = %v", err)
	}

	userinfo := new(oidc.UserInfo)
	if err := storage.SetUserinfoFromToken(t.Context(), userinfo, accessToken, userID, "console"); err != nil {
		t.Fatalf("SetUserinfoFromToken() error = %v", err)
	}
	if got := userinfo.Email; got != "claims@example.com" {
		t.Fatalf("userinfo.Email = %q", got)
	}
	if got := userinfo.Name; got != "Claims User" {
		t.Fatalf("userinfo.Name = %q", got)
	}
	if got := userinfo.PhoneNumber; got != "+15550001" {
		t.Fatalf("userinfo.PhoneNumber = %q", got)
	}

	introspection := new(oidc.IntrospectionResponse)
	if err := storage.SetIntrospectionFromToken(t.Context(), introspection, accessToken, userID, "console"); err != nil {
		t.Fatalf("SetIntrospectionFromToken() error = %v", err)
	}
	if introspection.ClientID != "console" {
		t.Fatalf("introspection.ClientID = %q", introspection.ClientID)
	}
	if len(introspection.Scope) == 0 {
		t.Fatal("expected introspection scopes")
	}
}

func TestStorageAuthorizeClientIDSecretFromCreatedApp(t *testing.T) {
	storage, db := newStorageWithServer(t)
	insertApp(t, db, "secret-app", "super-secret")

	if err := storage.AuthorizeClientIDSecret(context.Background(), "secret-app", "super-secret"); err != nil {
		t.Fatalf("AuthorizeClientIDSecret() error = %v", err)
	}
	if _, err := storage.ClientCredentials(context.Background(), "secret-app", "super-secret"); err != nil {
		t.Fatalf("ClientCredentials() error = %v", err)
	}

	req, err := storage.ClientCredentialsTokenRequest(context.Background(), "secret-app", []string{"openid"})
	if err != nil {
		t.Fatalf("ClientCredentialsTokenRequest() error = %v", err)
	}
	if req.GetSubject() != "secret-app" {
		t.Fatalf("GetSubject() = %q, want secret-app", req.GetSubject())
	}
}

func TestParseLocale_InvalidFallsBackToUnd(t *testing.T) {
	if got := parseLocale("!"); got.String() != "und" {
		t.Fatalf("parseLocale() = %q, want und", got.String())
	}
}

func TestBuildClientDataJSON_IgnoresInvalidJSON(t *testing.T) {
	got := buildClientDataJSON(`{"post_logout_redirect_uris":["https://example.com/logout"]}`, "spa", "nope", "[]", "[]")
	var parsed map[string]any
	if err := json.Unmarshal([]byte(got), &parsed); err != nil {
		t.Fatalf("unmarshal result: %v", err)
	}
	if parsed["app_type"] != "spa" {
		t.Fatalf("app_type = %v", parsed["app_type"])
	}
}

func TestStorageOpenWithDatabaseOpenHelper(t *testing.T) {
	dir := t.TempDir()
	db, err := database.Open("sqlite://" + filepath.Join(dir, "oidcop.db"))
	if err != nil {
		t.Fatalf("database.Open() error = %v", err)
	}
	t.Cleanup(func() { _ = db.Close() })

	if err := database.Migrate(db); err != nil {
		t.Fatalf("database.Migrate() error = %v", err)
	}

	box, err := crypto.NewSecretBox("", nil)
	if err != nil {
		t.Fatalf("NewSecretBox() error = %v", err)
	}
	store := crypto.NewSecretStore(db.SQL(), box)
	storage := NewStorage(db, store)

	if err := storage.Health(t.Context()); err != nil {
		t.Fatalf("Health() error = %v", err)
	}
}
