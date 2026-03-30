package login_test

import (
	"testing"
	"time"

	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/testutil"
)

func TestAnonymousLoginAuthSelectOnlyEchoesTypedIdentifier(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("privacy@example.com", "Private Person")
	if userID == "" {
		t.Fatal("expected test identity")
	}

	flowID := createLoginFlow(t, ts, nil)
	_, body := submitFlow(t, ts, flowID, map[string]any{
		"action":     "identifier",
		"identifier": "privacy@example.com",
	}, "")

	if got := body["step"]; got != "auth_select" {
		t.Fatalf("step = %v, want auth_select", got)
	}
	if _, ok := body["identity"]; ok {
		t.Fatalf("did not expect identity payload for anonymous login: %#v", body["identity"])
	}

	nodes := mustNodes(t, body)
	assertNoNodeType(t, nodes, "avatar")
	assertHeadingText(t, nodes, "privacy@example.com")
}

func TestTrustedSessionReauthCanShowKnownUserIdentity(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("reauth@example.com", "Reauth User")
	token := ts.CreateSession(userID)

	flowID := createLoginFlow(t, ts, map[string]string{"Authorization": "Bearer " + token})
	_, body := submitFlow(t, ts, flowID, map[string]any{
		"action":     "identifier",
		"identifier": "reauth@example.com",
	}, "")

	identity, ok := body["identity"].(map[string]any)
	if !ok {
		t.Fatalf("expected identity payload for trusted reauth, got %#v", body["identity"])
	}
	if identity["display_name"] != "Reauth User" {
		t.Fatalf("display_name = %v, want Reauth User", identity["display_name"])
	}

	nodes := mustNodes(t, body)
	assertHasNodeType(t, nodes, "avatar")
	assertHeadingText(t, nodes, "Reauth User")
}

func TestSwitchAccountResetsRevealModeUntilTrustedUserMatchesAgain(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("reauth@example.com", "Reauth User")
	token := ts.CreateSession(userID)
	adminID := ts.CreateIdentity("admin@example.com", "Admin User")
	if adminID == "" {
		t.Fatal("expected second test identity")
	}

	flowID := createLoginFlow(t, ts, map[string]string{"Authorization": "Bearer " + token})
	_, _ = submitFlow(t, ts, flowID, map[string]any{
		"action":     "identifier",
		"identifier": "reauth@example.com",
	}, "")

	_, backBody := submitFlow(t, ts, flowID, map[string]any{"action": "back"}, "")
	if got := backBody["step"]; got != "identifier" {
		t.Fatalf("back step = %v, want identifier", got)
	}
	if _, ok := backBody["identity"]; ok {
		t.Fatalf("did not expect identity payload after switching account: %#v", backBody["identity"])
	}

	_, body := submitFlow(t, ts, flowID, map[string]any{
		"action":     "identifier",
		"identifier": "admin@example.com",
	}, "")
	if _, ok := body["identity"]; ok {
		t.Fatalf("did not expect identity payload after switching to different account: %#v", body["identity"])
	}

	nodes := mustNodes(t, body)
	assertNoNodeType(t, nodes, "avatar")
	assertHeadingText(t, nodes, "admin@example.com")
}

func TestOIDCAuthStateWithKnownUserAllowsIdentityReveal(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("oidc-known@example.com", "OIDC Known")
	insertOIDCAuthState(t, ts, "oidc-known-state", userID)

	flowID := createLoginFlow(t, ts, nil, map[string]any{"state": "oidc-known-state"})
	_, body := submitFlow(t, ts, flowID, map[string]any{
		"action":     "identifier",
		"identifier": "oidc-known@example.com",
	}, "")

	if _, ok := body["identity"].(map[string]any); !ok {
		t.Fatalf("expected identity payload for trusted OIDC reauth, got %#v", body["identity"])
	}
	nodes := mustNodes(t, body)
	assertHasNodeType(t, nodes, "avatar")
	assertHeadingText(t, nodes, "OIDC Known")
}

func TestOIDCAuthStateWithoutKnownUserStaysAnonymous(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("oidc-anon@example.com", "OIDC Anonymous")
	if userID == "" {
		t.Fatal("expected test identity")
	}
	insertOIDCAuthState(t, ts, "oidc-anon-state", "")

	flowID := createLoginFlow(t, ts, nil, map[string]any{"state": "oidc-anon-state"})
	_, body := submitFlow(t, ts, flowID, map[string]any{
		"action":     "identifier",
		"identifier": "oidc-anon@example.com",
	}, "")

	if _, ok := body["identity"]; ok {
		t.Fatalf("did not expect identity payload for anonymous OIDC flow: %#v", body["identity"])
	}
	nodes := mustNodes(t, body)
	assertNoNodeType(t, nodes, "avatar")
	assertHeadingText(t, nodes, "oidc-anon@example.com")
}

func createLoginFlow(t *testing.T, ts *testutil.TestServer, headers map[string]string, bodies ...map[string]any) string {
	t.Helper()

	body := map[string]any{}
	if len(bodies) > 0 && bodies[0] != nil {
		body = bodies[0]
	}

	status, respBody := ts.RequestWithHeaders("POST", "/v1/login/flows", headers, body)
	if status != 200 {
		t.Fatalf("create flow status = %d, want 200 body=%#v", status, respBody)
	}
	flowID, _ := respBody["flow_id"].(string)
	if flowID == "" {
		t.Fatalf("missing flow_id in response: %#v", respBody)
	}
	return flowID
}

func submitFlow(t *testing.T, ts *testutil.TestServer, flowID string, body map[string]any, token string) (int, map[string]any) {
	t.Helper()
	path := "/v1/login/flows/" + flowID + "/submit"
	if token != "" {
		return ts.PostJSONWithBearer(path, body, token)
	}
	return ts.PostJSONRaw(path, body)
}

func insertOIDCAuthState(t *testing.T, ts *testutil.TestServer, state, userID string) {
	t.Helper()

	_, err := ts.DB.SQL().Exec(
		`INSERT INTO auth_states (id, type, state, user_id, expires_at, created_at)
		 VALUES (?, 'oidc_auth', ?, ?, ?, ?)`,
		id.New(),
		state,
		userID,
		time.Now().UTC().Add(10*time.Minute).Format("2006-01-02 15:04:05"),
		time.Now().UTC().Format("2006-01-02 15:04:05"),
	)
	if err != nil {
		t.Fatalf("insert auth_state: %v", err)
	}
}

func mustNodes(t *testing.T, body map[string]any) []any {
	t.Helper()
	nodes, ok := body["nodes"].([]any)
	if !ok {
		t.Fatalf("nodes missing or wrong type: %#v", body["nodes"])
	}
	return nodes
}

func assertNoNodeType(t *testing.T, nodes []any, wantType string) {
	t.Helper()
	for _, raw := range nodes {
		node, _ := raw.(map[string]any)
		if node["type"] == wantType {
			t.Fatalf("did not expect node type %q in %#v", wantType, node)
		}
	}
}

func assertHasNodeType(t *testing.T, nodes []any, wantType string) {
	t.Helper()
	for _, raw := range nodes {
		node, _ := raw.(map[string]any)
		if node["type"] == wantType {
			return
		}
	}
	t.Fatalf("expected node type %q in %#v", wantType, nodes)
}

func assertHeadingText(t *testing.T, nodes []any, want string) {
	t.Helper()
	for _, raw := range nodes {
		node, _ := raw.(map[string]any)
		if node["type"] == "heading" {
			if node["text"] != want {
				t.Fatalf("heading text = %v, want %q", node["text"], want)
			}
			return
		}
	}
	t.Fatalf("heading node not found in %#v", nodes)
}
