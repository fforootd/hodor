package api

import "testing"

func TestValidateSchemaAnnotations(t *testing.T) {
	tests := []struct {
		name   string
		json   string
		wantOK bool
	}{
		// ─── x-auth-methods (existing) ──────────────
		{
			name:   "valid x-auth-methods",
			json:   `{"x-auth-methods": {"password": {"enabled": true}}}`,
			wantOK: true,
		},
		{
			name:   "invalid auth method key",
			json:   `{"x-auth-methods": {"oauth2": {}}}`,
			wantOK: false,
		},

		// ─── x-captcha ──────────────────────────────
		{
			name:   "valid x-captcha (altcha)",
			json:   `{"x-captcha": {"provider": "altcha", "mode": "risk_based", "difficulty": 3}}`,
			wantOK: true,
		},
		{
			name:   "valid x-captcha (turnstile)",
			json:   `{"x-captcha": {"provider": "turnstile", "site_key": "test"}}`,
			wantOK: true,
		},
		{
			name:   "invalid captcha provider",
			json:   `{"x-captcha": {"provider": "friendlycaptcha"}}`,
			wantOK: false,
		},
		{
			name:   "invalid captcha mode",
			json:   `{"x-captcha": {"mode": "sometimes"}}`,
			wantOK: false,
		},
		{
			name:   "unknown captcha key",
			json:   `{"x-captcha": {"timeout": 5000}}`,
			wantOK: false,
		},
		{
			name:   "x-captcha not an object",
			json:   `{"x-captcha": "altcha"}`,
			wantOK: false,
		},

		// ─── x-fingerprint ──────────────────────────
		{
			name:   "valid x-fingerprint",
			json:   `{"x-fingerprint": {"enabled": true, "provider": "thumbmarkjs", "persist": true}}`,
			wantOK: true,
		},
		{
			name:   "valid x-fingerprint (built_in)",
			json:   `{"x-fingerprint": {"provider": "built_in"}}`,
			wantOK: true,
		},
		{
			name:   "invalid fingerprint provider",
			json:   `{"x-fingerprint": {"provider": "fingerprintjs"}}`,
			wantOK: false,
		},
		{
			name:   "unknown fingerprint key",
			json:   `{"x-fingerprint": {"canvas_hash": true}}`,
			wantOK: false,
		},

		// ─── x-rate-limit ───────────────────────────
		{
			name:   "valid x-rate-limit",
			json:   `{"x-rate-limit": {"max_attempts": 5, "window_seconds": 300, "lockout_seconds": 900, "scope": "ip"}}`,
			wantOK: true,
		},
		{
			name:   "valid x-rate-limit (fingerprint scope)",
			json:   `{"x-rate-limit": {"scope": "fingerprint"}}`,
			wantOK: true,
		},
		{
			name:   "invalid rate limit scope",
			json:   `{"x-rate-limit": {"scope": "session"}}`,
			wantOK: false,
		},
		{
			name:   "unknown rate limit key",
			json:   `{"x-rate-limit": {"burst": 10}}`,
			wantOK: false,
		},

		// ─── x-login-flow ───────────────────────────
		{
			name:   "valid x-login-flow",
			json:   `{"x-login-flow": {"flow_id": "lf_abc123", "inherit": true}}`,
			wantOK: true,
		},
		{
			name:   "valid x-login-flow (override)",
			json:   `{"x-login-flow": {"override": true}}`,
			wantOK: true,
		},
		{
			name:   "unknown login flow key",
			json:   `{"x-login-flow": {"template": "mfa_first"}}`,
			wantOK: false,
		},

		// ─── No annotations (valid) ─────────────────
		{
			name:   "no annotations",
			json:   `{"type": "object", "properties": {}}`,
			wantOK: true,
		},

		// ─── Multiple annotations (all valid) ───────
		{
			name: "all annotations valid",
			json: `{
				"x-auth-methods": {"password": {}},
				"x-captcha": {"provider": "altcha", "mode": "always"},
				"x-fingerprint": {"enabled": true},
				"x-rate-limit": {"max_attempts": 10, "scope": "ip"},
				"x-login-flow": {"flow_id": "lf_prod"}
			}`,
			wantOK: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := validateSchemaAnnotations([]byte(tt.json))
			if tt.wantOK && result != "" {
				t.Errorf("Expected valid, got error: %s", result)
			}
			if !tt.wantOK && result == "" {
				t.Errorf("Expected validation error, got none")
			}
		})
	}
}
