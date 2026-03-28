// Package config — PathConfig resolves all route prefixes at startup.
// It implements the two-tier model: global base_path + per-app overrides.
package config

import (
	"fmt"
	"strings"
)

// PathConfig holds resolved path prefixes for all Zitadel apps.
// Computed once at startup from ServerConfig.
type PathConfig struct {
	Base    string // Global prefix, e.g. "/auth" or ""
	API     string // Prefix for /v1/* routes
	Console string // e.g. "/auth/console" or "/console"
	Login   string // e.g. "/auth/login" or "/login"
	Assets  string // e.g. "/auth/assets" or "/assets"
	Health  string // Prefix for /healthz, /debug, /ready
	OIDC    string // Mount point for OIDC handler ("/" or "/auth")
	SAML    string // Mount point for SAML handler
	Account string // e.g. "/auth/account" or "/account"
	Static  string // e.g. "/auth/static" or "/static"
	Admin   string // e.g. "/auth/admin" or "/admin"
}

// ResolvePaths builds a PathConfig from the current ServerConfig.
// When BasePath is empty, all paths resolve to their defaults (root-level deployment).
// When BasePath is set (e.g., "/auth"), all apps inherit it unless overridden.
func (s *ServerConfig) ResolvePaths() *PathConfig {
	base := cleanPath(s.BasePath)

	p := &PathConfig{
		Base:    base,
		API:     resolve(base, s.PathOverrides.API),
		Console: joinPath(resolve(base, s.PathOverrides.Console), "/console"),
		Login:   joinPath(resolve(base, s.PathOverrides.Login), "/login"),
		Assets:  joinPath(resolve(base, s.PathOverrides.Assets), "/assets"),
		Health:  base,
		Account: joinPath(resolve(base, s.PathOverrides.Login), "/account"),
		Static:  joinPath(resolve(base, s.PathOverrides.Assets), "/static"),
		Admin:   joinPath(resolve(base, s.PathOverrides.Console), "/admin"),
	}

	// OIDC & SAML default to root when base_path is set (per OIDC Discovery spec).
	if base != "" {
		if s.PathOverrides.OIDC == "" {
			p.OIDC = "" // stay at root
		} else {
			p.OIDC = resolve(base, s.PathOverrides.OIDC)
		}
		if s.PathOverrides.SAML == "" {
			p.SAML = "" // stay at root
		} else {
			p.SAML = resolve(base, s.PathOverrides.SAML)
		}
	} else {
		p.OIDC = resolve(base, s.PathOverrides.OIDC)
		p.SAML = resolve(base, s.PathOverrides.SAML)
	}

	return p
}

// APIRoute returns a full HTTP route pattern with method and prefix.
// Example: APIRoute("POST", "/v1/entities") → "POST /auth/v1/entities"
func (p *PathConfig) APIRoute(method, path string) string {
	return method + " " + p.API + path
}

// Issuer returns the OIDC issuer URL based on the OIDC path configuration.
func (p *PathConfig) Issuer(externalDomain string, port int) string {
	scheme := "https"
	if externalDomain == "localhost" || externalDomain == "127.0.0.1" || externalDomain == "" {
		scheme = "http"
	}

	host := externalDomain
	if (scheme == "https" && port != 443) || (scheme == "http" && port != 80) {
		host = fmt.Sprintf("%s:%d", externalDomain, port)
	}

	// The issuer includes the OIDC base path
	return scheme + "://" + host + p.OIDC
}

// resolve determines the effective path for an app:
// - If override is "/", the app stays at root (returns "").
// - If override is empty, the app inherits the global base path.
// - Otherwise, the override is used as-is.
func resolve(base, override string) string {
	if override == "/" {
		return ""
	}
	if override != "" {
		return cleanPath(override)
	}
	return base
}

// joinPath combines a resolved base prefix with a relative path.
// Handles double-slash and empty-path edge cases.
func joinPath(base, path string) string {
	if base == "" {
		return path
	}
	return strings.TrimRight(base, "/") + "/" + strings.TrimLeft(path, "/")
}

// cleanPath normalizes a path: removes trailing slash, ensures leading slash.
func cleanPath(p string) string {
	p = strings.TrimSpace(p)
	if p == "" || p == "/" {
		return ""
	}
	if !strings.HasPrefix(p, "/") {
		p = "/" + p
	}
	return strings.TrimRight(p, "/")
}
