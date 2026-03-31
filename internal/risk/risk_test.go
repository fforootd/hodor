package risk

import "testing"

func TestEvaluatePreAuthAllowsKnownFingerprint(t *testing.T) {
	result := evaluate(Input{
		Stage: StagePreAuth,
		Signals: Signals{
			VisitorID: "fp-known",
			RequestID: "req_123",
		},
	}, historySnapshot{
		knownFingerprint: true,
	})

	if result.Level != LevelLow {
		t.Fatalf("Level = %q, want %q", result.Level, LevelLow)
	}
	if result.RecommendedNextStep != RecommendationAllow {
		t.Fatalf("RecommendedNextStep = %q, want %q", result.RecommendedNextStep, RecommendationAllow)
	}
}

func TestEvaluatePreAuthRequiresCaptchaWhenFingerprintMissing(t *testing.T) {
	result := evaluate(Input{
		Stage: StagePreAuth,
		Signals: Signals{
			RequestID: "req_456",
		},
	}, historySnapshot{})

	if result.Level != LevelMedium {
		t.Fatalf("Level = %q, want %q", result.Level, LevelMedium)
	}
	if result.RecommendedNextStep != RecommendationRequireCaptcha {
		t.Fatalf("RecommendedNextStep = %q, want %q", result.RecommendedNextStep, RecommendationRequireCaptcha)
	}
	if !containsReason(result.Reasons, ReasonMissingFingerprint) {
		t.Fatalf("Reasons = %#v, want %q", result.Reasons, ReasonMissingFingerprint)
	}
}

func TestEvaluatePostAuthFlagsLowAssuranceAndRecentFailures(t *testing.T) {
	result := evaluate(Input{
		Stage:      StagePostAuth,
		UserID:     "user_123",
		AuthMethod: "password",
		Signals: Signals{
			VisitorID:       "fp-known",
			FingerprintHash: "fp-known",
			CaptchaVerified: true,
			PoWCompleted:    true,
			PoWDurationMs:   1200,
			RequestID:       "req_789",
		},
	}, historySnapshot{
		recentLoginFailures:  4,
		recentSessionRevokes: 1,
		knownFingerprint:     true,
	})

	if result.Level != LevelMedium {
		t.Fatalf("Level = %q, want %q", result.Level, LevelMedium)
	}
	if result.RecommendedNextStep != RecommendationRequireStepUp {
		t.Fatalf("RecommendedNextStep = %q, want %q", result.RecommendedNextStep, RecommendationRequireStepUp)
	}
	if !containsReason(result.Reasons, ReasonLowAssuranceAuthMethod) {
		t.Fatalf("Reasons = %#v, want %q", result.Reasons, ReasonLowAssuranceAuthMethod)
	}
	if !containsReason(result.Reasons, ReasonHighRecentFailures) {
		t.Fatalf("Reasons = %#v, want %q", result.Reasons, ReasonHighRecentFailures)
	}
}

func TestEvaluatePostAuthFlagsSuspiciousPowTimingAndNewFingerprint(t *testing.T) {
	result := evaluate(Input{
		Stage:      StagePostAuth,
		UserID:     "user_999",
		AuthMethod: "password",
		Signals: Signals{
			VisitorID:       "fp-new",
			FingerprintHash: "fp-new",
			CaptchaVerified: true,
			PoWCompleted:    true,
			PoWDurationMs:   20,
			RequestID:       "req_fast",
		},
	}, historySnapshot{})

	if result.Level != LevelMedium {
		t.Fatalf("Level = %q, want %q", result.Level, LevelMedium)
	}
	if !containsReason(result.Reasons, ReasonSuspiciousPoWTiming) {
		t.Fatalf("Reasons = %#v, want %q", result.Reasons, ReasonSuspiciousPoWTiming)
	}
	if !containsReason(result.Reasons, ReasonNewFingerprint) {
		t.Fatalf("Reasons = %#v, want %q", result.Reasons, ReasonNewFingerprint)
	}
	if result.RecommendedNextStep != RecommendationRequireStepUp {
		t.Fatalf("RecommendedNextStep = %q, want %q", result.RecommendedNextStep, RecommendationRequireStepUp)
	}
}

func TestEvaluatePostAuthRewardsTrustedPasskeyReauth(t *testing.T) {
	result := evaluate(Input{
		Stage:          StagePostAuth,
		UserID:         "user_123",
		AuthMethod:     "passkey",
		TrustedSession: true,
		Reauth:         true,
		Signals: Signals{
			VisitorID:       "fp-passkey",
			FingerprintHash: "fp-passkey",
			RequestID:       "req_abc",
		},
	}, historySnapshot{
		knownFingerprint: true,
	})

	if result.Level != LevelLow {
		t.Fatalf("Level = %q, want %q", result.Level, LevelLow)
	}
	if result.RecommendedNextStep != RecommendationAllowAndLog {
		t.Fatalf("RecommendedNextStep = %q, want %q", result.RecommendedNextStep, RecommendationAllowAndLog)
	}
	if !containsReason(result.Reasons, ReasonTrustedReauth) {
		t.Fatalf("Reasons = %#v, want %q", result.Reasons, ReasonTrustedReauth)
	}
	if !containsReason(result.Reasons, ReasonPasskeyAuth) {
		t.Fatalf("Reasons = %#v, want %q", result.Reasons, ReasonPasskeyAuth)
	}
}

func containsReason(reasons []Reason, want Reason) bool {
	for _, reason := range reasons {
		if reason == want {
			return true
		}
	}
	return false
}
