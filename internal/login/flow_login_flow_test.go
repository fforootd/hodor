package login

import (
	"testing"
)

func TestExtractLoginFlowConfig(t *testing.T) {
	schema := `{
		"$schema": "https://zitadel.com/schemas/v1/login-flow",
		"type": "object",
		"x-login-flow": {
			"user_schema": "human_user",
			"version": ">=1"
		},
		"x-login": {
			"strategy": "passkey_first",
			"mfa_required": true,
			"registration_allowed": false
		},
		"x-branding": {
			"heading": "Acme Login",
			"layout": "split"
		},
		"x-captcha": {
			"provider": "altcha",
			"mode": "invisible",
			"on": ["register", "forgot_password"],
			"algorithm": "SHA-384",
			"max_number": 200000
		},
		"x-fingerprint": {
			"enabled": true,
			"provider": "thumbmarkjs",
			"persist": true,
			"on": ["login", "register"]
		},
		"x-rate-limit": {
			"max_attempts": 10,
			"window_seconds": 600,
			"lockout_seconds": 1800
		}
	}`

	cfg := ExtractLoginFlowConfig(schema)

	// x-login-flow
	if cfg.Ref.UserSchema != "human_user" {
		t.Errorf("Ref.UserSchema = %q, want human_user", cfg.Ref.UserSchema)
	}
	if cfg.Ref.Version != ">=1" {
		t.Errorf("Ref.Version = %q, want >=1", cfg.Ref.Version)
	}

	// x-login
	if cfg.Login.Strategy != "passkey_first" {
		t.Errorf("Login.Strategy = %q, want passkey_first", cfg.Login.Strategy)
	}
	if !cfg.Login.MFARequired {
		t.Error("Login.MFARequired = false, want true")
	}
	if cfg.Login.RegistrationAllowed {
		t.Error("Login.RegistrationAllowed = true, want false")
	}

	// x-branding
	if cfg.Branding.Heading != "Acme Login" {
		t.Errorf("Branding.Heading = %q, want Acme Login", cfg.Branding.Heading)
	}
	if cfg.Branding.Layout != "split" {
		t.Errorf("Branding.Layout = %q, want split", cfg.Branding.Layout)
	}

	// x-captcha
	if cfg.Captcha == nil {
		t.Fatal("Captcha is nil")
	}
	if cfg.Captcha.Provider != "altcha" {
		t.Errorf("Captcha.Provider = %q, want altcha", cfg.Captcha.Provider)
	}
	if cfg.Captcha.Mode != "invisible" {
		t.Errorf("Captcha.Mode = %q, want invisible", cfg.Captcha.Mode)
	}
	if len(cfg.Captcha.On) != 2 {
		t.Fatalf("Captcha.On len = %d, want 2", len(cfg.Captcha.On))
	}
	if cfg.Captcha.Algorithm != "SHA-384" {
		t.Errorf("Captcha.Algorithm = %q, want SHA-384", cfg.Captcha.Algorithm)
	}
	if cfg.Captcha.MaxNumber != 200000 {
		t.Errorf("Captcha.MaxNumber = %d, want 200000", cfg.Captcha.MaxNumber)
	}

	// x-fingerprint
	if cfg.Fingerprint == nil {
		t.Fatal("Fingerprint is nil")
	}
	if !cfg.Fingerprint.Enabled {
		t.Error("Fingerprint.Enabled = false, want true")
	}
	if cfg.Fingerprint.Provider != "thumbmarkjs" {
		t.Errorf("Fingerprint.Provider = %q, want thumbmarkjs", cfg.Fingerprint.Provider)
	}
	if !cfg.Fingerprint.Persist {
		t.Error("Fingerprint.Persist = false, want true")
	}
	if len(cfg.Fingerprint.On) != 2 {
		t.Fatalf("Fingerprint.On len = %d, want 2", len(cfg.Fingerprint.On))
	}

	// x-rate-limit
	if cfg.RateLimit == nil {
		t.Fatal("RateLimit is nil")
	}
	if cfg.RateLimit.MaxAttempts != 10 {
		t.Errorf("RateLimit.MaxAttempts = %d, want 10", cfg.RateLimit.MaxAttempts)
	}
	if cfg.RateLimit.WindowSeconds != 600 {
		t.Errorf("RateLimit.WindowSeconds = %d, want 600", cfg.RateLimit.WindowSeconds)
	}
	if cfg.RateLimit.LockoutSeconds != 1800 {
		t.Errorf("RateLimit.LockoutSeconds = %d, want 1800", cfg.RateLimit.LockoutSeconds)
	}
}

func TestExtractLoginFlowConfig_Defaults(t *testing.T) {
	// Minimal schema with captcha but no algorithm/max_number.
	schema := `{
		"x-captcha": { "provider": "altcha", "on": ["login"] },
		"x-fingerprint": { "enabled": true },
		"x-rate-limit": {}
	}`

	cfg := ExtractLoginFlowConfig(schema)

	// Captcha defaults.
	if cfg.Captcha.Algorithm != "SHA-256" {
		t.Errorf("default Algorithm = %q, want SHA-256", cfg.Captcha.Algorithm)
	}
	if cfg.Captcha.MaxNumber != 100000 {
		t.Errorf("default MaxNumber = %d, want 100000", cfg.Captcha.MaxNumber)
	}

	// Fingerprint defaults.
	if cfg.Fingerprint.Provider != "thumbmarkjs" {
		t.Errorf("default Provider = %q, want thumbmarkjs", cfg.Fingerprint.Provider)
	}

	// Rate limit defaults.
	if cfg.RateLimit.MaxAttempts != 5 {
		t.Errorf("default MaxAttempts = %d, want 5", cfg.RateLimit.MaxAttempts)
	}
	if cfg.RateLimit.WindowSeconds != 300 {
		t.Errorf("default WindowSeconds = %d, want 300", cfg.RateLimit.WindowSeconds)
	}
	if cfg.RateLimit.LockoutSeconds != 900 {
		t.Errorf("default LockoutSeconds = %d, want 900", cfg.RateLimit.LockoutSeconds)
	}
}

func TestExtractLoginFlowConfig_RuntimeConfigShape(t *testing.T) {
	schema := `{
		"strategy": "identifier_first",
		"branding": {
			"heading": "Runtime Login",
			"layout": "split"
		},
		"captcha": {
			"provider": "altcha",
			"mode": "risk_based"
		},
		"fingerprint": {
			"enabled": true
		},
		"rate_limit": {
			"max_attempts": 8
		}
	}`

	cfg := ExtractLoginFlowConfig(schema)

	if cfg.Login.Strategy != "identifier_first" {
		t.Errorf("Login.Strategy = %q, want identifier_first", cfg.Login.Strategy)
	}
	if cfg.Branding.Heading != "Runtime Login" {
		t.Errorf("Branding.Heading = %q, want Runtime Login", cfg.Branding.Heading)
	}
	if cfg.Branding.Layout != "split" {
		t.Errorf("Branding.Layout = %q, want split", cfg.Branding.Layout)
	}
	if cfg.Captcha == nil || cfg.Captcha.Provider != "altcha" {
		t.Fatalf("Captcha = %#v, want provider altcha", cfg.Captcha)
	}
	if cfg.Fingerprint == nil || !cfg.Fingerprint.Enabled {
		t.Fatalf("Fingerprint = %#v, want enabled true", cfg.Fingerprint)
	}
	if cfg.RateLimit == nil || cfg.RateLimit.MaxAttempts != 8 {
		t.Fatalf("RateLimit = %#v, want max_attempts 8", cfg.RateLimit)
	}
}

func TestExtractLoginFlowConfig_InvalidJSON(t *testing.T) {
	cfg := ExtractLoginFlowConfig("not valid json")

	// Should return defaults, not panic.
	if cfg.Login.Strategy != "identifier_first" {
		t.Errorf("default Strategy = %q, want identifier_first", cfg.Login.Strategy)
	}
	if cfg.Captcha != nil {
		t.Error("Captcha should be nil for invalid JSON")
	}
}

func TestResolveFlowConfig(t *testing.T) {
	userConfig := &SchemaAuthConfig{
		Identifiers: []string{"email"},
		Fields:      map[string]AuthFieldConfig{"email": {Identifier: true}},
		AuthMethods: defaultAuthMethods(),
		Login:       defaultLoginConfig(),
		Branding:    defaultBrandingConfig(),
	}

	flowConfig := &LoginFlowConfig{
		Ref:         LoginFlowRef{UserSchema: "human_user", Version: ">=1"},
		Login:       LoginConfig{Strategy: "passkey_first", MFARequired: true},
		Branding:    BrandingConfig{Heading: "Custom Login", Layout: "split"},
		Captcha:     &CaptchaConfig{Provider: "altcha", On: []string{"login"}},
		Fingerprint: &FingerprintConfig{Enabled: true, Provider: "thumbmarkjs"},
	}

	merged := ResolveFlowConfig(userConfig, flowConfig)

	// User-level fields should be preserved.
	if len(merged.Identifiers) != 1 || merged.Identifiers[0] != "email" {
		t.Error("User identifiers should be preserved")
	}
	if len(merged.AuthMethods) == 0 {
		t.Error("User auth methods should be preserved")
	}

	// Flow-level UX should override.
	if merged.Login.Strategy != "passkey_first" {
		t.Errorf("Login.Strategy = %q, want passkey_first (from flow)", merged.Login.Strategy)
	}
	if merged.Branding.Heading != "Custom Login" {
		t.Errorf("Branding.Heading = %q, want Custom Login (from flow)", merged.Branding.Heading)
	}
	if merged.Captcha == nil || merged.Captcha.Provider != "altcha" {
		t.Error("Captcha should come from flow config")
	}
	if merged.Fingerprint == nil || !merged.Fingerprint.Enabled {
		t.Error("Fingerprint should come from flow config")
	}
}

func TestResolveFlowConfig_NilFlow(t *testing.T) {
	userConfig := &SchemaAuthConfig{
		Login: defaultLoginConfig(),
	}

	merged := ResolveFlowConfig(userConfig, nil)
	if merged != userConfig {
		t.Error("ResolveFlowConfig with nil flow should return user config unchanged")
	}
}

func TestCaptchaActiveForStep(t *testing.T) {
	cc := &CaptchaConfig{On: []string{"login", "register"}}

	tests := []struct {
		step     StepType
		expected bool
	}{
		{StepIdentifier, true},
		{StepPassword, true},
		{StepRegister, true},
		{StepMagicLink, false},
		{StepComplete, false},
	}

	for _, tt := range tests {
		if got := captchaActiveForStep(cc, tt.step); got != tt.expected {
			t.Errorf("captchaActiveForStep(%q) = %v, want %v", tt.step, got, tt.expected)
		}
	}

	// Nil config.
	if captchaActiveForStep(nil, StepIdentifier) {
		t.Error("nil config should return false")
	}
}

func TestFingerprintActiveForStep(t *testing.T) {
	fp := &FingerprintConfig{Enabled: true, On: []string{"login"}}

	if !fingerprintActiveForStep(fp, StepIdentifier) {
		t.Error("Expected true for login step")
	}
	if fingerprintActiveForStep(fp, StepRegister) {
		t.Error("Expected false for register step (not in On list)")
	}

	// Empty On = collect on all steps.
	fpAll := &FingerprintConfig{Enabled: true, On: []string{}}
	if !fingerprintActiveForStep(fpAll, StepRegister) {
		t.Error("Empty On should collect on all steps")
	}

	// Disabled.
	fpOff := &FingerprintConfig{Enabled: false, On: []string{"login"}}
	if fingerprintActiveForStep(fpOff, StepIdentifier) {
		t.Error("Disabled fingerprint should return false")
	}
}
