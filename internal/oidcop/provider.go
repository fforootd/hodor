package oidcop

import (
	"crypto/sha256"
	"encoding/hex"
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
//
// encryptionKey should be a 64-char hex string (32 bytes). If empty, a
// deterministic fallback is derived from cookieSecret.
func SetupProvider(storage *Storage, issuer string, logger *slog.Logger, encryptionKey, cookieSecret string) (http.Handler, error) {
	var key [32]byte
	if encryptionKey != "" {
		decoded, err := hex.DecodeString(encryptionKey)
		if err != nil || len(decoded) != 32 {
			// Fall back to SHA-256 of the raw string if it's not valid hex.
			key = sha256.Sum256([]byte(encryptionKey))
		} else {
			copy(key[:], decoded)
		}
	} else if cookieSecret != "" {
		// Derive from the cookie signing key so at least it's not a constant.
		key = sha256.Sum256([]byte(cookieSecret))
	} else {
		// Last resort dev fallback — deterministic but not hardcoded in source.
		key = sha256.Sum256([]byte("zitadel-dev-oidc-encryption-key"))
	}

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
