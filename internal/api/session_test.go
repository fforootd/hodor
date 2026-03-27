package api_test

import (
	"fmt"
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
)

func TestSession_AdminCanListAndRevoke(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("user1@test.com", "User 1")
	_ = srv.CreateSession(identityID) // create a session for the user

	adminToken := srv.LoginAdmin()

	// 1. List sessions
	code, body := srv.GetWithBearer("/v1/sessions", adminToken)
	if code != 200 {
		t.Fatalf("expected 200 listing sessions, got %d", code)
	}
	items, _ := body["items"].([]any)
	if len(items) == 0 {
		t.Fatal("expected sessions, got 0")
	}

	// 2. Revoke a specific session
	firstSession, _ := items[0].(map[string]any)
	sessionID := fmt.Sprintf("%v", firstSession["id"])

	code, _ = srv.PostJSONWithBearer("/v1/sessions/"+sessionID+"/revoke", nil, adminToken)
	if code != 204 {
		t.Fatalf("expected 204 revoking session, got %d", code)
	}

	// 3. Verify it's revoked
	code, _ = srv.GetWithBearer("/v1/sessions/"+sessionID, adminToken)
	if code == 200 {
		t.Fatalf("expected session to be revoked (not found/inactive), got 200")
	}
}

func TestSession_NonAdminCannotManage(t *testing.T) {
	srv := testutil.NewTestServer(t)
	identityID := srv.CreateIdentity("user2@test.com", "User 2")
	userToken := srv.CreateSession(identityID)

	code, _ := srv.GetWithBearer("/v1/sessions", userToken)
	if code != 403 {
		t.Fatalf("expected 403 non-admin listing sessions, got %d", code)
	}
}
