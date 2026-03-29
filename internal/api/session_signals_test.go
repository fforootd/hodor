package api

import "testing"

func TestComputeRiskLevel(t *testing.T) {
	tests := []struct {
		name     string
		signals  *ClientSignals
		expected string
	}{
		{
			name:     "nil signals",
			signals:  nil,
			expected: "unknown",
		},
		{
			name:     "empty signals",
			signals:  &ClientSignals{},
			expected: "high", // no positive signals = high risk
		},
		{
			name: "PoW only",
			signals: &ClientSignals{
				PoWCompleted:  true,
				PoWDurationMs: 500,
			},
			expected: "medium", // 0.3 + 0.1 = 0.4
		},
		{
			name: "full human signals",
			signals: &ClientSignals{
				PoWCompleted:    true,
				PoWDurationMs:   1500,
				CaptchaVerified: true,
				VisitorID:       "abc123def456",
				RequestID:       "00-abcdef-12345-01",
				DocumentLoadMs:  342,
			},
			expected: "low", // 0.3 + 0.1 + 0.2 + 0.2 + 0.1 + 0.1 = 1.0
		},
		{
			name: "fingerprint + trace only",
			signals: &ClientSignals{
				VisitorID:      "visitor-xyz",
				RequestID:      "00-request-id",
				DocumentLoadMs: 500,
			},
			expected: "medium", // 0.2 + 0.1 + 0.1 = 0.4
		},
		{
			name: "fast PoW (suspicious)",
			signals: &ClientSignals{
				PoWCompleted:  true,
				PoWDurationMs: 5, // too fast
			},
			expected: "high", // 0.3 only (no realistic timing bonus)
		},
		{
			name: "captcha verified + fingerprint",
			signals: &ClientSignals{
				CaptchaVerified: true,
				VisitorID:       "visitor-123",
				DocumentLoadMs:  800,
			},
			expected: "medium", // 0.2 + 0.2 + 0.1 = 0.5
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := computeRiskLevel(tt.signals)
			if result != tt.expected {
				t.Errorf("computeRiskLevel() = %q, want %q", result, tt.expected)
			}
		})
	}
}
