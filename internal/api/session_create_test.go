package api_test

import (
	"net/http"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

func TestSession_AdminCanCreateSession(t *testing.T) {
	srv := testutil.NewTestServer(t)
	adminToken := srv.LoginAdmin()
	userID := srv.CreateIdentity("session-create@example.com", "Session Create")

	createCode, created := srv.PostJSONWithBearer("/v1/sessions", map[string]any{
		"user_id":    userID,
		"user_agent": "api-test",
		"ip_address": "127.0.0.2",
	}, adminToken)
	if createCode != http.StatusCreated {
		t.Fatalf("create session status = %d body=%#v", createCode, created)
	}

	sessionMap, _ := created["session"].(map[string]any)
	if sessionMap["user_id"] != userID {
		t.Fatalf("session user_id = %v, want %s", sessionMap["user_id"], userID)
	}
	if created["token"] == "" {
		t.Fatal("expected raw session token in response")
	}

	sessionID, _ := sessionMap["id"].(string)
	if sessionID == "" {
		t.Fatal("expected session id")
	}

	getCode, loaded := srv.GetWithBearer("/v1/sessions/"+sessionID, adminToken)
	if getCode != http.StatusOK {
		t.Fatalf("get session status = %d", getCode)
	}
	if loaded["user_agent"] != "api-test" {
		t.Fatalf("loaded user_agent = %v", loaded["user_agent"])
	}
}
