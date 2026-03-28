package ratelimit

import (
	"fmt"
	"log"
	"net"
	"net/http"
	"strconv"
	"time"
)

// ClientIPFunc extracts the real client IP from a request.
// This is injected by the caller to avoid import cycles with the server package.
type ClientIPFunc func(r *http.Request) string

// DefaultClientIP falls back to r.RemoteAddr (stripping port).
func DefaultClientIP(r *http.Request) string {
	host, _, err := net.SplitHostPort(r.RemoteAddr)
	if err != nil {
		return r.RemoteAddr
	}
	return host
}

// Middleware returns HTTP middleware that enforces rate limits.
// It runs at the on_request pipeline stage, before AuthGate.
//
// Pipeline position: OTel → RealIP → SecurityHeaders → AppGate → **RateLimit** → AuthGate
func Middleware(limiter *Limiter, clientIP ClientIPFunc) func(http.Handler) http.Handler {
	if clientIP == nil {
		clientIP = DefaultClientIP
	}

	return func(next http.Handler) http.Handler {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			// Skip exempt paths (healthz, readyz).
			if IsExempt(r.URL.Path) {
				next.ServeHTTP(w, r)
				return
			}

			// Extract real client IP (set by RealIP middleware upstream).
			ip := clientIP(r)

			// Resolve org from header (API clients use X-Zitadel-Org).
			// Browser-based org resolution from Host header is a future Domain Resolver feature.
			orgID := r.Header.Get("X-Zitadel-Org")

			// Evaluate on_request stage rules first.
			env := &RequestEnv{
				Method:  r.Method,
				Path:    r.URL.Path,
				Headers: flattenHeaders(r.Header),
				IP:      ip,
				OrgID:   orgID,
			}

			results, err := limiter.actions.EvaluateHook(r.Context(), "on_request", env)
			if err != nil {
				log.Printf("[actions] evaluation error: %v", err)
				// Fail open for action errors — don't block traffic.
			}

			// Process action results: look for rate_limit action type overrides.
			for _, result := range results {
				if result.Matched && result.ActionType == "rate_limit" && result.Config != nil {
					// Action-based rate limit: use config from the action.
					if key, ok := result.Config["key"].(string); ok {
						limitVal := 60 // default
						if lv, ok := result.Config["limit"].(float64); ok {
							limitVal = int(lv)
						}
						burst := 10
						if bv, ok := result.Config["burst"].(float64); ok {
							burst = int(bv)
						}

						decision, err := limiter.store.Allow(r.Context(), "rule:"+key, limitVal, burst, time.Minute)
						if err != nil {
							log.Printf("[actions] store error: %v", err)
							continue
						}

						if !decision.Allowed {
							writeRateLimited(w, decision)
							return
						}
					}
				}
			}

			// Standard settings-based rate limit check.
			decision, err := limiter.Check(r.Context(), ip, orgID, "")
			if err != nil {
				log.Printf("[ratelimit] check error: %v", err)
				// Fail open.
				next.ServeHTTP(w, r)
				return
			}

			// Write rate limit headers if custom_headers is enabled.
			if decision.Limit > 0 {
				w.Header().Set("X-Ratelimit-Limit", strconv.Itoa(decision.Limit))
				w.Header().Set("X-Ratelimit-Remaining", strconv.Itoa(decision.Remaining))
				w.Header().Set("X-Ratelimit-Reset", strconv.FormatInt(decision.ResetAt.Unix(), 10))
			}

			if !decision.Allowed {
				writeRateLimited(w, decision)
				return
			}

			next.ServeHTTP(w, r)
		})
	}
}

// writeRateLimited writes a 429 Too Many Requests response with appropriate headers.
func writeRateLimited(w http.ResponseWriter, d Decision) {
	w.Header().Set("Retry-After", FormatRetryAfter(d.RetryAfter))
	w.Header().Set("X-Ratelimit-Limit", strconv.Itoa(d.Limit))
	w.Header().Set("X-Ratelimit-Remaining", "0")
	w.Header().Set("X-Ratelimit-Reset", strconv.FormatInt(d.ResetAt.Unix(), 10))
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusTooManyRequests)
	fmt.Fprintf(w,
		`{"error":"rate_limit_exceeded","message":"Too many requests. Retry after %s seconds.","retry_after":%s}`,
		FormatRetryAfter(d.RetryAfter),
		FormatRetryAfter(d.RetryAfter),
	)
}

// flattenHeaders converts http.Header to a flat map (first value per key).
func flattenHeaders(h http.Header) map[string]string {
	flat := make(map[string]string, len(h))
	for k, vs := range h {
		if len(vs) > 0 {
			flat[k] = vs[0]
		}
	}
	return flat
}
