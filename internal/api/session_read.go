package api

import (
	"context"
	"fmt"
	"net/http"

	"github.com/zitadel/zitadel/internal/httputil"
)

func (a *API) getSession(w http.ResponseWriter, r *http.Request) {
	sessionID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	sess, err := a.loadSession(r.Context(), sessionID)
	if err != nil {
		httputil.WriteError(w, http.StatusNotFound, "session not found")
		return
	}

	httputil.WriteJSON(w, http.StatusOK, sess)
}

func (a *API) listSessions(w http.ResponseWriter, r *http.Request) {
	userID, _ := r.URL.Query().Get("user_id"), ""
	limit := 50

	records, err := a.sessionStore.List(r.Context(), userID, limit)
	if err != nil {
		httputil.WriteError(w, http.StatusInternalServerError, "query failed")
		return
	}

	sessions := make([]SessionResponse, 0, len(records))
	for _, record := range records {
		sessions = append(sessions, sessionResponseFromRecord(record))
	}

	httputil.WriteJSON(w, http.StatusOK, ListResponse{Items: sessions})
}

func (a *API) revokeSession(w http.ResponseWriter, r *http.Request) {
	sessionID, err := parseID(r, "id")
	if err != nil {
		httputil.WriteError(w, http.StatusBadRequest, "invalid id")
		return
	}

	if err := a.RevokeSessionInternal(r.Context(), sessionID); err != nil {
		httputil.WriteError(w, http.StatusNotFound, err.Error())
		return
	}

	w.WriteHeader(http.StatusNoContent)
}

// RevokeSessionInternal revokes a session programmatically (used by UI logout).
func (a *API) RevokeSessionInternal(ctx context.Context, sessionID string) error {
	if err := a.sessionStore.Revoke(ctx, sessionID, "api_revoke"); err != nil {
		return fmt.Errorf("revoke: %w", err)
	}

	a.bus.Signal()
	return nil
}

func (a *API) loadSession(ctx context.Context, sessionID string) (SessionResponse, error) {
	record, err := a.sessionStore.Get(ctx, sessionID)
	if err != nil {
		return SessionResponse{}, err
	}
	return sessionResponseFromRecord(record), nil
}
