// Package httputil provides shared HTTP response helpers used across
// all Zitadel API, login, analytics, and FGA packages.
package httputil

import (
	"encoding/json"
	"fmt"
	"net/http"
)

// WriteJSON serialises v as JSON and writes it with the given HTTP status code.
func WriteJSON(w http.ResponseWriter, status int, v any) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(v)
}

// WriteError writes a JSON error response: {"error": msg, "code": status}.
func WriteError(w http.ResponseWriter, status int, msg string) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_, _ = fmt.Fprintf(w, `{"error":%q,"code":%d}`, msg, status)
}
