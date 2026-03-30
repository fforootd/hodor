package login

import (
	"net/http/httptest"
	"strings"
	"testing"
)

func TestEnsureCaptchaVerifiedForAction_BlocksProtectedAction(t *testing.T) {
	h := &Handler{flows: NewFlowStore()}
	flow := &Flow{
		ID:          "flow_123",
		CurrentStep: StepIdentifier,
		SchemaConfig: &SchemaAuthConfig{
			Branding: defaultBrandingConfig(),
			Captcha:  &CaptchaConfig{Provider: "altcha", Mode: "always"},
		},
	}
	h.flows.Put(flow)

	req := httptest.NewRequest("POST", "/v1/login/flows/flow_123/submit", nil)
	rec := httptest.NewRecorder()

	allowed := h.ensureCaptchaVerifiedForAction(rec, req, flow, "identifier")
	if allowed {
		t.Fatal("expected protected action to be blocked until captcha is verified")
	}
	if rec.Code != 200 {
		t.Fatalf("status = %d, want 200", rec.Code)
	}
	if body := rec.Body.String(); body == "" || !strings.Contains(body, "captcha_required") {
		t.Fatalf("response body = %q, want captcha_required error", body)
	}
}

func TestEnsureCaptchaVerifiedForAction_AllowsVerifiedScope(t *testing.T) {
	h := &Handler{flows: NewFlowStore()}
	flow := &Flow{
		ID:                   "flow_123",
		CurrentStep:          StepPassword,
		CaptchaVerified:      true,
		CaptchaVerifiedScope: "login",
		SchemaConfig: &SchemaAuthConfig{
			Branding: defaultBrandingConfig(),
			Captcha:  &CaptchaConfig{Provider: "altcha", Mode: "always"},
		},
	}

	req := httptest.NewRequest("POST", "/v1/login/flows/flow_123/submit", nil)
	rec := httptest.NewRecorder()

	allowed := h.ensureCaptchaVerifiedForAction(rec, req, flow, "password")
	if !allowed {
		t.Fatal("expected verified captcha scope to allow protected action")
	}
	if rec.Body.Len() != 0 {
		t.Fatalf("unexpected response body: %q", rec.Body.String())
	}
}
