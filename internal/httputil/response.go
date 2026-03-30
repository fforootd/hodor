// Package httputil provides shared HTTP response helpers used across
// all Zitadel API, login, analytics, and FGA packages.
package httputil

import (
	"encoding/json"
	"fmt"
	"net/http"
)

const (
	cacheControlNoStore   = "no-store"
	cacheControlImmutable = "public, max-age=31536000, immutable"
)

// SetNoStore marks a dynamic response as non-cacheable unless the handler already chose a policy.
func SetNoStore(w http.ResponseWriter) {
	if w.Header().Get("Cache-Control") == "" {
		w.Header().Set("Cache-Control", cacheControlNoStore)
	}
}

// SetImmutableAssetCache marks an asset response as long-lived and immutable.
func SetImmutableAssetCache(w http.ResponseWriter) {
	w.Header().Set("Cache-Control", cacheControlImmutable)
}

// WriteJSON serialises v as JSON and writes it with the given HTTP status code.
func WriteJSON(w http.ResponseWriter, status int, v any) {
	SetNoStore(w)
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}

// WriteError writes a JSON error response: {"error": msg, "code": status}.
func WriteError(w http.ResponseWriter, status int, msg string) {
	SetNoStore(w)
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_, _ = fmt.Fprintf(w, `{"error":%q,"code":%d}`, msg, status)
}
