package login

import (
	"crypto/tls"
	"net/http/httptest"
	"testing"

	providers "github.com/zitadel/zitadel/internal/provider"
)

func TestSanitizeContinueTo_AllowsSameOriginAbsoluteAndRelative(t *testing.T) {
	t.Parallel()

	req := httptest.NewRequest("GET", "https://login.example.com/start", nil)
	req.Header.Set("X-Forwarded-Proto", "https")

	if got := sanitizeContinueTo(req, "/console"); got != "/console" {
		t.Fatalf("relative sanitizeContinueTo() = %q, want /console", got)
	}
	if got := sanitizeContinueTo(req, "https://login.example.com/console?tab=1#section"); got != "/console?tab=1#section" {
		t.Fatalf("absolute sanitizeContinueTo() = %q", got)
	}
	if got := sanitizeContinueTo(req, "https://evil.example.com/phish"); got != "" {
		t.Fatalf("cross-origin sanitizeContinueTo() = %q, want empty", got)
	}
}

func TestRequestOriginURL_UsesTLSAndForwardedProto(t *testing.T) {
	t.Parallel()

	req := httptest.NewRequest("GET", "http://login.example.com/start", nil)
	req.Header.Set("X-Forwarded-Proto", "https")
	req.TLS = &tls.ConnectionState{}

	got := requestOriginURL(req)
	if got == nil || got.String() != "https://login.example.com" {
		t.Fatalf("requestOriginURL() = %#v", got)
	}
}

func TestProviderAllowedForConfig_HonorsAllowlistAndSchemaType(t *testing.T) {
	t.Parallel()

	prov := providers.Provider{
		ID:     "google",
		Target: providers.Target{SchemaType: "human_user"},
	}
	cfg := &SchemaAuthConfig{
		SSOProviderMode:        "allowlist",
		SSOProviderIDs:         []string{"google"},
		RegistrationSchemaType: "human_user",
	}
	if !providerAllowedForConfig(prov, cfg) {
		t.Fatal("providerAllowedForConfig() = false, want true")
	}

	cfg.SSOProviderIDs = []string{"github"}
	if providerAllowedForConfig(prov, cfg) {
		t.Fatal("providerAllowedForConfig() = true for disallowed provider")
	}
}

func TestScopeString_DefaultsByProtocol(t *testing.T) {
	t.Parallel()

	if got := scopeString(nil, "oauth2"); got != "user:email read:user" {
		t.Fatalf("scopeString(oauth2) = %q", got)
	}
	if got := scopeString(nil, "oidc"); got != "openid email profile" {
		t.Fatalf("scopeString(oidc) = %q", got)
	}
	if got := scopeString([]any{"openid", "email"}, "oidc"); got != "openid email" {
		t.Fatalf("scopeString(list) = %q", got)
	}
}
