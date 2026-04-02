package login

import (
	"errors"
	"fmt"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

func (h *Handler) flowSubmitIdentifier(w http.ResponseWriter, r *http.Request, flow *Flow, identifier string) {
	identifier = strings.TrimSpace(identifier)
	if identifier == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "identifier_required", Message: "Email or username is required"})
		h.renderFlowStep(w, r, flow)
		return
	}

	orgID := httputil.ResolveOrgID(r, "")

	resolved, err := uniqueness.ResolveIdentifier(r.Context(), h.db.SQL(), identifier, orgID)
	if errors.Is(err, uniqueness.ErrIdentityNotFound) {
		flow.Identifier = identifier
		flow.Errors = append(flow.Errors, FlowError{Code: "not_found", Message: "Account not found"})
		h.renderFlowStep(w, r, flow)
		return
	}
	if err != nil {
		logging.Printf("[flow] %s identifier resolve error: %v", flow.ID, err)
		writeLoginError(w, http.StatusInternalServerError, loginInternalError("Login could not continue. Please try again."))
		return
	}

	flow.IduserID = resolved.UserID
	flow.Identifier = identifier
	if flow.TrustedUserID != "" && flow.TrustedUserID == resolved.UserID {
		flow.RevealMode = IdentityRevealModeKnownUser
		flow.DisplayName = resolved.DisplayName
	} else {
		flow.RevealMode = IdentityRevealModeAnonymous
		flow.DisplayName = ""
	}
	flow.transitionToStep(StepAuthSelect)
	flow.Errors = nil

	logging.Printf("[flow] %s identifier resolved: %s (identity=%s)", flow.ID, identifier, resolved.UserID)
	h.renderFlowStep(w, r, flow)
}

func (h *Handler) flowSubmitUseSession(w http.ResponseWriter, r *http.Request, flow *Flow) {
	if flow.TrustedUserID == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "session_unavailable", Message: "Your existing session is no longer available. Please sign in again."})
		flow.transitionToStep(StepIdentifier)
		h.renderFlowStep(w, r, flow)
		return
	}
	currentTrustedUserID, ok := h.resolveTrustedUserIDFromRequest(r)
	if !ok || currentTrustedUserID != flow.TrustedUserID {
		flow.Errors = append(flow.Errors, FlowError{Code: "session_unavailable", Message: "Your existing session is no longer available. Please sign in again."})
		flow.transitionToStep(StepIdentifier)
		h.renderFlowStep(w, r, flow)
		return
	}
	if flow.AuthRequestID == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "oidc_request_missing", Message: "OIDC login could not continue. Please sign in again."})
		flow.transitionToStep(StepIdentifier)
		h.renderFlowStep(w, r, flow)
		return
	}
	if err := h.completeOIDCAuthRequest(r.Context(), flow.AuthRequestID, flow.TrustedUserID); err != nil {
		flow.Errors = append(flow.Errors, FlowError{Code: "oidc_complete_failed", Message: "OIDC login could not continue. Please sign in again."})
		flow.transitionToStep(StepIdentifier)
		h.renderFlowStep(w, r, flow)
		return
	}

	flow.IduserID = flow.TrustedUserID
	flow.AuthMethod = "session_reuse"
	flow.transitionToStep(StepComplete)
	h.completeFlowWithTrustedSession(w, flow)
	h.flows.Delete(flow.ID)
}

func (h *Handler) flowSubmitPassword(w http.ResponseWriter, r *http.Request, flow *Flow, password string) {
	if password == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "password_required", Message: "Password is required"})
		h.renderFlowStep(w, r, flow)
		return
	}

	instanceID := httputil.InstanceIDFromContext(r.Context())
	var credData string
	err := h.db.SQL().QueryRowContext(r.Context(),
		`SELECT data FROM credentials WHERE user_id = ? AND type = 'password' AND instance_id = ?`,
		flow.IduserID, instanceID,
	).Scan(&credData)
	if err != nil {
		logging.Printf("[flow] %s password lookup failed for identity=%s: %v", flow.ID, flow.IduserID, err)
		flow.Errors = append(flow.Errors, FlowError{Code: "internal", Message: "Something went wrong. Please try again."})
		h.renderFlowStep(w, r, flow)
		return
	}

	hash := auth.DecodeCredentialJSON(credData)
	if hash == "" {
		logging.Printf("[flow] %s invalid credential data for identity=%s", flow.ID, flow.IduserID)
		flow.Errors = append(flow.Errors, FlowError{Code: "internal", Message: "Something went wrong. Please try again."})
		h.renderFlowStep(w, r, flow)
		return
	}

	ok, _, err := h.passwords.Verify(hash, password)
	if err != nil || !ok {
		h.api.EmitAuthEvent(r.Context(), "auth.login_failed", flow.IduserID, map[string]any{
			"reason":  "invalid_password",
			"flow_id": flow.ID,
		})
		flow.Errors = append(flow.Errors, FlowError{Code: "invalid_password", Message: "Invalid password. Please try again."})
		h.renderFlowStep(w, r, flow)
		return
	}

	flow.Verified = true
	flow.AuthMethod = "password"
	flow.Errors = nil

	if flow.SchemaConfig.Login.MFARequired {
		flow.transitionToStep(StepMFA)
		h.renderFlowStep(w, r, flow)
		return
	}

	h.flowComplete(w, r, flow)
}

func (h *Handler) flowSubmitMagicLink(w http.ResponseWriter, r *http.Request, flow *Flow) {
	if flow.Identifier == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "no_identifier", Message: "No identifier set"})
		h.renderFlowStep(w, r, flow)
		return
	}

	if _, _, err := h.queueMagicLink(r.Context(), flow.Identifier, "login"); err != nil {
		logging.Printf("[flow] %s failed to queue magic link to %s: %v", flow.ID, flow.Identifier, err)
		flow.Errors = append(flow.Errors, FlowError{Code: "magic_link_failed", Message: "We couldn't send a sign-in link right now. Please try again."})
		h.renderFlowStep(w, r, flow)
		return
	}

	logging.Printf("[flow] %s queued magic link to %s", flow.ID, flow.Identifier)
	flow.transitionToStep(StepMagicLink)
	flow.Errors = nil
	flow.Messages = append(flow.Messages, FlowMessage{Type: "success", Text: "Sign-in link sent!"})
	h.renderFlowStep(w, r, flow)
}

func (h *Handler) flowSubmitSSO(w http.ResponseWriter, r *http.Request, flow *Flow, providerID string) {
	if providerID == "" {
		flow.Errors = append(flow.Errors, FlowError{Code: "provider_required", Message: "Provider is required"})
		h.renderFlowStep(w, r, flow)
		return
	}

	httputil.WriteJSON(w, http.StatusOK, map[string]any{
		"flow_id":      flow.ID,
		"action":       "redirect",
		"redirect_url": fmt.Sprintf("/v1/auth/sso/%s/start?flow_id=%s", providerID, flow.ID),
	})
}
