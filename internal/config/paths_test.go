package config

import "testing"

func TestResolvePaths_Default(t *testing.T) {
	s := &ServerConfig{}
	p := s.ResolvePaths()

	if p.Base != "" {
		t.Errorf("Base = %q, want empty", p.Base)
	}
	if p.Console != "/console" {
		t.Errorf("Console = %q, want /console", p.Console)
	}
	if p.Login != "/login" {
		t.Errorf("Login = %q, want /login", p.Login)
	}
	if p.API != "" {
		t.Errorf("API = %q, want empty", p.API)
	}
	if p.OIDC != "" {
		t.Errorf("OIDC = %q, want empty", p.OIDC)
	}
}

func TestResolvePaths_WithBasePath(t *testing.T) {
	s := &ServerConfig{BasePath: "/auth"}
	p := s.ResolvePaths()

	if p.Base != "/auth" {
		t.Errorf("Base = %q, want /auth", p.Base)
	}
	if p.Console != "/auth/console" {
		t.Errorf("Console = %q, want /auth/console", p.Console)
	}
	if p.Login != "/auth/login" {
		t.Errorf("Login = %q, want /auth/login", p.Login)
	}
	if p.Account != "/auth/account" {
		t.Errorf("Account = %q, want /auth/account", p.Account)
	}
	if p.API != "/auth" {
		t.Errorf("API = %q, want /auth", p.API)
	}
	// OIDC defaults to root when base_path is set
	if p.OIDC != "" {
		t.Errorf("OIDC = %q, want empty (root)", p.OIDC)
	}
}

func TestResolvePaths_OIDCOverride(t *testing.T) {
	s := &ServerConfig{
		BasePath: "/auth",
		PathOverrides: PathOverrideConfig{
			OIDC: "/auth", // explicitly put OIDC under base path
		},
	}
	p := s.ResolvePaths()

	if p.OIDC != "/auth" {
		t.Errorf("OIDC = %q, want /auth", p.OIDC)
	}
}

func TestResolvePaths_AppOverrideToRoot(t *testing.T) {
	s := &ServerConfig{
		BasePath: "/auth",
		PathOverrides: PathOverrideConfig{
			Console: "/", // keep console at root
		},
	}
	p := s.ResolvePaths()

	if p.Console != "/console" {
		t.Errorf("Console = %q, want /console (at root)", p.Console)
	}
	if p.Login != "/auth/login" {
		t.Errorf("Login = %q, want /auth/login", p.Login)
	}
}

func TestResolvePaths_TrailingSlash(t *testing.T) {
	s := &ServerConfig{BasePath: "/auth/"}
	p := s.ResolvePaths()

	if p.Base != "/auth" {
		t.Errorf("Base = %q, want /auth (no trailing slash)", p.Base)
	}
}

func TestAPIRoute(t *testing.T) {
	tests := []struct {
		base   string
		method string
		path   string
		want   string
	}{
		{"", "POST", "/v1/entities", "POST /v1/entities"},
		{"/auth", "POST", "/v1/entities", "POST /auth/v1/entities"},
		{"/auth", "GET", "/v1/schemas", "GET /auth/v1/schemas"},
	}

	for _, tt := range tests {
		s := &ServerConfig{BasePath: tt.base}
		p := s.ResolvePaths()
		got := p.APIRoute(tt.method, tt.path)
		if got != tt.want {
			t.Errorf("APIRoute(%q, %q) with base=%q = %q, want %q",
				tt.method, tt.path, tt.base, got, tt.want)
		}
	}
}

func TestIssuer(t *testing.T) {
	tests := []struct {
		domain string
		port   int
		oidc   string
		want   string
	}{
		{"localhost", 8080, "", "http://localhost:8080"},
		{"example.com", 443, "", "https://example.com"},
		{"example.com", 443, "/auth", "https://example.com/auth"},
	}

	for _, tt := range tests {
		s := &ServerConfig{
			ExternalDomain: tt.domain,
			BasePath:       tt.oidc,
		}
		p := s.ResolvePaths()
		// Override OIDC to match the test expectation
		p.OIDC = tt.oidc
		got := p.Issuer(tt.domain, tt.port)
		if got != tt.want {
			t.Errorf("Issuer(%q, %d) = %q, want %q", tt.domain, tt.port, got, tt.want)
		}
	}
}
