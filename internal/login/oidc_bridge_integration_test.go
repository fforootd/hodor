package login_test

import (
	"encoding/json"
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/testutil"
)

func TestOIDCFlowWithTrustedSessionRequiresConfirmationByDefault(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("trusted-oidc@example.com", "Trusted OIDC")
	token := ts.CreateSession(userID)
	authRequestID := insertOIDCAuthRequest(t, ts, "trusted-oidc-state", "https://oidcdebugger.com/debug")

	status, body := ts.RequestWithHeaders("POST", "/v1/login/flows", map[string]string{
		"Authorization": "Bearer " + token,
	}, map[string]any{
		"auth_request_id": authRequestID,
	})
	if status != 200 {
		t.Fatalf("create flow status = %d, want 200 body=%#v", status, body)
	}
	if got := body["step"]; got != "session_reuse" {
		t.Fatalf("step = %v, want session_reuse", got)
	}
	nodes := mustNodes(t, body)
	assertHasNodeType(t, nodes, "avatar")
	assertHeadingText(t, nodes, "Use your existing session?")
	assertOIDCAuthRequestPending(t, ts, authRequestID)

	status, completeBody := submitFlow(t, ts, body["flow_id"].(string), map[string]any{
		"action": "use_session",
	}, token)
	if status != 200 {
		t.Fatalf("use_session status = %d, want 200 body=%#v", status, completeBody)
	}
	if got := completeBody["redirect_uri"]; got != "/authorize/callback?id="+authRequestID {
		t.Fatalf("redirect_uri = %v, want authorize callback", got)
	}

	assertOIDCAuthRequestCompleted(t, ts, authRequestID, userID)
}

func TestOIDCFlowWithPromptNoneCompletesSilently(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("silent-oidc@example.com", "Silent OIDC")
	token := ts.CreateSession(userID)
	authRequestID := insertOIDCAuthRequest(t, ts, "silent-oidc-state", "https://oidcdebugger.com/debug", map[string]any{
		"prompt": []string{"none"},
	})

	status, body := ts.RequestWithHeaders("POST", "/v1/login/flows", map[string]string{
		"Authorization": "Bearer " + token,
	}, map[string]any{
		"auth_request_id": authRequestID,
	})
	if status != 200 {
		t.Fatalf("create flow status = %d, want 200 body=%#v", status, body)
	}
	if got := body["step"]; got != "complete" {
		t.Fatalf("step = %v, want complete", got)
	}
	if got := body["redirect_uri"]; got != "/authorize/callback?id="+authRequestID {
		t.Fatalf("redirect_uri = %v, want authorize callback", got)
	}

	assertOIDCAuthRequestCompleted(t, ts, authRequestID, userID)
}

func TestOIDCFlowWithPromptLoginSkipsSessionReuse(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("prompt-login@example.com", "Prompt Login")
	token := ts.CreateSession(userID)
	authRequestID := insertOIDCAuthRequest(t, ts, "prompt-login-state", "https://oidcdebugger.com/debug", map[string]any{
		"prompt":     []string{"login"},
		"login_hint": "prompt-login@example.com",
	})

	status, body := ts.RequestWithHeaders("POST", "/v1/login/flows", map[string]string{
		"Authorization": "Bearer " + token,
	}, map[string]any{
		"auth_request_id": authRequestID,
	})
	if status != 200 {
		t.Fatalf("create flow status = %d, want 200 body=%#v", status, body)
	}
	if got := body["step"]; got != "identifier" {
		t.Fatalf("step = %v, want identifier", got)
	}
	assertIdentifierPrefill(t, body, "prompt-login@example.com")
	assertOIDCAuthRequestPending(t, ts, authRequestID)
}

func TestOIDCFlowWithPromptSelectAccountSkipsSessionReuse(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("select-account@example.com", "Select Account")
	token := ts.CreateSession(userID)
	authRequestID := insertOIDCAuthRequest(t, ts, "select-account-state", "https://oidcdebugger.com/debug", map[string]any{
		"prompt":     []string{"select_account"},
		"login_hint": "select-account@example.com",
	})

	status, body := ts.RequestWithHeaders("POST", "/v1/login/flows", map[string]string{
		"Authorization": "Bearer " + token,
	}, map[string]any{
		"auth_request_id": authRequestID,
	})
	if status != 200 {
		t.Fatalf("create flow status = %d, want 200 body=%#v", status, body)
	}
	if got := body["step"]; got != "identifier" {
		t.Fatalf("step = %v, want identifier", got)
	}
	assertIdentifierPrefill(t, body, "select-account@example.com")
	assertOIDCAuthRequestPending(t, ts, authRequestID)
}

func TestOIDCFlowCompleteReturnsAuthorizeCallback(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("password-oidc@example.com", "Password OIDC")
	if err := auth.NewPasswords(ts.DB).SetPassword(t.Context(), userID, "super-secret-password"); err != nil {
		t.Fatalf("SetPassword: %v", err)
	}
	authRequestID := insertOIDCAuthRequest(t, ts, "password-oidc-state", "https://oidcdebugger.com/debug")

	flowID := createLoginFlow(t, ts, nil, map[string]any{
		"auth_request_id": authRequestID,
	})
	_, body := submitFlow(t, ts, flowID, map[string]any{
		"action":     "identifier",
		"identifier": "password-oidc@example.com",
	}, "")
	if got := body["step"]; got != "auth_select" {
		t.Fatalf("step after identifier = %v, want auth_select", got)
	}

	status, completeBody := submitFlow(t, ts, flowID, map[string]any{
		"action":   "password",
		"password": "super-secret-password",
	}, "")
	if status != 200 {
		t.Fatalf("password submit status = %d, want 200 body=%#v", status, completeBody)
	}
	if got := completeBody["step"]; got != "complete" {
		t.Fatalf("complete step = %v, want complete", got)
	}
	if got := completeBody["redirect_uri"]; got != "/authorize/callback?id="+authRequestID {
		t.Fatalf("redirect_uri = %v, want authorize callback", got)
	}

	assertOIDCAuthRequestCompleted(t, ts, authRequestID, userID)
}

func insertOIDCAuthRequest(t *testing.T, ts *testutil.TestServer, state, redirectURI string, extras ...map[string]any) string {
	t.Helper()

	requestID := id.New()
	dataJSON := "{}"
	if len(extras) > 0 && extras[0] != nil {
		encoded, err := json.Marshal(extras[0])
		if err != nil {
			t.Fatalf("marshal oidc auth request data: %v", err)
		}
		dataJSON = string(encoded)
	}
	_, err := ts.DB.SQL().Exec(
		`INSERT INTO auth_states (id, type, state, client_id, redirect_uri, scopes, nonce, response_type, data, expires_at, created_at)
		 VALUES (?, 'oidc_auth', ?, 'test-client', ?, 'openid', 'nonce-123', 'code', ?, ?, ?)`,
		requestID,
		state,
		redirectURI,
		dataJSON,
		time.Now().UTC().Add(10*time.Minute).Format("2006-01-02 15:04:05"),
		time.Now().UTC().Format("2006-01-02 15:04:05"),
	)
	if err != nil {
		t.Fatalf("insert oidc auth request: %v", err)
	}
	return requestID
}

func assertOIDCAuthRequestPending(t *testing.T, ts *testutil.TestServer, authRequestID string) {
	t.Helper()

	var gotUserID string
	var done int
	err := ts.DB.SQL().QueryRow(
		`SELECT COALESCE(user_id, ''), done FROM auth_states WHERE id = ?`,
		authRequestID,
	).Scan(&gotUserID, &done)
	if err != nil {
		t.Fatalf("load auth request: %v", err)
	}
	if gotUserID != "" {
		t.Fatalf("user_id = %q, want empty", gotUserID)
	}
	if done != 0 {
		t.Fatalf("done = %d, want 0", done)
	}
}

func assertOIDCAuthRequestCompleted(t *testing.T, ts *testutil.TestServer, authRequestID, wantUserID string) {
	t.Helper()

	var gotUserID string
	var done int
	err := ts.DB.SQL().QueryRow(
		`SELECT user_id, done FROM auth_states WHERE id = ?`,
		authRequestID,
	).Scan(&gotUserID, &done)
	if err != nil {
		t.Fatalf("load auth request: %v", err)
	}
	if gotUserID != wantUserID {
		t.Fatalf("user_id = %q, want %q", gotUserID, wantUserID)
	}
	if done != 1 {
		t.Fatalf("done = %d, want 1", done)
	}
}

func assertIdentifierPrefill(t *testing.T, body map[string]any, want string) {
	t.Helper()

	nodes := mustNodes(t, body)
	for _, raw := range nodes {
		node, _ := raw.(map[string]any)
		if node["type"] != "input" || node["name"] != "identifier" {
			continue
		}
		if got := node["value"]; got != want {
			t.Fatalf("identifier value = %v, want %q", got, want)
		}
		return
	}
	t.Fatalf("identifier input not found in %#v", nodes)
}
