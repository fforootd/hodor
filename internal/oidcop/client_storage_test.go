package oidcop

import (
	"path/filepath"
	"reflect"
	"testing"

	"github.com/zitadel/oidc/v3/pkg/oidc"
	"github.com/zitadel/oidc/v3/pkg/op"

	"github.com/zitadel/zitadel/internal/database"
)

func TestClientFromIdentity_UsesAppDataOverrides(t *testing.T) {
	t.Parallel()

	client, err := ClientFromIdentity(
		"client-123",
		`{
			"app_type": "native",
			"redirect_uris": ["http://localhost:3000/callback"],
			"post_logout_redirect_uris": ["http://localhost:3000/logout"],
			"grant_types": ["authorization_code"],
			"response_types": ["code"]
		}`,
		`{
			"x-oidc": {
				"grant_types": ["client_credentials"],
				"response_types": ["token"],
				"token_endpoint_auth_method": "client_secret_post",
				"access_token_type": "jwt"
			}
		}`,
	)
	if err != nil {
		t.Fatalf("ClientFromIdentity() error = %v", err)
	}

	if got, want := client.ApplicationType(), op.ApplicationTypeNative; got != want {
		t.Fatalf("ApplicationType() = %v, want %v", got, want)
	}
	if got, want := client.RedirectURIs(), []string{"http://localhost:3000/callback"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("RedirectURIs() = %v, want %v", got, want)
	}
	if got, want := client.PostLogoutRedirectURIs(), []string{"http://localhost:3000/logout"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("PostLogoutRedirectURIs() = %v, want %v", got, want)
	}
	if got, want := client.GrantTypes(), []oidc.GrantType{oidc.GrantTypeCode}; !reflect.DeepEqual(got, want) {
		t.Fatalf("GrantTypes() = %v, want %v", got, want)
	}
	if got, want := client.ResponseTypes(), []oidc.ResponseType{oidc.ResponseTypeCode}; !reflect.DeepEqual(got, want) {
		t.Fatalf("ResponseTypes() = %v, want %v", got, want)
	}
	if got, want := client.AuthMethod(), oidc.AuthMethodPost; got != want {
		t.Fatalf("AuthMethod() = %v, want %v", got, want)
	}
	if got, want := client.AccessTokenType(), op.AccessTokenTypeJWT; got != want {
		t.Fatalf("AccessTokenType() = %v, want %v", got, want)
	}
}

func TestStorageGetClientByClientID_UsesTypedAppColumns(t *testing.T) {
	dir := t.TempDir()
	db, err := database.Open("sqlite://" + filepath.Join(dir, "test.db"))
	if err != nil {
		t.Fatalf("database.Open() error = %v", err)
	}
	t.Cleanup(func() {
		_ = db.Close()
	})

	if err := database.Migrate(db); err != nil {
		t.Fatalf("database.Migrate() error = %v", err)
	}

	_, err = db.SQL().ExecContext(
		t.Context(),
		`INSERT INTO apps (id, org_id, name, app_type, client_id, redirect_uris, grant_types, response_types, state, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'active', ?, datetime('now'), datetime('now'))`,
		"app-123",
		"org-123",
		"Debugger App",
		"spa",
		"client-typed",
		`["https://oidcdebugger.com/debug"]`,
		`["authorization_code"]`,
		`["code"]`,
		`{"post_logout_redirect_uris":["https://oidcdebugger.com/logout"]}`,
	)
	if err != nil {
		t.Fatalf("insert app: %v", err)
	}

	client, err := NewStorage(db, nil).GetClientByClientID(t.Context(), "client-typed")
	if err != nil {
		t.Fatalf("GetClientByClientID() error = %v", err)
	}

	if got, want := client.RedirectURIs(), []string{"https://oidcdebugger.com/debug"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("RedirectURIs() = %v, want %v", got, want)
	}
	if got, want := client.PostLogoutRedirectURIs(), []string{"https://oidcdebugger.com/logout"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("PostLogoutRedirectURIs() = %v, want %v", got, want)
	}
	if got, want := client.ApplicationType(), op.ApplicationTypeUserAgent; got != want {
		t.Fatalf("ApplicationType() = %v, want %v", got, want)
	}
}
