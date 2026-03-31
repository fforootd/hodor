package login

import (
	"fmt"
	"net/http"
	"net/url"
	"strings"

	providers "github.com/zitadel/zitadel/internal/provider"
)

func sanitizeContinueTo(r *http.Request, candidate string) string {
	candidate = strings.TrimSpace(candidate)
	if candidate == "" {
		return ""
	}
	if strings.HasPrefix(candidate, "//") {
		return ""
	}
	if strings.HasPrefix(candidate, "/") {
		return candidate
	}

	parsed, err := url.Parse(candidate)
	if err != nil || !parsed.IsAbs() {
		return ""
	}

	requestOrigin := requestOriginURL(r)
	if requestOrigin == nil {
		return ""
	}
	if !sameURLOrigin(requestOrigin, parsed) {
		return ""
	}

	result := parsed.RequestURI()
	if parsed.Fragment != "" {
		result += "#" + parsed.Fragment
	}
	return result
}

func requestOriginURL(r *http.Request) *url.URL {
	scheme := "http"
	if r.TLS != nil {
		scheme = "https"
	}
	if forwarded := strings.TrimSpace(r.Header.Get("X-Forwarded-Proto")); forwarded != "" {
		scheme = strings.Split(forwarded, ",")[0]
	}
	if r.Host == "" {
		return nil
	}
	return &url.URL{Scheme: scheme, Host: r.Host}
}

func sameURLOrigin(left, right *url.URL) bool {
	if left == nil || right == nil {
		return false
	}
	return strings.EqualFold(left.Scheme, right.Scheme) && strings.EqualFold(left.Host, right.Host)
}

func claimBool(value any) bool {
	switch typed := value.(type) {
	case bool:
		return typed
	case string:
		return strings.EqualFold(typed, "true")
	case float64:
		return typed != 0
	default:
		return false
	}
}

func providerAllowedForConfig(prov providers.Provider, cfg *SchemaAuthConfig) bool {
	if cfg == nil {
		return true
	}
	if cfg.SSOProviderMode == "allowlist" && len(cfg.SSOProviderIDs) > 0 {
		allowed := false
		for _, providerID := range cfg.SSOProviderIDs {
			if providerID == prov.ID {
				allowed = true
				break
			}
		}
		if !allowed {
			return false
		}
	}
	targetSchemaType := cfg.RegistrationSchemaType
	if targetSchemaType == "" {
		targetSchemaType = "human_user"
	}
	return prov.Target.SchemaType == "" || prov.Target.SchemaType == targetSchemaType || prov.Target.SchemaID != ""
}

func stringifyClaim(value any) string {
	switch typed := value.(type) {
	case string:
		return typed
	case float64:
		return fmt.Sprintf("%.0f", typed)
	default:
		return ""
	}
}

func scopeString(value any, protocol string) string {
	switch typed := value.(type) {
	case string:
		if strings.TrimSpace(typed) != "" {
			return typed
		}
	case []any:
		parts := make([]string, 0, len(typed))
		for _, item := range typed {
			if str, ok := item.(string); ok && strings.TrimSpace(str) != "" {
				parts = append(parts, str)
			}
		}
		if len(parts) > 0 {
			return strings.Join(parts, " ")
		}
	}
	if protocol == "oauth2" {
		return "user:email read:user"
	}
	return "openid email profile"
}
