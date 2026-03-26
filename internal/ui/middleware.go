package ui

import (
	"context"
	"net/http"
)

type contextKey string

const ctxKeyIdentity contextKey = "identity"

// requireAdmin wraps a handler, checking for a valid session with the "admin" capability.
func (u *UI) requireAdmin(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		ident, ok := u.getSession(r)
		if !ok {
			http.Redirect(w, r, "/login?redirect_to="+r.URL.Path, http.StatusSeeOther)
			return
		}

		// Check for admin capability.
		hasAdmin := false
		for _, cap := range ident.Capabilities {
			if cap == "admin" {
				hasAdmin = true
				break
			}
		}
		if !hasAdmin {
			http.Error(w, "Forbidden — admin capability required", http.StatusForbidden)
			return
		}

		// Set identity in context.
		ctx := context.WithValue(r.Context(), ctxKeyIdentity, ident)
		next(w, r.WithContext(ctx))
	}
}
