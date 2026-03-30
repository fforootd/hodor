package login

import (
	"context"
	"database/sql"
	"encoding/json"
	"errors"
	"net/http"
	"strings"
)

type LoginErrorKind string

const (
	LoginErrorKindStartup       LoginErrorKind = "startup"
	LoginErrorKindTransport     LoginErrorKind = "transport"
	LoginErrorKindConfiguration LoginErrorKind = "configuration"
	LoginErrorKindFlow          LoginErrorKind = "flow"
	LoginErrorKindInternal      LoginErrorKind = "internal"
)

type LoginAPIError struct {
	Code      string         `json:"code"`
	Message   string         `json:"message"`
	Retryable bool           `json:"retryable"`
	Kind      LoginErrorKind `json:"kind"`
}

type loginAPIErrorEnvelope struct {
	Error LoginAPIError `json:"error"`
}

func writeLoginError(w http.ResponseWriter, status int, apiErr LoginAPIError) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(status)
	_ = json.NewEncoder(w).Encode(loginAPIErrorEnvelope{Error: apiErr})
}

func newLoginError(code, message string, retryable bool, kind LoginErrorKind) LoginAPIError {
	return LoginAPIError{
		Code:      code,
		Message:   message,
		Retryable: retryable,
		Kind:      kind,
	}
}

func loginBadRequest(message string) LoginAPIError {
	return newLoginError("internal_error", message, false, LoginErrorKindInternal)
}

func loginFlowNotFound(message string) LoginAPIError {
	return newLoginError("flow_not_found", message, false, LoginErrorKindFlow)
}

func loginInternalError(message string) LoginAPIError {
	return newLoginError("internal_error", message, false, LoginErrorKindInternal)
}

func (h *Handler) classifyInitError(ctx context.Context, err error) (int, LoginAPIError) {
	if err == nil {
		return http.StatusInternalServerError, loginInternalError("Login could not be initialized.")
	}

	if errors.Is(err, sql.ErrNoRows) {
		return http.StatusNotFound, loginFlowNotFound("Requested login flow was not found.")
	}

	if pingErr := h.db.SQL().PingContext(ctx); pingErr != nil {
		return http.StatusServiceUnavailable, newLoginError(
			"service_unavailable",
			"Login is temporarily unavailable. Try again in a moment.",
			true,
			LoginErrorKindTransport,
		)
	}

	msg := strings.ToLower(err.Error())
	switch {
	case strings.Contains(msg, "no such table"),
		strings.Contains(msg, "schema version"),
		strings.Contains(msg, "database is locked"),
		strings.Contains(msg, "database is closed"),
		strings.Contains(msg, "starting"):
		return http.StatusServiceUnavailable, newLoginError(
			"service_starting",
			"Zitadel is still starting. Try again in a moment.",
			true,
			LoginErrorKindStartup,
		)
	case strings.Contains(msg, "no login flows found"),
		strings.Contains(msg, "no matching login flow"),
		strings.Contains(msg, "no default schema configured"),
		strings.Contains(msg, "no user schema configured"):
		return http.StatusInternalServerError, newLoginError(
			"flow_config_invalid",
			"Login is not configured correctly.",
			false,
			LoginErrorKindConfiguration,
		)
	default:
		return http.StatusInternalServerError, loginInternalError("Login could not be initialized.")
	}
}
