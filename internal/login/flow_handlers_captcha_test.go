package login

import (
	"context"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/zitadel/zitadel/internal/risk"
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

func TestEnsureCaptchaVerifiedForAction_RiskBasedFailsSafeOnEvaluatorError(t *testing.T) {
	h := &Handler{
		flows: NewFlowStore(),
		api:   noopSessionCreator{},
		risk:  stubRiskEvaluator{err: context.DeadlineExceeded},
	}
	flow := &Flow{
		ID:          "flow_risk",
		CurrentStep: StepIdentifier,
		SchemaConfig: &SchemaAuthConfig{
			Branding: defaultBrandingConfig(),
			Captcha:  &CaptchaConfig{Provider: "altcha", Mode: "risk_based"},
		},
	}
	h.flows.Put(flow)

	req := httptest.NewRequest("POST", "/v1/login/flows/flow_risk/submit", nil)
	rec := httptest.NewRecorder()

	allowed := h.ensureCaptchaVerifiedForAction(rec, req, flow, "identifier")
	if allowed {
		t.Fatal("expected protected action to fail safe to captcha on evaluator error")
	}
	if rec.Code != 200 {
		t.Fatalf("status = %d, want 200", rec.Code)
	}
	if body := rec.Body.String(); body == "" || !strings.Contains(body, "captcha_required") {
		t.Fatalf("response body = %q, want captcha_required error", body)
	}
}

type noopSessionCreator struct{}

func (noopSessionCreator) CreateSessionForLogin(context.Context, string, string, string, *risk.Signals, *SessionProvenance) (*CreateSessionResponse, error) {
	return &CreateSessionResponse{Session: SessionInfo{ID: "session_test"}, Token: "token"}, nil
}

func (noopSessionCreator) EmitAuthEvent(context.Context, string, string, map[string]any) {}

func (noopSessionCreator) EmitEvent(context.Context, string, string, string, string, map[string]any) {
}

type stubRiskEvaluator struct {
	err error
}

func (s stubRiskEvaluator) Evaluate(context.Context, risk.Input) (*risk.Result, error) {
	return nil, s.err
}
