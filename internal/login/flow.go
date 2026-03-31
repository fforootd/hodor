// Package login Flow Engine — schema-driven login flow state machine.
//
// Reads per-field annotations (x-identifier, x-verify, x-recover, x-mfa)
// plus x-auth-methods from user schemas, and reads login UX annotations
// from login flow configs to determine step ordering and generate UI node trees.
//
// ADR-019: Server-Driven Login UI + Web Components
// The FlowStep + UINode contract is the SOLE interface between the
// server and client. The client is a dumb renderer.
package login

import (
	"encoding/json"
	"fmt"
	"sort"
	"sync"

	"github.com/zitadel/zitadel/internal/risk"
)

// ─── Schema Annotation Types ────────────────────────────────

// AuthFieldConfig represents the per-field auth annotations (x-identifier, x-verify, x-recover, x-mfa).
type AuthFieldConfig struct {
	Identifier   bool   `json:"identifier"`
	Verification string `json:"verification,omitempty"` // "email", "sms"
	Recovery     string `json:"recovery,omitempty"`     // "email", "sms"
	MFA          string `json:"mfa,omitempty"`          // "sms", "totp"
}

// AuthMethodEntry represents a single auth method in x-auth-methods.
type AuthMethodEntry struct {
	Enabled     bool `json:"enabled"`
	Interactive bool `json:"interactive"`
	Position    int  `json:"position,omitempty"`
	Preferred   bool `json:"preferred,omitempty"`
	MaxTokens   int  `json:"max_tokens,omitempty"` // for PAT/API key limits
}

// LoginConfig represents flow-owned login configuration.
type LoginConfig struct {
	Strategy            string `json:"strategy"` // "identifier_first", "passkey_first", "sso_only", "custom"
	MFARequired         bool   `json:"mfa_required"`
	RegistrationAllowed bool   `json:"registration_allowed"`
}

// ConsentItem represents a checkbox with optional links (terms, marketing opt-in, etc.).
type ConsentItem struct {
	ID       string `json:"id"`       // unique key, e.g. "terms", "marketing"
	Label    string `json:"label"`    // supports markdown links: "I agree to the [Terms](https://...)"
	Required bool   `json:"required"` // must be checked to proceed
}

// BrandingConfig represents flow-owned branding configuration.
type BrandingConfig struct {
	Heading     string            `json:"heading"`
	Description string            `json:"description"`
	LogoURL     string            `json:"logo_url"`
	OrgName     string            `json:"org_name"`
	Colors      map[string]string `json:"colors"`
	FontFamily  string            `json:"font_family"`
	FontURL     string            `json:"font_url"`
	Texts       map[string]string `json:"texts"`
	CustomCSS   string            `json:"custom_css"`
	HideZitadel bool              `json:"hide_zitadel_branding"`

	// Layout & visual presets
	Layout       string `json:"layout"`        // "centered" | "split" | "muted" | "card_image" | "minimal"
	DarkMode     string `json:"dark_mode"`     // "light" | "dark" | "auto"
	CoverImage   string `json:"cover_image"`   // URL for split/card_image layouts
	LogoDark     string `json:"logo_dark"`     // Alt logo for dark mode
	Favicon      string `json:"favicon"`       // Custom favicon URL
	BorderRadius string `json:"border_radius"` // "sm" | "md" | "lg" | "xl" | "full"

	// Legal/terms links
	TermsURL   string `json:"terms_url"`
	PrivacyURL string `json:"privacy_url"`

	// Social provider display
	SocialPosition string `json:"social_position"` // "top" | "bottom" (relative to email/password form)

	// Consent checkboxes (registration & login)
	Consent []ConsentItem `json:"consent"`
}

// CaptchaConfig represents x-captcha on the login flow schema.
type CaptchaConfig struct {
	Provider  string   `json:"provider"`             // "altcha" | "hcaptcha" | "recaptcha" | "turnstile"
	Mode      string   `json:"mode"`                 // "always" | "risk_based" | "never"
	On        []string `json:"on"`                   // ["register", "forgot_password", "login"]
	Steps     []string `json:"steps,omitempty"`      // legacy alias from console/runtime config
	Algorithm string   `json:"algorithm,omitempty"`  // "SHA-256" | "SHA-384" | "SHA-512" (Altcha PoW hash)
	MaxNumber int      `json:"max_number,omitempty"` // PoW difficulty range (default: 100000)
	SiteKey   string   `json:"site_key,omitempty"`   // for third-party providers
	SecretKey string   `json:"secret_key,omitempty"` // server-side only, never sent to client
	Threshold float64  `json:"threshold,omitempty"`  // score threshold for score-based providers
}

// FingerprintConfig represents x-fingerprint on the login flow schema.
type FingerprintConfig struct {
	Enabled  bool     `json:"enabled"`      // enable browser fingerprinting
	Provider string   `json:"provider"`     // "thumbmarkjs" (extensible)
	Persist  bool     `json:"persist"`      // persist visitor ID across sessions
	On       []string `json:"on,omitempty"` // ["login", "register"] — when to collect
	Steps    []string `json:"steps,omitempty"`
}

// RateLimitConfig represents x-rate-limit on the login flow schema.
type RateLimitConfig struct {
	MaxAttempts    int `json:"max_attempts"` // per IP per window
	WindowSeconds  int `json:"window_seconds"`
	LockoutSeconds int `json:"lockout_seconds"`
}

// LoginFlowRef links a login flow schema to a user schema.
type LoginFlowRef struct {
	UserSchema string `json:"user_schema"` // schema type identifier
	Version    string `json:"version"`     // semver constraint, e.g. ">=1"
}

type SSOProviderConfig struct {
	Mode string   `json:"mode"`
	IDs  []string `json:"ids"`
}

type SSOConfig struct {
	Providers SSOProviderConfig `json:"providers"`
}

// LoginFlowConfig is the fully extracted config from a login flow schema.
type LoginFlowConfig struct {
	Ref         LoginFlowRef       `json:"ref"`
	Login       LoginConfig        `json:"login"`
	Branding    BrandingConfig     `json:"branding"`
	SSO         SSOConfig          `json:"sso"`
	Captcha     *CaptchaConfig     `json:"captcha,omitempty"`
	Fingerprint *FingerprintConfig `json:"fingerprint,omitempty"`
	RateLimit   *RateLimitConfig   `json:"rate_limit,omitempty"`
}

// SchemaFieldDef represents a single field from a schema's properties block.
// Used by the registration step to generate input nodes from the schema.
type SchemaFieldDef struct {
	Name        string   `json:"name"`
	Type        string   `json:"type"`
	Format      string   `json:"format,omitempty"`
	Title       string   `json:"title,omitempty"`
	Description string   `json:"description,omitempty"`
	Required    bool     `json:"required"`
	Hidden      bool     `json:"hidden"`
	Sensitive   bool     `json:"sensitive"`
	Identifier  bool     `json:"identifier"`
	MinLength   int      `json:"min_length,omitempty"`
	MaxLength   int      `json:"max_length,omitempty"`
	Pattern     string   `json:"pattern,omitempty"`
	Enum        []string `json:"enum,omitempty"`
}

// SchemaAuthConfig is the fully extracted auth/login/branding config from a schema.
type SchemaAuthConfig struct {
	Identifiers            []string                    // field names that can be used as identifiers
	Fields                 map[string]AuthFieldConfig  // field name → auth config
	AuthMethods            map[string]*AuthMethodEntry // method name → config (from x-auth-methods)
	SchemaID               string
	SchemaType             string
	Login                  LoginConfig
	Branding               BrandingConfig
	SchemaProps            []SchemaFieldDef // all visible schema fields (for registration)
	RegistrationSchemaType string
	SSOProviderMode        string
	SSOProviderIDs         []string
	// Login flow specific (populated when a login flow schema is linked)
	LoginFlowID string             `json:"-"` // ID of the linked login flow schema
	Captcha     *CaptchaConfig     `json:"-"` // from login flow x-captcha
	Fingerprint *FingerprintConfig `json:"-"` // from login flow x-fingerprint
	RateLimit   *RateLimitConfig   `json:"-"` // from login flow x-rate-limit
}

// ─── Annotation Extraction ──────────────────────────────────

// ExtractAuthConfig parses per-field auth annotations and x-auth-methods from a user schema string.
func ExtractAuthConfig(schemaJSON string) *SchemaAuthConfig {
	var raw struct {
		Properties   map[string]map[string]any `json:"properties"`
		Required     []string                  `json:"required"`
		XAuthMethods json.RawMessage           `json:"x-auth-methods"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &raw); err != nil {
		return defaultConfig()
	}

	config := &SchemaAuthConfig{
		Fields:      make(map[string]AuthFieldConfig),
		AuthMethods: defaultAuthMethods(),
		Login:       defaultLoginConfig(),
		Branding:    defaultBrandingConfig(),
	}

	// Build required set for fast lookup.
	requiredSet := make(map[string]bool, len(raw.Required))
	for _, r := range raw.Required {
		requiredSet[r] = true
	}

	// Extract per-field auth annotations and schema field definitions.
	for name, def := range raw.Properties {
		extractFieldConfig(config, name, def, requiredSet)
	}

	// Extract x-auth-methods.
	if len(raw.XAuthMethods) > 0 {
		var methods map[string]*AuthMethodEntry
		if json.Unmarshal(raw.XAuthMethods, &methods) == nil && len(methods) > 0 {
			config.AuthMethods = methods
		}
	}

	// User schemas no longer own login/branding; keep runtime defaults here.
	config.Login = mergeLoginDefaults(config.Login)
	config.Branding = mergeBrandingDefaults(config.Branding)

	return config
}

// ExtractLoginFlowConfig parses a login flow schema JSON to extract
// x-login-flow, x-login, x-branding, x-captcha, x-fingerprint, x-rate-limit.
// This is the login flow schema counterpart to ExtractAuthConfig (which reads user schemas).
func ExtractLoginFlowConfig(flowSchemaJSON string) *LoginFlowConfig {
	var raw struct {
		XLoginFlow   json.RawMessage `json:"x-login-flow"`
		XLogin       json.RawMessage `json:"x-login"`
		Login        json.RawMessage `json:"login"`
		XBranding    json.RawMessage `json:"x-branding"`
		Branding     json.RawMessage `json:"branding"`
		SSO          json.RawMessage `json:"sso"`
		XCaptcha     json.RawMessage `json:"x-captcha"`
		Captcha      json.RawMessage `json:"captcha"`
		XFingerprint json.RawMessage `json:"x-fingerprint"`
		Fingerprint  json.RawMessage `json:"fingerprint"`
		XRateLimit   json.RawMessage `json:"x-rate-limit"`
		RateLimit    json.RawMessage `json:"rate_limit"`
	}
	if err := json.Unmarshal([]byte(flowSchemaJSON), &raw); err != nil {
		return &LoginFlowConfig{
			Login:    defaultLoginConfig(),
			Branding: defaultBrandingConfig(),
		}
	}

	cfg := &LoginFlowConfig{
		Login:    defaultLoginConfig(),
		Branding: defaultBrandingConfig(),
	}

	if len(raw.XLoginFlow) > 0 {
		_ = json.Unmarshal(raw.XLoginFlow, &cfg.Ref)
	}
	if source := firstNonEmptyRaw(raw.XLogin, raw.Login); len(source) > 0 {
		_ = json.Unmarshal(source, &cfg.Login)
	}
	if source := firstNonEmptyRaw(raw.XBranding, raw.Branding); len(source) > 0 {
		_ = json.Unmarshal(source, &cfg.Branding)
	}
	if len(raw.SSO) > 0 {
		_ = json.Unmarshal(raw.SSO, &cfg.SSO)
	}
	if source := firstNonEmptyRaw(raw.XCaptcha, raw.Captcha); len(source) > 0 {
		var cc CaptchaConfig
		if json.Unmarshal(source, &cc) == nil {
			if len(cc.On) == 0 && len(cc.Steps) > 0 {
				cc.On = normalizeFlowStepTargets(cc.Steps)
			} else {
				cc.On = normalizeFlowStepTargets(cc.On)
			}
			if cc.Algorithm == "" {
				cc.Algorithm = "SHA-256"
			}
			if cc.MaxNumber == 0 {
				cc.MaxNumber = 100000
			}
			cfg.Captcha = &cc
		}
	}
	if source := firstNonEmptyRaw(raw.XFingerprint, raw.Fingerprint); len(source) > 0 {
		var fp FingerprintConfig
		if json.Unmarshal(source, &fp) == nil {
			if len(fp.On) == 0 && len(fp.Steps) > 0 {
				fp.On = normalizeFlowStepTargets(fp.Steps)
			} else {
				fp.On = normalizeFlowStepTargets(fp.On)
			}
			if fp.Provider == "" {
				fp.Provider = "thumbmarkjs"
			}
			cfg.Fingerprint = &fp
		}
	}
	if source := firstNonEmptyRaw(raw.XRateLimit, raw.RateLimit); len(source) > 0 {
		var rl RateLimitConfig
		if json.Unmarshal(source, &rl) == nil {
			if rl.MaxAttempts == 0 {
				rl.MaxAttempts = 5
			}
			if rl.WindowSeconds == 0 {
				rl.WindowSeconds = 300
			}
			if rl.LockoutSeconds == 0 {
				rl.LockoutSeconds = 900
			}
			cfg.RateLimit = &rl
		}
	}

	cfg.Login = mergeLoginDefaults(cfg.Login)
	cfg.Branding = mergeBrandingDefaults(cfg.Branding)
	if cfg.SSO.Providers.Mode == "" {
		cfg.SSO.Providers.Mode = "all_enabled"
	}
	return cfg
}

func firstNonEmptyRaw(values ...json.RawMessage) json.RawMessage {
	for _, value := range values {
		if len(value) > 0 && string(value) != "null" {
			return value
		}
	}
	return nil
}

func normalizeFlowStepTargets(values []string) []string {
	if len(values) == 0 {
		return nil
	}
	seen := make(map[string]struct{}, len(values))
	out := make([]string, 0, len(values))
	for _, value := range values {
		normalized := value
		switch value {
		case "identifier", "password", "auth_select", "login":
			normalized = "login"
		case "register":
			normalized = "register"
		case "forgot_password":
			normalized = "forgot_password"
		}
		if normalized == "" {
			continue
		}
		if _, ok := seen[normalized]; ok {
			continue
		}
		seen[normalized] = struct{}{}
		out = append(out, normalized)
	}
	return out
}

func captchaScopeForStep(step StepType) string {
	switch step {
	case StepIdentifier, StepAuthSelect, StepPassword:
		return "login"
	case StepRegister:
		return "register"
	case StepForgotPassword:
		return "forgot_password"
	default:
		return ""
	}
}

// ResolveFlowConfig merges a user schema's field-level auth config with a
// login flow schema's UX config. The login flow's Login, Branding, Captcha,
// Fingerprint, and RateLimit override whatever was on the user schema.
func ResolveFlowConfig(userConfig *SchemaAuthConfig, flowConfig *LoginFlowConfig) *SchemaAuthConfig {
	if flowConfig == nil {
		return userConfig
	}

	// Clone the user config so we don't mutate the original.
	merged := *userConfig

	// Login flow takes precedence for UX-level settings.
	merged.Login = flowConfig.Login
	merged.Branding = flowConfig.Branding
	merged.Captcha = flowConfig.Captcha
	merged.Fingerprint = flowConfig.Fingerprint
	merged.RateLimit = flowConfig.RateLimit

	return &merged
}

// captchaActiveForStep returns true if the captcha should be shown on the given step.
func captchaActiveForStep(cc *CaptchaConfig, step StepType) bool {
	if cc == nil || cc.Provider == "" || cc.Provider == "none" {
		return false
	}
	if cc.Mode == "never" {
		return false
	}
	stepName := captchaScopeForStep(step)
	if stepName == "" {
		return false
	}
	if cc.Mode == "always" {
		return true
	}
	if len(cc.On) == 0 {
		return true
	}
	for _, s := range cc.On {
		if s == stepName {
			return true
		}
	}
	return false
}

// fingerprintActiveForStep returns true if fingerprinting should collect on the given step.
func fingerprintActiveForStep(fp *FingerprintConfig, step StepType) bool {
	if fp == nil || !fp.Enabled {
		return false
	}
	if len(fp.On) == 0 {
		return true // collect on all steps if no filter specified
	}
	var stepName string
	switch step {
	case StepIdentifier, StepAuthSelect, StepPassword:
		stepName = "login"
	case StepRegister:
		stepName = "register"
	default:
		return false
	}
	for _, s := range fp.On {
		if s == stepName {
			return true
		}
	}
	return false
}

// extractFieldConfig parses auth annotations and schema metadata from a single
// property definition and appends it to the config.
func extractFieldConfig(config *SchemaAuthConfig, name string, def map[string]any, requiredSet map[string]bool) {
	var fc AuthFieldConfig
	if v, ok := def["x-identifier"].(bool); ok {
		fc.Identifier = v
	}
	if v, ok := def["x-verify"].(string); ok {
		fc.Verification = v
	}
	if v, ok := def["x-recover"].(string); ok {
		fc.Recovery = v
	}
	if v, ok := def["x-mfa"].(string); ok {
		fc.MFA = v
	}
	if fc.Identifier || fc.Verification != "" || fc.Recovery != "" || fc.MFA != "" {
		config.Fields[name] = fc
		if fc.Identifier {
			config.Identifiers = append(config.Identifiers, name)
		}
	}

	sfd := buildSchemaFieldDef(name, def, requiredSet[name], fc.Identifier)
	if !sfd.Hidden {
		config.SchemaProps = append(config.SchemaProps, sfd)
	}
}

// buildSchemaFieldDef creates a SchemaFieldDef from a JSON schema property definition.
func buildSchemaFieldDef(name string, def map[string]any, required, identifier bool) SchemaFieldDef {
	sfd := SchemaFieldDef{
		Name:        name,
		Type:        stringOr(def, "type", "string"),
		Format:      stringOr(def, "format", ""),
		Title:       stringOr(def, "title", ""),
		Description: stringOr(def, "description", ""),
		Required:    required,
		Hidden:      boolOr(def, "x-hidden", false),
		Sensitive:   boolOr(def, "x-sensitive", false),
		Identifier:  identifier,
	}
	if v, ok := def["minLength"].(float64); ok {
		sfd.MinLength = int(v)
	}
	if v, ok := def["maxLength"].(float64); ok {
		sfd.MaxLength = int(v)
	}
	if v, ok := def["pattern"].(string); ok {
		sfd.Pattern = v
	}
	if v, ok := def["enum"].([]any); ok {
		for _, e := range v {
			if s, ok := e.(string); ok {
				sfd.Enum = append(sfd.Enum, s)
			}
		}
	}
	return sfd
}

func stringOr(m map[string]any, key, fallback string) string {
	if v, ok := m[key].(string); ok {
		return v
	}
	return fallback
}

func boolOr(m map[string]any, key string, fallback bool) bool {
	if v, ok := m[key].(bool); ok {
		return v
	}
	return fallback
}

func defaultConfig() *SchemaAuthConfig {
	return &SchemaAuthConfig{
		Identifiers: []string{"email"},
		Fields:      map[string]AuthFieldConfig{"email": {Identifier: true}},
		AuthMethods: defaultAuthMethods(),
		Login:       defaultLoginConfig(),
		Branding:    defaultBrandingConfig(),
	}
}

func defaultAuthMethods() map[string]*AuthMethodEntry {
	return map[string]*AuthMethodEntry{
		"password":    {Enabled: true, Interactive: true, Position: 1},
		"passkey":     {Enabled: false, Interactive: true, Position: 0},
		"magic_link":  {Enabled: true, Interactive: true, Position: 2},
		"sso":         {Enabled: true, Interactive: true, Position: 3},
		"pat":         {Enabled: false, Interactive: false},
		"api_key":     {Enabled: false, Interactive: false},
		"client_cert": {Enabled: false, Interactive: false},
	}
}

func defaultLoginConfig() LoginConfig {
	return LoginConfig{
		Strategy:            "identifier_first",
		MFARequired:         false,
		RegistrationAllowed: true,
	}
}

func defaultBrandingConfig() BrandingConfig {
	return BrandingConfig{
		Heading:     "Welcome back",
		Description: "Sign in to your account",
		OrgName:     "Zitadel",
		Colors: map[string]string{
			"primary":            "#6366f1",
			"primary_foreground": "#ffffff",
			"background":         "#f0f2ff",
			"surface":            "#ffffff",
			"text":               "#1a1a2e",
			"muted":              "#f4f4f5",
			"accent":             "#6366f1",
			"border":             "#e4e4e7",
			"error":              "#ef4444",
		},
		FontFamily:     "Inter, system-ui, sans-serif",
		Layout:         "centered",
		DarkMode:       "light",
		BorderRadius:   "md",
		SocialPosition: "bottom",
	}
}

func mergeLoginDefaults(lc LoginConfig) LoginConfig {
	if lc.Strategy == "" {
		lc.Strategy = "identifier_first"
	}
	return lc
}

func mergeBrandingDefaults(bc BrandingConfig) BrandingConfig {
	defaults := defaultBrandingConfig()
	if bc.Heading == "" {
		bc.Heading = defaults.Heading
	}
	if bc.Description == "" {
		bc.Description = defaults.Description
	}
	if bc.OrgName == "" {
		bc.OrgName = defaults.OrgName
	}
	if bc.FontFamily == "" {
		bc.FontFamily = defaults.FontFamily
	}
	if bc.Layout == "" {
		bc.Layout = defaults.Layout
	}
	if bc.DarkMode == "" {
		bc.DarkMode = defaults.DarkMode
	}
	if bc.BorderRadius == "" {
		bc.BorderRadius = defaults.BorderRadius
	}
	if bc.SocialPosition == "" {
		bc.SocialPosition = defaults.SocialPosition
	}
	if bc.Colors == nil {
		bc.Colors = defaults.Colors
	} else {
		for k, v := range defaults.Colors {
			if _, ok := bc.Colors[k]; !ok {
				bc.Colors[k] = v
			}
		}
	}
	return bc
}

// ResolveBranding merges branding from schema (base) with optional org and app overrides.
// Per ADR-009, settings cascade: instance → org → app.
func ResolveBranding(schema, org, app *BrandingConfig) BrandingConfig {
	base := defaultBrandingConfig()
	if schema != nil {
		base = mergeBrandingOver(base, *schema)
	}
	if org != nil {
		base = mergeBrandingOver(base, *org)
	}
	if app != nil {
		base = mergeBrandingOver(base, *app)
	}
	return base
}

// mergeBrandingOver overlays non-zero fields from overlay onto base.
func mergeBrandingOver(base, overlay BrandingConfig) BrandingConfig {
	if overlay.Heading != "" {
		base.Heading = overlay.Heading
	}
	if overlay.Description != "" {
		base.Description = overlay.Description
	}
	if overlay.LogoURL != "" {
		base.LogoURL = overlay.LogoURL
	}
	if overlay.OrgName != "" {
		base.OrgName = overlay.OrgName
	}
	if overlay.FontFamily != "" {
		base.FontFamily = overlay.FontFamily
	}
	if overlay.FontURL != "" {
		base.FontURL = overlay.FontURL
	}
	if overlay.Layout != "" {
		base.Layout = overlay.Layout
	}
	if overlay.DarkMode != "" {
		base.DarkMode = overlay.DarkMode
	}
	if overlay.CoverImage != "" {
		base.CoverImage = overlay.CoverImage
	}
	if overlay.LogoDark != "" {
		base.LogoDark = overlay.LogoDark
	}
	if overlay.Favicon != "" {
		base.Favicon = overlay.Favicon
	}
	if overlay.BorderRadius != "" {
		base.BorderRadius = overlay.BorderRadius
	}
	if overlay.TermsURL != "" {
		base.TermsURL = overlay.TermsURL
	}
	if overlay.PrivacyURL != "" {
		base.PrivacyURL = overlay.PrivacyURL
	}
	if overlay.SocialPosition != "" {
		base.SocialPosition = overlay.SocialPosition
	}
	if overlay.CustomCSS != "" {
		base.CustomCSS = overlay.CustomCSS
	}
	if overlay.HideZitadel {
		base.HideZitadel = true
	}
	if len(overlay.Consent) > 0 {
		base.Consent = overlay.Consent
	}
	if overlay.Texts != nil {
		if base.Texts == nil {
			base.Texts = map[string]string{}
		}
		for k, v := range overlay.Texts {
			base.Texts[k] = v
		}
	}
	if overlay.Colors != nil {
		if base.Colors == nil {
			base.Colors = map[string]string{}
		}
		for k, v := range overlay.Colors {
			base.Colors[k] = v
		}
	}
	return base
}

// ─── Flow State Machine ────────────────────────────────────

// StepType identifies a step in the login flow.
type StepType string

const (
	StepIdentifier     StepType = "identifier"
	StepSessionReuse   StepType = "session_reuse"
	StepAuthSelect     StepType = "auth_select"
	StepPassword       StepType = "password"
	StepPasskey        StepType = "passkey"
	StepMagicLink      StepType = "magic_link_sent"
	StepMFA            StepType = "mfa"
	StepRegister       StepType = "register"
	StepForgotPassword StepType = "forgot_password"
	StepResetPassword  StepType = "reset_password"
	StepComplete       StepType = "complete"
)

// UINode represents a single renderable element in the login UI.
// The client maps UINode.Type → DOM element. No business logic in the client.
type UINode struct {
	Type         string            `json:"type"`                 // "heading", "input", "submit", "button", "divider", "sso_button", "error", "group", "hidden", "registration_link", etc.
	Name         string            `json:"name,omitempty"`       // form field name
	InputType    string            `json:"input_type,omitempty"` // "text", "password", "email"
	Label        string            `json:"label,omitempty"`      // display label
	Text         string            `json:"text,omitempty"`       // heading/description text
	Placeholder  string            `json:"placeholder,omitempty"`
	Autocomplete string            `json:"autocomplete,omitempty"`
	Required     bool              `json:"required,omitempty"`
	Action       string            `json:"action,omitempty"` // "identifier", "password", "magic_link", "sso", "back", "register", "register_submit"
	ProviderID   string            `json:"provider_id,omitempty"`
	ProviderName string            `json:"provider_name,omitempty"`
	Template     string            `json:"template,omitempty"`   // SSO template (google, entraid, etc.)
	Initial      string            `json:"initial,omitempty"`    // avatar initial
	Value        string            `json:"value,omitempty"`      // pre-filled value
	Disabled     bool              `json:"disabled,omitempty"`   // disable input/button
	Errors       []string          `json:"errors,omitempty"`     // per-field validation errors
	Attributes   map[string]string `json:"attributes,omitempty"` // arbitrary HTML attrs
	Children     []UINode          `json:"children,omitempty"`   // nested nodes (e.g. form groups)
	MinLength    int               `json:"min_length,omitempty"`
	MaxLength    int               `json:"max_length,omitempty"`
	Pattern      string            `json:"pattern,omitempty"`
}

// FlowError represents a global error in the flow.
type FlowError struct {
	Code    string `json:"code"`
	Message string `json:"message"`
}

// FlowMessage represents an info/warning message in the flow.
type FlowMessage struct {
	Type string `json:"type"` // "info", "warning", "success"
	Text string `json:"text"`
}

// FlowStep is the current step response sent to the UI.
type FlowStep struct {
	FlowID          string         `json:"flow_id"`
	Step            StepType       `json:"step"`
	Nodes           []UINode       `json:"nodes"`
	Branding        BrandingConfig `json:"branding"`
	Identity        *FlowIdentity  `json:"identity,omitempty"`
	Errors          []FlowError    `json:"errors,omitempty"`
	Messages        []FlowMessage  `json:"messages,omitempty"`
	CSS             string         `json:"css,omitempty"`
	CaptchaRequired bool           `json:"captcha_required,omitempty"`
	CaptchaVerified bool           `json:"captcha_verified,omitempty"`
}

// FlowIdentity is the resolved identity info shown during auth steps.
type FlowIdentity struct {
	DisplayName   string `json:"display_name"`
	AvatarInitial string `json:"avatar_initial"`
}

type IdentityRevealMode string

const (
	IdentityRevealModeAnonymous IdentityRevealMode = "anonymous"
	IdentityRevealModeKnownUser IdentityRevealMode = "known_user"
)

// Flow holds the server-side state for an in-progress login.
type Flow struct {
	ID            string
	SchemaConfig  *SchemaAuthConfig
	CurrentStep   StepType
	IduserID      string
	Identifier    string
	DisplayName   string
	TrustedUserID string
	RevealMode    IdentityRevealMode
	Verified      bool
	SSOProviders  []map[string]any
	Errors        []FlowError       // accumulated errors for current step
	Messages      []FlowMessage     // accumulated messages for current step
	AuthRequestID string            // OIDC auth_request_id from the OP
	RedirectURI   string            // OIDC redirect_uri (stored from auth request)
	OIDCState     string            // OIDC state parameter
	OIDCPrompts   []string          // OIDC prompt values
	OIDCLoginHint string            // OIDC login_hint
	RegData       map[string]string // registration form data (accumulated)
	LoginFlowID   string            // if set, login flow schema was used
	AuthMethod    string

	// Client signal accumulation (populated during flow, sent to session on complete)
	CaptchaProvider      string  // "altcha", "hcaptcha", etc.
	CaptchaVerified      bool    // was captcha successfully verified?
	CaptchaVerifiedScope string  // logical scope the captcha was solved for
	CaptchaScore         float64 // score from score-based providers
	PoWCompleted         bool    // was PoW challenge solved?
	PoWDurationMs        float64 // how long the PoW solve took (client-reported)
	VisitorID            string  // ThumbmarkJS persistent fingerprint
	FingerprintHash      string  // composite component hash
	AdaptiveCaptcha      bool
	PreAuthRisk          *risk.Result
}

// FlowStore is an in-memory store for active login flows.
type FlowStore struct {
	mu    sync.RWMutex
	flows map[string]*Flow
}

// NewFlowStore creates a new flow store.
func NewFlowStore() *FlowStore {
	return &FlowStore{flows: make(map[string]*Flow)}
}

// Put stores a flow.
func (s *FlowStore) Put(f *Flow) {
	s.mu.Lock()
	defer s.mu.Unlock()
	s.flows[f.ID] = f
}

// Get retrieves a flow by ID.
func (s *FlowStore) Get(id string) (*Flow, bool) {
	s.mu.RLock()
	defer s.mu.RUnlock()
	f, ok := s.flows[id]
	return f, ok
}

// Delete removes a flow.
func (s *FlowStore) Delete(id string) {
	s.mu.Lock()
	defer s.mu.Unlock()
	delete(s.flows, id)
}

// ─── Node Builder ──────────────────────────────────────────

// BuildNodes generates the UI node tree for the current flow step.
func BuildNodes(flow *Flow) []UINode {
	cfg := flow.SchemaConfig
	texts := cfg.Branding.Texts
	if texts == nil {
		texts = map[string]string{}
	}

	var nodes []UINode
	switch flow.CurrentStep {
	case StepIdentifier:
		nodes = buildIdentifierNodes(flow, cfg, texts)
	case StepSessionReuse:
		nodes = buildSessionReuseNodes(flow, texts)
	case StepAuthSelect:
		nodes = buildAuthSelectNodes(flow, cfg, texts)
	case StepPassword:
		nodes = buildPasswordNodes(flow, texts)
	case StepMagicLink:
		nodes = buildMagicLinkSentNodes(flow, texts)
	case StepRegister:
		nodes = buildRegisterNodes(flow, cfg, texts)
	case StepForgotPassword:
		nodes = buildForgotPasswordNodes(flow, texts)
	case StepResetPassword:
		nodes = buildResetPasswordNodes(flow, texts)
	case StepComplete:
		return []UINode{
			{Type: "heading", Text: "Welcome!"},
			{Type: "description", Text: "Redirecting you now..."},
			{Type: "spinner"},
		}
	default:
		return []UINode{{Type: "heading", Text: "Unknown step"}}
	}

	// Append captcha node if this step's policy requires it.
	if flow.captchaRequiredForCurrentStep() {
		cc := cfg.Captcha
		attrs := map[string]string{}
		switch cc.Provider {
		case "altcha":
			attrs["algorithm"] = cc.Algorithm
			attrs["max-number"] = fmt.Sprintf("%d", cc.MaxNumber)
			if cc.Mode != "" {
				attrs["mode"] = cc.Mode
			}
			nodes = append(nodes, UINode{
				Type:       "captcha_altcha",
				Name:       "altcha_payload",
				Attributes: attrs,
			})
		default:
			// Generic captcha (hcaptcha, recaptcha, turnstile)
			attrs["provider"] = cc.Provider
			if cc.SiteKey != "" {
				attrs["site-key"] = cc.SiteKey
			}
			if cc.Mode != "" {
				attrs["mode"] = cc.Mode
			}
			nodes = append(nodes, UINode{
				Type:       "captcha_checkbox",
				Name:       "captcha_token",
				Attributes: attrs,
			})
		}
	}

	// Append fingerprint collector node if configured for this step.
	if fingerprintActiveForStep(cfg.Fingerprint, flow.CurrentStep) {
		fp := cfg.Fingerprint
		nodes = append(nodes, UINode{
			Type: "fingerprint_collect",
			Name: "visitor_id",
			Attributes: map[string]string{
				"provider": fp.Provider,
				"persist":  fmt.Sprintf("%v", fp.Persist),
			},
		})
	}

	return nodes
}

func buildIdentifierNodes(flow *Flow, cfg *SchemaAuthConfig, texts map[string]string) []UINode {
	label := textOr(texts, "identifier_label", "Email or username")
	placeholder := textOr(texts, "identifier_placeholder", "you@example.com")

	nodes := []UINode{}

	// If passkey_first, add passkey button before identifier.
	if cfg.Login.Strategy == "passkey_first" {
		if m := cfg.AuthMethods["passkey"]; m != nil && m.Enabled {
			nodes = append(nodes,
				UINode{Type: "heading", Text: cfg.Branding.Heading},
				UINode{Type: "description", Text: cfg.Branding.Description},
				UINode{Type: "button", Label: "🔑 Sign in with a passkey", Action: "passkey"},
				UINode{Type: "divider"},
			)
		}
	}

	// Social providers at top (if configured).
	if cfg.Branding.SocialPosition == "top" {
		if m := cfg.AuthMethods["sso"]; m != nil && m.Enabled && len(flow.SSOProviders) > 0 {
			children := make([]UINode, 0, len(flow.SSOProviders))
			for _, p := range flow.SSOProviders {
				children = append(children, UINode{
					Type:         "sso_button",
					ProviderID:   fmt.Sprintf("%v", p["id"]),
					ProviderName: fmt.Sprintf("%v", p["name"]),
					Template:     fmt.Sprintf("%v", p["template"]),
					Label:        fmt.Sprintf("Login with %v", p["name"]),
					Action:       "sso",
				})
			}
			nodes = append(nodes, UINode{Type: "social_group", Children: children})
			nodes = append(nodes, UINode{Type: "divider"})
		}
	}

	// Heading + description (only if not already emitted by passkey_first).
	if cfg.Login.Strategy != "passkey_first" {
		nodes = append(nodes,
			UINode{Type: "heading", Text: cfg.Branding.Heading},
			UINode{Type: "description", Text: cfg.Branding.Description},
		)
	}

	// Identifier input + submit.
	nodes = append(nodes,
		UINode{Type: "input", Name: "identifier", InputType: "text",
			Label: label, Placeholder: placeholder,
			Autocomplete: "username", Required: true,
			Value: flow.Identifier},
		UINode{Type: "submit", Label: textOr(texts, "continue_button", "Continue"), Action: "identifier"},
	)

	// Social providers at bottom (default).
	if cfg.Branding.SocialPosition == "bottom" {
		if m := cfg.AuthMethods["sso"]; m != nil && m.Enabled && len(flow.SSOProviders) > 0 {
			nodes = append(nodes, UINode{Type: "divider"})
			for _, p := range flow.SSOProviders {
				nodes = append(nodes, UINode{
					Type:         "sso_button",
					ProviderID:   fmt.Sprintf("%v", p["id"]),
					ProviderName: fmt.Sprintf("%v", p["name"]),
					Template:     fmt.Sprintf("%v", p["template"]),
					Label:        fmt.Sprintf("Login with %v", p["name"]),
					Action:       "sso",
				})
			}
		}
	}

	// Registration link if allowed.
	if cfg.Login.RegistrationAllowed {
		nodes = append(nodes,
			UINode{Type: "divider"},
			UINode{Type: "registration_link", Label: textOr(texts, "register_link", "Don't have an account? Create one"), Action: "register"},
		)
	}

	// Terms/privacy footer.
	if cfg.Branding.TermsURL != "" || cfg.Branding.PrivacyURL != "" {
		nodes = append(nodes, UINode{
			Type: "terms_footer",
			Attributes: map[string]string{
				"terms_url":   cfg.Branding.TermsURL,
				"privacy_url": cfg.Branding.PrivacyURL,
			},
		})
	}

	return nodes
}

func buildSessionReuseNodes(flow *Flow, texts map[string]string) []UINode {
	heading := textOr(texts, "session_reuse_heading", "Use your existing session?")
	description := textOr(texts, "session_reuse_description", "You're already signed in. Continue with that session or choose a different account.")
	primaryLabel := textOr(texts, "session_reuse_continue", "Continue with this session")
	switchAccountLabel := textOr(texts, "session_reuse_switch", "Use a different account")

	initial := ""
	if flow.DisplayName != "" {
		initial = string([]rune(flow.DisplayName)[0])
	} else if flow.Identifier != "" {
		initial = string([]rune(flow.Identifier)[0])
	}

	nodes := []UINode{
		{Type: "heading", Text: heading},
		{Type: "description", Text: description},
	}
	if flow.DisplayName != "" || flow.Identifier != "" {
		nodes = append(nodes, UINode{Type: "avatar", Initial: initial, Text: flow.authSelectHeading()})
	}
	nodes = append(nodes,
		UINode{Type: "button", Label: primaryLabel, Action: "use_session"},
		UINode{Type: "link", Label: switchAccountLabel, Action: "back"},
	)
	return nodes
}

func buildAuthSelectNodes(flow *Flow, cfg *SchemaAuthConfig, texts map[string]string) []UINode {
	heading := flow.authSelectHeading()
	initial := ""
	if flow.revealsKnownUserIdentity() {
		switch {
		case flow.DisplayName != "":
			initial = string([]rune(flow.DisplayName)[0])
		case flow.Identifier != "":
			initial = string([]rune(flow.Identifier)[0])
		}
	}

	nodes := []UINode{
		{Type: "heading", Text: heading},
		{Type: "description", Text: textOr(texts, "auth_select_description", "Choose how to sign in")},
	}
	if flow.revealsKnownUserIdentity() {
		nodes = append([]UINode{{Type: "avatar", Initial: initial, Text: flow.DisplayName}}, nodes...)
	}

	// Sort auth methods by Position for deterministic rendering order.
	type methodEntry struct {
		name  string
		entry *AuthMethodEntry
	}
	var sortedMethods []methodEntry
	for name, m := range cfg.AuthMethods {
		if m != nil && m.Enabled {
			sortedMethods = append(sortedMethods, methodEntry{name, m})
		}
	}
	sort.Slice(sortedMethods, func(i, j int) bool {
		return sortedMethods[i].entry.Position < sortedMethods[j].entry.Position
	})

	// Render each method in sorted order.
	hasPrimary := false
	hasAlternatives := false

	for _, me := range sortedMethods {
		switch me.name {
		case "password":
			hasPrimary = true
			nodes = append(nodes,
				UINode{Type: "input", Name: "password", InputType: "password",
					Label:       textOr(texts, "password_label", "Password"),
					Placeholder: "••••••••", Autocomplete: "current-password", Required: true},
			)
			// Password hint (forgot password).
			hasRecovery := false
			for _, fc := range cfg.Fields {
				if fc.Recovery != "" {
					hasRecovery = true
					break
				}
			}
			if hasRecovery {
				nodes = append(nodes, UINode{
					Type:   "password_hint",
					Label:  textOr(texts, "forgot_password", "Forgot your password?"),
					Action: "forgot_password",
				})
			}
			nodes = append(nodes,
				UINode{Type: "submit", Label: textOr(texts, "signin_button", "Sign in with password"), Action: "password"},
			)

		case "magic_link":
			if hasPrimary && !hasAlternatives {
				nodes = append(nodes, UINode{Type: "divider"})
				hasAlternatives = true
			}
			nodes = append(nodes, UINode{
				Type: "button", Label: textOr(texts, "magic_link_button", "✉ Send me a sign-in link"), Action: "magic_link",
			})

		case "passkey":
			if hasPrimary && !hasAlternatives {
				nodes = append(nodes, UINode{Type: "divider"})
				hasAlternatives = true
			}
			nodes = append(nodes, UINode{
				Type: "button", Label: "🔑 Use a passkey", Action: "passkey",
			})

		case "sso":
			for _, p := range flow.SSOProviders {
				if hasPrimary && !hasAlternatives {
					nodes = append(nodes, UINode{Type: "divider"})
					hasAlternatives = true
				}
				nodes = append(nodes, UINode{
					Type:         "sso_button",
					ProviderID:   fmt.Sprintf("%v", p["id"]),
					ProviderName: fmt.Sprintf("%v", p["name"]),
					Template:     fmt.Sprintf("%v", p["template"]),
					Label:        fmt.Sprintf("Continue with %v", p["name"]),
					Action:       "sso",
				})
			}
		}
	}

	// Back link.
	nodes = append(nodes, UINode{
		Type: "link", Label: textOr(texts, "back_link", "← Use a different account"), Action: "back",
	})

	return nodes
}

func buildPasswordNodes(flow *Flow, texts map[string]string) []UINode {
	// Same as auth_select but focused on password only (used when identifier_first + only password).
	return buildAuthSelectNodes(flow, flow.SchemaConfig, texts)
}

func buildMagicLinkSentNodes(flow *Flow, texts map[string]string) []UINode {
	return []UINode{
		{Type: "icon", Text: "✉"},
		{Type: "heading", Text: "Check your email"},
		{Type: "description", Text: fmt.Sprintf("We sent a sign-in link to %s", flow.Identifier)},
		{Type: "info", Text: "Click the link in your email to sign in. The link expires in 15 minutes."},
		{Type: "button", Label: textOr(texts, "resend_button", "Resend link"), Action: "resend_magic_link"},
		{Type: "link", Label: textOr(texts, "back_link", "← Back to sign in"), Action: "back"},
	}
}

func buildForgotPasswordNodes(flow *Flow, texts map[string]string) []UINode {
	return []UINode{
		{Type: "icon", Text: "🔒"},
		{Type: "heading", Text: textOr(texts, "forgot_heading", "Reset your password")},
		{Type: "description", Text: textOr(texts, "forgot_description", "We'll send a password reset link to your email.")},
		{Type: "info", Text: fmt.Sprintf("A reset link will be sent to %s", flow.Identifier)},
		{Type: "submit", Label: textOr(texts, "send_reset_button", "Send reset link"), Action: "send_reset"},
		{Type: "link", Label: textOr(texts, "back_link", "← Back to sign in"), Action: "back"},
	}
}

func buildResetPasswordNodes(flow *Flow, texts map[string]string) []UINode {
	return []UINode{
		{Type: "icon", Text: "🔒"},
		{Type: "heading", Text: textOr(texts, "reset_heading", "Set a new password")},
		{Type: "input", Name: "new_password", InputType: "password",
			Label:       textOr(texts, "new_password_label", "New password"),
			Placeholder: "••••••••", Required: true, Autocomplete: "new-password"},
		{Type: "input", Name: "confirm_password", InputType: "password",
			Label:       textOr(texts, "confirm_password_label", "Confirm password"),
			Placeholder: "••••••••", Required: true, Autocomplete: "new-password"},
		{Type: "submit", Label: textOr(texts, "reset_button", "Reset password"), Action: "reset_password"},
	}
}

// buildRegisterNodes generates registration form nodes from schema field definitions.
func buildRegisterNodes(flow *Flow, cfg *SchemaAuthConfig, texts map[string]string) []UINode {
	nodes := make([]UINode, 0, 2+len(cfg.SchemaProps)*2+len(cfg.Branding.Consent)+2)
	nodes = append(nodes,
		UINode{Type: "heading", Text: textOr(texts, "register_heading", "Create your account")},
		UINode{Type: "description", Text: textOr(texts, "register_description", "Enter your details to get started")},
	)

	// Generate an input node per visible schema field.
	for _, field := range cfg.SchemaProps {
		inputType := fieldInputType(field)
		label := field.Title
		if label == "" {
			label = humanize(field.Name)
		}
		placeholder := ""
		if field.Format == "email" {
			placeholder = "you@example.com"
		}

		node := UINode{
			Type:        "input",
			Name:        field.Name,
			InputType:   inputType,
			Label:       label,
			Placeholder: placeholder,
			Required:    field.Required,
			MinLength:   field.MinLength,
			MaxLength:   field.MaxLength,
			Pattern:     field.Pattern,
		}

		// Pre-fill from accumulated reg data.
		if flow.RegData != nil {
			if v, ok := flow.RegData[field.Name]; ok {
				node.Value = v
			}
		}

		// Auto-fill identifier from the identifier step.
		if field.Identifier && node.Value == "" && flow.Identifier != "" {
			node.Value = flow.Identifier
		}

		if field.Description != "" {
			node.Attributes = map[string]string{"data-description": field.Description}
		}

		nodes = append(nodes, node)

		// Emit field_description node after the input for helper text.
		if field.Description != "" {
			nodes = append(nodes, UINode{
				Type: "field_description",
				Text: field.Description,
				Name: field.Name,
			})
		}
	}

	// Consent checkboxes (from x-branding).
	for _, consent := range cfg.Branding.Consent {
		nodes = append(nodes, UINode{
			Type:     "consent_checkbox",
			Name:     "consent_" + consent.ID,
			Label:    consent.Label,
			Required: consent.Required,
		})
	}

	nodes = append(nodes,
		UINode{Type: "submit", Label: textOr(texts, "register_button", "Create account"), Action: "register_submit"},
		UINode{Type: "link", Label: textOr(texts, "register_back_link", "← Already have an account? Sign in"), Action: "back"},
	)

	return nodes
}

// fieldInputType maps a SchemaFieldDef to an HTML input type.
func fieldInputType(f SchemaFieldDef) string {
	if f.Sensitive {
		return "password"
	}
	switch f.Format {
	case "email":
		return "email"
	case "uri":
		return "url"
	case "date":
		return "date"
	default:
		switch f.Type {
		case "integer", "number":
			return "number"
		case "boolean":
			return "checkbox"
		default:
			return "text"
		}
	}
}

// humanize converts "display_name" → "Display Name".
func humanize(s string) string {
	result := make([]byte, 0, len(s))
	upper := true
	for i := 0; i < len(s); i++ {
		c := s[i]
		if c == '_' || c == '-' {
			result = append(result, ' ')
			upper = true
			continue
		}
		if upper && c >= 'a' && c <= 'z' {
			c -= 32 // ASCII lowercase to uppercase
		}
		result = append(result, c)
		upper = false
	}
	return string(result)
}

func textOr(texts map[string]string, key, fallback string) string {
	if v, ok := texts[key]; ok && v != "" {
		return v
	}
	return fallback
}

func (f *Flow) revealsKnownUserIdentity() bool {
	return f.RevealMode == IdentityRevealModeKnownUser
}

func (f *Flow) authSelectHeading() string {
	if f.revealsKnownUserIdentity() && f.DisplayName != "" {
		return f.DisplayName
	}
	return f.Identifier
}

func (f *Flow) currentCaptchaScope() string {
	return captchaScopeForStep(f.CurrentStep)
}

func (f *Flow) captchaVerifiedForCurrentStep() bool {
	scope := f.currentCaptchaScope()
	return scope != "" && f.CaptchaVerified && f.CaptchaVerifiedScope == scope
}

func (f *Flow) captchaRequiredForCurrentStep() bool {
	if f == nil || f.SchemaConfig == nil {
		return false
	}
	if !captchaActiveForStep(f.SchemaConfig.Captcha, f.CurrentStep) {
		return false
	}
	if f.SchemaConfig.Captcha != nil && f.SchemaConfig.Captcha.Mode == "risk_based" {
		if f.PreAuthRisk == nil {
			return true
		}
		return f.AdaptiveCaptcha
	}
	return true
}

func (f *Flow) clearCaptchaState() {
	f.CaptchaProvider = ""
	f.CaptchaVerified = false
	f.CaptchaVerifiedScope = ""
	f.CaptchaScore = 0
	f.PoWCompleted = false
	f.PoWDurationMs = 0
}

func (f *Flow) transitionToStep(step StepType) {
	currentScope := f.currentCaptchaScope()
	nextScope := captchaScopeForStep(step)
	if currentScope != "" && nextScope != currentScope {
		f.clearCaptchaState()
	}
	f.AdaptiveCaptcha = false
	f.PreAuthRisk = nil
	f.CurrentStep = step
}

// ToFlowStep converts flow state to the response sent to the UI.
func (f *Flow) ToFlowStep() *FlowStep {
	step := &FlowStep{
		FlowID:          f.ID,
		Step:            f.CurrentStep,
		Nodes:           BuildNodes(f),
		Branding:        f.SchemaConfig.Branding,
		Errors:          f.Errors,
		Messages:        f.Messages,
		CSS:             f.SchemaConfig.Branding.CustomCSS,
		CaptchaRequired: f.captchaRequiredForCurrentStep(),
		CaptchaVerified: f.captchaVerifiedForCurrentStep(),
	}
	if f.revealsKnownUserIdentity() && f.DisplayName != "" {
		initial := string([]rune(f.DisplayName)[0])
		step.Identity = &FlowIdentity{
			DisplayName:   f.DisplayName,
			AvatarInitial: initial,
		}
	}

	// Clear transient errors/messages after rendering.
	f.Errors = nil
	f.Messages = nil

	return step
}
