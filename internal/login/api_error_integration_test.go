package login_test

import (
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

func TestHandleFlowCreate_ReturnsStructuredFlowNotFound(t *testing.T) {
	ts := testutil.NewTestServer(t)

	status, body := ts.PostJSONRaw("/v1/login/flows?flow=lf_missing", map[string]any{})
	if status != 404 {
		t.Fatalf("status = %d, want 404", status)
	}

	errBody, ok := body["error"].(map[string]any)
	if !ok {
		t.Fatalf("error body missing or wrong type: %#v", body)
	}
	if errBody["code"] != "flow_not_found" {
		t.Fatalf("code = %v, want flow_not_found", errBody["code"])
	}
	if errBody["kind"] != "flow" {
		t.Fatalf("kind = %v, want flow", errBody["kind"])
	}
	if errBody["retryable"] != false {
		t.Fatalf("retryable = %v, want false", errBody["retryable"])
	}
}

func TestHandleFlowCreate_ReturnsStructuredConfigError(t *testing.T) {
	ts := testutil.NewTestServer(t)

	if _, err := ts.DB.SQL().Exec(`DELETE FROM login_flows`); err != nil {
		t.Fatalf("delete login_flows: %v", err)
	}

	status, body := ts.PostJSONRaw("/v1/login/flows", map[string]any{})
	if status != 500 {
		t.Fatalf("status = %d, want 500", status)
	}

	errBody, ok := body["error"].(map[string]any)
	if !ok {
		t.Fatalf("error body missing or wrong type: %#v", body)
	}
	if errBody["code"] != "flow_config_invalid" {
		t.Fatalf("code = %v, want flow_config_invalid", errBody["code"])
	}
	if errBody["kind"] != "configuration" {
		t.Fatalf("kind = %v, want configuration", errBody["kind"])
	}
	if errBody["retryable"] != false {
		t.Fatalf("retryable = %v, want false", errBody["retryable"])
	}
}

func TestHandleFlowCreate_ReturnsStructuredStartupError(t *testing.T) {
	ts := testutil.NewTestServer(t)

	if _, err := ts.DB.SQL().Exec(`DROP TABLE login_flows`); err != nil {
		t.Fatalf("drop login_flows: %v", err)
	}

	status, body := ts.PostJSONRaw("/v1/login/flows", map[string]any{})
	if status != 503 {
		t.Fatalf("status = %d, want 503", status)
	}

	errBody, ok := body["error"].(map[string]any)
	if !ok {
		t.Fatalf("error body missing or wrong type: %#v", body)
	}
	if errBody["code"] != "service_starting" {
		t.Fatalf("code = %v, want service_starting", errBody["code"])
	}
	if errBody["kind"] != "startup" {
		t.Fatalf("kind = %v, want startup", errBody["kind"])
	}
	if errBody["retryable"] != true {
		t.Fatalf("retryable = %v, want true", errBody["retryable"])
	}
}
