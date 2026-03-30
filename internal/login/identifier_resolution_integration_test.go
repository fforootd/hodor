package login_test

import (
	"testing"

	"github.com/zitadel/zitadel/internal/testutil"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

func TestCreateIdentityIndexesIdentifierForLoginResolution(t *testing.T) {
	ts := testutil.NewTestServer(t)

	userID := ts.CreateIdentity("privacy@example.com", "Private Person")
	resolved, err := uniqueness.ResolveIdentifier(t.Context(), ts.DB.SQL(), "privacy@example.com", "")
	if err != nil {
		t.Fatalf("ResolveIdentifier() error = %v", err)
	}
	if resolved.UserID != userID {
		t.Fatalf("ResolveIdentifier().UserID = %q, want %q", resolved.UserID, userID)
	}
}

func TestLoginFlowIdentifierSubmitResolvesIndexedUser(t *testing.T) {
	ts := testutil.NewTestServer(t)

	ts.CreateIdentity("privacy@example.com", "Private Person")
	flowID := createLoginFlow(t, ts, nil)
	status, body := submitFlow(t, ts, flowID, map[string]any{
		"action":     "identifier",
		"identifier": "privacy@example.com",
	}, "")
	if status != 200 {
		t.Fatalf("submit status = %d body=%#v", status, body)
	}
	if got := body["step"]; got != "auth_select" {
		t.Fatalf("step = %v body=%#v", got, body)
	}
}
