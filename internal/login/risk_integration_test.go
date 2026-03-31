package login_test

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"strconv"
	"testing"

	"github.com/zitadel/zitadel/internal/auth"
	"github.com/zitadel/zitadel/internal/captcha"
	"github.com/zitadel/zitadel/internal/id"
	"github.com/zitadel/zitadel/internal/testutil"
)

func TestRiskBasedCaptcha_AllowsKnownFingerprintWithoutChallenge(t *testing.T) {
	ts := testutil.NewTestServer(t)
	configureDefaultFlowRiskBased(t, ts, true)

	userID := ts.CreateIdentity("known@example.com", "Known User")
	if err := auth.NewPasswords(ts.DB).SetPassword(t.Context(), userID, "s3cret-password"); err != nil {
		t.Fatalf("set password: %v", err)
	}
	seedKnownFingerprintEvent(t, ts, userID, "fp-known")

	headers := map[string]string{"X-Fingerprint": "fp-known"}
	status, body := ts.RequestWithHeaders("POST", "/v1/login/flows", headers, map[string]any{"fingerprint": "fp-known"})
	if status != 200 {
		t.Fatalf("create flow status = %d, want 200 body=%#v", status, body)
	}
	if body["captcha_required"] == true {
		t.Fatalf("expected low-risk flow create to skip captcha, body=%#v", body)
	}

	flowID := body["flow_id"].(string)
	status, body = ts.RequestWithHeaders("POST", "/v1/login/flows/"+flowID+"/submit", headers, map[string]any{
		"action":     "identifier",
		"identifier": "known@example.com",
	})
	if status != 200 {
		t.Fatalf("submit identifier status = %d, want 200 body=%#v", status, body)
	}
	if body["captcha_required"] == true {
		t.Fatalf("expected known fingerprint auth_select to skip captcha, body=%#v", body)
	}

	status, body = ts.RequestWithHeaders("POST", "/v1/login/flows/"+flowID+"/submit", headers, map[string]any{
		"action":   "password",
		"password": "s3cret-password",
	})
	if status != 200 {
		t.Fatalf("submit password status = %d, want 200 body=%#v", status, body)
	}
	if body["step"] != "complete" {
		t.Fatalf("step = %v, want complete body=%#v", body["step"], body)
	}
}

func TestRiskBasedCaptcha_RequiresChallengeAndPersistsRiskMetadata(t *testing.T) {
	ts := testutil.NewTestServer(t)
	configureDefaultFlowRiskBased(t, ts, true)

	userID := ts.CreateIdentity("unknown-device@example.com", "Unknown Device")
	if err := auth.NewPasswords(ts.DB).SetPassword(t.Context(), userID, "s3cret-password"); err != nil {
		t.Fatalf("set password: %v", err)
	}

	status, body := ts.PostJSONRaw("/v1/login/flows", map[string]any{})
	if status != 200 {
		t.Fatalf("create flow status = %d, want 200 body=%#v", status, body)
	}
	if body["captcha_required"] != true {
		t.Fatalf("expected elevated-risk flow create to require captcha, body=%#v", body)
	}

	flowID := body["flow_id"].(string)
	status, body = ts.PostJSONRaw("/v1/login/flows/"+flowID+"/submit", map[string]any{
		"action":     "identifier",
		"identifier": "unknown-device@example.com",
	})
	if status != 200 {
		t.Fatalf("submit identifier status = %d, want 200 body=%#v", status, body)
	}
	if body["step"] != "identifier" {
		t.Fatalf("step = %v, want identifier when captcha blocks body=%#v", body["step"], body)
	}

	_, challengeBody := ts.RequestWithHeaders("GET", "/v1/login/flows/"+flowID+"/captcha/challenge", nil, nil)
	payload := solveAltchaPayload(t, challengeBody)

	status, body = ts.PostJSONRaw("/v1/login/flows/"+flowID+"/submit", map[string]any{
		"action":         "captcha_submit",
		"altcha_payload": payload,
	})
	if status != 200 {
		t.Fatalf("submit captcha status = %d, want 200 body=%#v", status, body)
	}
	if body["captcha_verified"] != true {
		t.Fatalf("captcha_verified = %v, want true body=%#v", body["captcha_verified"], body)
	}

	status, body = ts.PostJSONRaw("/v1/login/flows/"+flowID+"/submit", map[string]any{
		"action":     "identifier",
		"identifier": "unknown-device@example.com",
	})
	if status != 200 {
		t.Fatalf("submit identifier after captcha status = %d, want 200 body=%#v", status, body)
	}

	status, body = ts.PostJSONRaw("/v1/login/flows/"+flowID+"/submit", map[string]any{
		"action":   "password",
		"password": "s3cret-password",
	})
	if status != 200 {
		t.Fatalf("submit password status = %d, want 200 body=%#v", status, body)
	}
	if body["step"] != "complete" {
		t.Fatalf("step = %v, want complete body=%#v", body["step"], body)
	}

	sessionID := body["session_id"].(string)
	var metadataJSON string
	if err := ts.DB.SQL().QueryRow(`SELECT metadata FROM sessions WHERE id = ?`, sessionID).Scan(&metadataJSON); err != nil {
		t.Fatalf("load session metadata: %v", err)
	}

	var metadata map[string]any
	if err := json.Unmarshal([]byte(metadataJSON), &metadata); err != nil {
		t.Fatalf("unmarshal session metadata: %v", err)
	}
	riskMeta, ok := metadata["risk"].(map[string]any)
	if !ok {
		t.Fatalf("risk metadata missing: %#v", metadata)
	}
	if riskMeta["recommended_next_step"] != "require_step_up" {
		t.Fatalf("recommended_next_step = %v, want require_step_up", riskMeta["recommended_next_step"])
	}
	reasons, _ := riskMeta["reasons"].([]any)
	if len(reasons) == 0 {
		t.Fatalf("expected risk reasons, got %#v", riskMeta["reasons"])
	}

	var riskEventCount int
	if err := ts.DB.SQL().QueryRow(
		`SELECT COUNT(*) FROM events WHERE event_type = 'signal.risk_evaluated' AND flow_id = ?`,
		flowID,
	).Scan(&riskEventCount); err != nil {
		t.Fatalf("count risk events: %v", err)
	}
	if riskEventCount < 2 {
		t.Fatalf("riskEventCount = %d, want >= 2", riskEventCount)
	}
}

func configureDefaultFlowRiskBased(t *testing.T, ts *testutil.TestServer, fingerprintEnabled bool) {
	t.Helper()

	var flowID, configJSON string
	if err := ts.DB.SQL().QueryRow(`SELECT id, COALESCE(config,'{}') FROM login_flows WHERE is_default = 1 OR is_default = true LIMIT 1`).Scan(&flowID, &configJSON); err != nil {
		t.Fatalf("load default login flow: %v", err)
	}

	var config map[string]any
	if err := json.Unmarshal([]byte(configJSON), &config); err != nil {
		t.Fatalf("unmarshal flow config: %v", err)
	}
	config["captcha"] = map[string]any{
		"provider":   "altcha",
		"mode":       "risk_based",
		"difficulty": 2,
	}
	config["fingerprint"] = map[string]any{
		"enabled":  fingerprintEnabled,
		"provider": "thumbmarkjs",
		"persist":  true,
	}

	updated, err := json.Marshal(config)
	if err != nil {
		t.Fatalf("marshal flow config: %v", err)
	}
	if _, err := ts.DB.SQL().Exec(`UPDATE login_flows SET config = ? WHERE id = ?`, string(updated), flowID); err != nil {
		t.Fatalf("update flow config: %v", err)
	}
}

func seedKnownFingerprintEvent(t *testing.T, ts *testutil.TestServer, userID, fingerprint string) {
	t.Helper()

	_, err := ts.DB.SQL().Exec(
		`INSERT INTO events (id, event_type, category, org_id, actor_id, aggregate_id, aggregate_type, payload, metadata, fingerprint, created_at)
		 VALUES (?, 'auth.login_completed', 'auth', '0', ?, ?, 'session', '{}', '{}', ?, datetime('now'))`,
		id.New(), userID, userID, fingerprint,
	)
	if err != nil {
		t.Fatalf("seed fingerprint event: %v", err)
	}
}

func solveAltchaPayload(t *testing.T, challengeBody map[string]any) string {
	t.Helper()

	challenge := captcha.Challenge{
		Algorithm: challengeBody["algorithm"].(string),
		Challenge: challengeBody["challenge"].(string),
		Salt:      challengeBody["salt"].(string),
		Signature: challengeBody["signature"].(string),
		MaxNumber: int(challengeBody["maxnumber"].(float64)),
	}

	solutionNumber := -1
	for i := 0; i < challenge.MaxNumber; i++ {
		hash := sha256.Sum256([]byte(challenge.Salt + strconv.Itoa(i)))
		if hex.EncodeToString(hash[:]) == challenge.Challenge {
			solutionNumber = i
			break
		}
	}
	if solutionNumber < 0 {
		t.Fatal("failed to solve altcha challenge")
	}

	payload, err := json.Marshal(captcha.Solution{
		Algorithm: challenge.Algorithm,
		Challenge: challenge.Challenge,
		Number:    solutionNumber,
		Salt:      challenge.Salt,
		Signature: challenge.Signature,
		Took:      250,
	})
	if err != nil {
		t.Fatalf("marshal altcha solution: %v", err)
	}
	return string(payload)
}
