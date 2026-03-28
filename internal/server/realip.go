// Package server provides HTTP middleware for Zitadel.
// realip.go resolves the real client IP from proxy headers.
package server

import (
	"context"
	"net"
	"net/http"
	"strings"
)

// realIPCtxKey is an unexported type to prevent context key collisions.
type realIPCtxKey int

const (
	realIPKey      realIPCtxKey = iota
	realProtoKey
	realHostKey
	requestIDKey
	proxyMetaKey
)

// ProxyMeta holds enrichment data from CDN/WAF proxy headers (e.g., JP3A tags).
type ProxyMeta struct {
	ClientID          string
	SessionID         string
	DeviceFingerprint string
	Raw               map[string]string // all custom headers captured
}

// RealIPConfig holds the parsed trusted proxy CIDRs and header mode.
type RealIPConfig struct {
	TrustedCIDRs []*net.IPNet
	Mode         string // "standard" | "cloudflare" | "custom"
	CustomHeader string // e.g., "CF-Connecting-IP"
}

// ParseTrustedProxies parses CIDR strings into net.IPNet slices.
func ParseTrustedProxies(cidrs []string) ([]*net.IPNet, error) {
	var nets []*net.IPNet
	for _, cidr := range cidrs {
		_, ipNet, err := net.ParseCIDR(strings.TrimSpace(cidr))
		if err != nil {
			return nil, err
		}
		nets = append(nets, ipNet)
	}
	return nets, nil
}

// RealIP returns middleware that resolves the true client IP from proxy headers.
// It ONLY trusts headers when the direct connection comes from a trusted proxy CIDR.
//
// Header resolution order by mode:
//   - "standard":    X-Forwarded-For → X-Real-IP → r.RemoteAddr
//   - "cloudflare":  CF-Connecting-IP → X-Forwarded-For → r.RemoteAddr
//   - "custom":      CustomHeader → X-Forwarded-For → r.RemoteAddr
//
// Additional headers always parsed when present:
//   - X-Forwarded-Proto (http/https)
//   - X-Forwarded-Host
//   - X-Request-ID (passthrough for correlation)
//   - True-Client-IP (Akamai/Cloudflare)
//   - JP3A-Client-ID, JP3A-Session-ID, JP3A-Device-Fingerprint
func RealIP(cfg *RealIPConfig) func(http.Handler) http.Handler {
	if cfg == nil || len(cfg.TrustedCIDRs) == 0 {
		// No trusted proxies configured: pass through unchanged.
		return func(next http.Handler) http.Handler { return next }
	}

	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// Parse the direct connection IP.
			remoteIP := extractIP(r.RemoteAddr)

			// Only trust proxy headers if the direct connection is from a trusted proxy.
			if isTrusted(remoteIP, cfg.TrustedCIDRs) {
				clientIP := resolveClientIP(r, cfg)
				if clientIP != "" {
					r.RemoteAddr = clientIP
				}

				// Inject into context.
				ctx := r.Context()
				ctx = context.WithValue(ctx, realIPKey, clientIP)

				if proto := r.Header.Get("X-Forwarded-Proto"); proto != "" {
					ctx = context.WithValue(ctx, realProtoKey, proto)
				}
				if host := r.Header.Get("X-Forwarded-Host"); host != "" {
					ctx = context.WithValue(ctx, realHostKey, host)
				}
				if reqID := r.Header.Get("X-Request-ID"); reqID != "" {
					ctx = context.WithValue(ctx, requestIDKey, reqID)
				}

				// Parse JP3A/vendor enrichment tags.
				meta := &ProxyMeta{Raw: make(map[string]string)}
				if v := r.Header.Get("Jp3a-Client-Id"); v != "" {
					meta.ClientID = v
					meta.Raw["JP3A-Client-ID"] = v
				}
				if v := r.Header.Get("Jp3a-Session-Id"); v != "" {
					meta.SessionID = v
					meta.Raw["JP3A-Session-ID"] = v
				}
				if v := r.Header.Get("Jp3a-Device-Fingerprint"); v != "" {
					meta.DeviceFingerprint = v
					meta.Raw["JP3A-Device-Fingerprint"] = v
				}
				ctx = context.WithValue(ctx, proxyMetaKey, meta)

				r = r.WithContext(ctx)
			}

			next.ServeHTTP(w, r)
		})
	}
}

// FromContext returns the real client IP from request context.
// Falls back to r.RemoteAddr if not set.
func FromContext(r *http.Request) string {
	if ip, ok := r.Context().Value(realIPKey).(string); ok && ip != "" {
		return ip
	}
	return extractIP(r.RemoteAddr)
}

// ProtoFromContext returns the original protocol (http/https) from proxy headers.
func ProtoFromContext(r *http.Request) string {
	if proto, ok := r.Context().Value(realProtoKey).(string); ok {
		return proto
	}
	return ""
}

// RequestIDFromContext returns the X-Request-ID from proxy headers.
func RequestIDFromContext(r *http.Request) string {
	if id, ok := r.Context().Value(requestIDKey).(string); ok {
		return id
	}
	return ""
}

// ProxyMetaFromContext returns vendor enrichment data (JP3A tags, etc.)
func ProxyMetaFromContext(r *http.Request) *ProxyMeta {
	if meta, ok := r.Context().Value(proxyMetaKey).(*ProxyMeta); ok {
		return meta
	}
	return nil
}

// resolveClientIP extracts the real client IP from proxy headers based on mode.
func resolveClientIP(r *http.Request, cfg *RealIPConfig) string {
	switch cfg.Mode {
	case "cloudflare":
		// CF-Connecting-IP is set by Cloudflare edge, most reliable.
		if ip := r.Header.Get("Cf-Connecting-Ip"); ip != "" {
			return strings.TrimSpace(ip)
		}
		return rightmostUntrustedXFF(r, cfg.TrustedCIDRs)

	case "custom":
		if cfg.CustomHeader != "" {
			if ip := r.Header.Get(cfg.CustomHeader); ip != "" {
				return strings.TrimSpace(ip)
			}
		}
		return rightmostUntrustedXFF(r, cfg.TrustedCIDRs)

	default: // "standard"
		// Try True-Client-IP first (some CDNs set this).
		if ip := r.Header.Get("True-Client-Ip"); ip != "" {
			return strings.TrimSpace(ip)
		}
		// Then X-Forwarded-For (rightmost untrusted).
		if ip := rightmostUntrustedXFF(r, cfg.TrustedCIDRs); ip != "" {
			return ip
		}
		// Then X-Real-IP (single-proxy setups like nginx).
		if ip := r.Header.Get("X-Real-IP"); ip != "" {
			return strings.TrimSpace(ip)
		}
		return extractIP(r.RemoteAddr)
	}
}

// rightmostUntrustedXFF returns the rightmost IP in X-Forwarded-For that is NOT
// in the trusted proxy list. This is the correct way to extract the client IP:
// the leftmost entry is easily spoofable, but the rightmost untrusted entry is
// set by the nearest trusted proxy.
func rightmostUntrustedXFF(r *http.Request, trusted []*net.IPNet) string {
	xff := r.Header.Get("X-Forwarded-For")
	if xff == "" {
		return ""
	}

	parts := strings.Split(xff, ",")
	for i := len(parts) - 1; i >= 0; i-- {
		ip := strings.TrimSpace(parts[i])
		if ip == "" {
			continue
		}
		parsed := net.ParseIP(ip)
		if parsed == nil {
			continue
		}
		if !isTrusted(ip, trusted) {
			return ip
		}
	}

	// All IPs in the chain are trusted (shouldn't happen, but fallback).
	return ""
}

// isTrusted checks if an IP string is within any of the trusted CIDR ranges.
func isTrusted(ipStr string, cidrs []*net.IPNet) bool {
	ip := net.ParseIP(ipStr)
	if ip == nil {
		return false
	}
	for _, cidr := range cidrs {
		if cidr.Contains(ip) {
			return true
		}
	}
	return false
}

// extractIP strips port from host:port address strings.
func extractIP(addr string) string {
	if host, _, err := net.SplitHostPort(addr); err == nil {
		return host
	}
	return addr
}
