package login

import (
	"encoding/json"
	"fmt"
	"net/http"
	"strings"

	"github.com/zitadel/zitadel/internal/httputil"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/logging"
	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

func (h *Handler) flowTransitionToRegister(w http.ResponseWriter, r *http.Request, flow *Flow) {
	if !flow.SchemaConfig.Login.RegistrationAllowed {
		flow.Errors = append(flow.Errors, FlowError{Code: "registration_disabled", Message: "Registration is not available"})
		h.renderFlowStep(w, r, flow)
		return
	}

	flow.transitionToStep(StepRegister)
	flow.Errors = nil
	if flow.RegData == nil {
		flow.RegData = make(map[string]string)
	}
	h.renderFlowStep(w, r, flow)
}

func (h *Handler) flowSubmitRegister(w http.ResponseWriter, r *http.Request, flow *Flow, formData map[string]string) {
	h.mergeRegistrationData(flow, formData)

	if validationErrors := validateFlowRegistrationFields(flow.SchemaConfig.SchemaProps, flow.RegData); len(validationErrors) > 0 {
		h.respondWithFlowErrors(w, r, flow, validationErrors)
		return
	}

	identifier := resolveFlowRegistrationIdentifier(flow.SchemaConfig.SchemaProps, flow.RegData)
	displayName := flow.RegData["display_name"]
	if displayName == "" {
		displayName = identifier
	}

	schemaRec, ok := h.resolveFlowRegistrationSchema(w, r, flow)
	if !ok {
		return
	}

	payload, profileJSON, ok := h.buildFlowRegistrationPayload(w, r, flow, schemaRec.Schema, identifier, displayName)
	if !ok {
		return
	}

	newID := id.New()
	schemaID := schemaRec.ID
	orgID := httputil.ResolveOrgID(r, "1")
	instanceID := httputil.InstanceIDFromContext(r.Context())

	tx, err := h.db.SQL().BeginTx(r.Context(), nil)
	if err != nil {
		logging.Printf("[flow] %s registration tx failed: %v", flow.ID, err)
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "internal", Message: "Registration failed. Please try again."}})
		return
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(r.Context(),
		`INSERT INTO users (id, instance_id, org_id, identifier, display_name, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, ?, ?, ?, ?, 'active', ?, ?, datetime('now'), datetime('now'))`,
		newID, instanceID, orgID, identifier, displayName, schemaID, profileJSON,
	)
	if err != nil {
		logging.Printf("[flow] %s registration failed: %v", flow.ID, err)
		if strings.Contains(err.Error(), "UNIQUE") || strings.Contains(err.Error(), "unique") {
			h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "already_exists", Message: "An account with this identifier already exists"}})
		} else {
			h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "internal", Message: "Registration failed. Please try again."}})
		}
		return
	}
	if err := uniqueness.EnforceFromIdentifier(r.Context(), tx, newID, orgID, identifier); err != nil {
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "already_exists", Message: "An account with this identifier already exists"}})
		return
	}
	if err := uniqueness.Enforce(r.Context(), tx, newID, orgID, uniqueness.ExtractConstraints(schemaRec.Schema), payload); err != nil {
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "already_exists", Message: "An account with this identifier already exists"}})
		return
	}
	if err := tx.Commit(); err != nil {
		logging.Printf("[flow] %s registration commit failed: %v", flow.ID, err)
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "internal", Message: "Registration failed. Please try again."}})
		return
	}

	logging.Printf("[flow] %s registered new identity %s (%s)", flow.ID, newID, identifier)

	flow.IduserID = newID
	flow.Identifier = identifier
	flow.DisplayName = displayName
	flow.Verified = true
	flow.AuthMethod = "registration"
	flow.Errors = nil

	h.api.EmitAuthEvent(r.Context(), "auth.registration_completed", newID, map[string]any{
		"flow_id":    flow.ID,
		"identifier": identifier,
	})

	h.flowComplete(w, r, flow)
}

func (h *Handler) mergeRegistrationData(flow *Flow, formData map[string]string) {
	if flow.RegData == nil {
		flow.RegData = make(map[string]string)
	}
	for k, v := range formData {
		if k != "action" {
			flow.RegData[k] = v
		}
	}
}

func validateFlowRegistrationFields(fields []SchemaFieldDef, values map[string]string) []FlowError {
	var validationErrors []FlowError
	for _, field := range fields {
		if !field.Required {
			continue
		}
		if strings.TrimSpace(values[field.Name]) != "" {
			continue
		}
		label := field.Title
		if label == "" {
			label = humanize(field.Name)
		}
		validationErrors = append(validationErrors, FlowError{
			Code:    "field_required",
			Message: fmt.Sprintf("%s is required", label),
		})
	}
	return validationErrors
}

func resolveFlowRegistrationIdentifier(fields []SchemaFieldDef, values map[string]string) string {
	for _, field := range fields {
		if !field.Identifier {
			continue
		}
		if value := strings.TrimSpace(values[field.Name]); value != "" {
			return value
		}
	}
	if value := strings.TrimSpace(values["email"]); value != "" {
		return value
	}
	for _, value := range values {
		if trimmed := strings.TrimSpace(value); trimmed != "" {
			return trimmed
		}
	}
	return ""
}

func (h *Handler) resolveFlowRegistrationSchema(w http.ResponseWriter, r *http.Request, flow *Flow) (*schema.SchemaRecord, bool) {
	schemaID := flow.SchemaConfig.SchemaID
	if schemaID == "" {
		schemaRec, err := schema.ResolveDefaultHumanUserSchema(r.Context(), h.db.SQL(), h.db.Dialect())
		if err != nil {
			logging.Printf("[flow] %s default human user schema unavailable: %v", flow.ID, err)
			h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "internal", Message: "Registration is not available right now."}})
			return nil, false
		}
		return schemaRec, true
	}

	schemaRec, err := schema.LoadSchemaRecord(r.Context(), h.db.SQL(), schemaID, h.db.Dialect())
	if err != nil {
		logging.Printf("[flow] %s failed to load schema %s: %v", flow.ID, schemaID, err)
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "internal", Message: "Registration is not available right now."}})
		return nil, false
	}
	return schemaRec, true
}

func (h *Handler) buildFlowRegistrationPayload(w http.ResponseWriter, r *http.Request, flow *Flow, schemaJSON, identifier, displayName string) (map[string]any, string, bool) {
	registrationData := make(map[string]any, len(flow.RegData))
	for key, value := range flow.RegData {
		registrationData[key] = value
	}
	payload := schema.MaterializeUserData(schemaJSON, identifier, displayName, registrationData)
	if err := schema.ValidateData(schemaJSON, payload); err != nil {
		logging.Printf("[flow] %s registration validation failed: %v", flow.ID, err)
		h.respondWithFlowErrors(w, r, flow, []FlowError{{Code: "invalid_registration", Message: "Please complete the highlighted fields and try again."}})
		return nil, "", false
	}

	profileJSON := "{}"
	if len(flow.RegData) > 0 {
		if b, err := json.Marshal(flow.RegData); err == nil {
			profileJSON = string(b)
		}
	}

	return payload, profileJSON, true
}

func (h *Handler) respondWithFlowErrors(w http.ResponseWriter, r *http.Request, flow *Flow, errs []FlowError) {
	flow.Errors = errs
	h.renderFlowStep(w, r, flow)
}
