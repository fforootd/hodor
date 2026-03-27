package oidcop

import (
	"crypto/sha256"
	"log/slog"
	"net/http"

	"golang.org/x/text/language"

	"github.com/zitadel/oidc/v3/pkg/op"
)

// SetupProvider creates an OpenID Provider backed by the OIDC storage.
// It returns an http.Handler that serves the OIDC endpoints:
//   - GET  /.well-known/openid-configuration
//   - GET  /keys (JWKS)
//   - GET  /authorize
//   - POST /oauth/token
//   - GET  /userinfo
//   - POST /revoke
//   - GET  /end_session
func SetupProvider(storage *Storage, issuer string, logger *slog.Logger) (http.Handler, error) {
	// The OP needs a 32-byte encryption key for token encryption.
	// In production, manage this securely and persist it.
	key := sha256.Sum256([]byte("zitadel-poc-oidc-encryption-key"))

	config := &op.Config{
		CryptoKey: key,

		// Default logout redirect.
		DefaultLogoutRedirectURI: "/login",

		// Enable PKCE with S256.
		CodeMethodS256: true,

		// Allow client authentication via POST body (not just HTTP Basic).
		AuthMethodPost: true,

		// Enable refresh_token grant.
		GrantTypeRefreshToken: true,

		// Supported UI locales.
		SupportedUILocales: []language.Tag{language.English},
	}

	opts := []op.Option{
		// Allow http:// issuer for development.
		op.WithAllowInsecure(),
	}
	if logger != nil {
		opts = append(opts, op.WithLogger(logger.WithGroup("oidc")))
	}

	provider, err := op.NewOpenIDProvider(issuer, config, storage, opts...)
	if err != nil {
		return nil, err
	}

	return provider, nil
}
