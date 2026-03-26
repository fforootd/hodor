// Package login Flow Engine — schema-driven login flow state machine.
//
// Reads x-auth (per-field), x-login (schema-level), and x-branding
// annotations from identity schemas to determine step ordering
// and generate UI node trees.
package login

import (
	"encoding/json"
	"fmt"
	"sync"
)

// ─── Schema Annotation Types ────────────────────────────────

// AuthFieldConfig represents the x-auth annotation on a schema property.
type AuthFieldConfig struct {
	Identifier   bool   `json:"identifier"`
	Verification string `json:"verification,omitempty"` // "email", "sms"
	Recovery     string `json:"recovery,omitempty"`     // "email", "sms"
	MFA          string `json:"mfa,omitempty"`          // "sms", "totp"
}

// AuthMethodConfig represents a single auth method inside x-login.
type AuthMethodConfig struct {
	Enabled   bool `json:"enabled"`
	Position  int  `json:"position"`
	Preferred bool `json:"preferred,omitempty"`
}

// LoginConfig represents the x-login schema-level annotation.
type LoginConfig struct {
	Preset              string                       `json:"preset"` // "identifier_first", "passkey_first", "sso_only", "custom"
	AuthMethods         map[string]*AuthMethodConfig `json:"auth_methods"`
	MFARequired         bool                         `json:"mfa_required"`
	RegistrationAllowed bool                         `json:"registration_allowed"`
}

// BrandingConfig represents the x-branding schema-level annotation.
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
}

// SchemaAuthConfig is the fully extracted auth/login/branding config from a schema.
type SchemaAuthConfig struct {
	Identifiers []string                   // field names that can be used as identifiers
	Fields      map[string]AuthFieldConfig // field name → auth config
	Login       LoginConfig
	Branding    BrandingConfig
}

// ─── Annotation Extraction ──────────────────────────────────

// ExtractAuthConfig parses x-auth, x-login, and x-branding from a JSON schema string.
func ExtractAuthConfig(schemaJSON string) *SchemaAuthConfig {
	var raw struct {
		Properties map[string]map[string]any `json:"properties"`
		XLogin     json.RawMessage           `json:"x-login"`
		XBranding  json.RawMessage           `json:"x-branding"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &raw); err != nil {
		return defaultConfig()
	}

	config := &SchemaAuthConfig{
		Fields:   make(map[string]AuthFieldConfig),
		Login:    defaultLoginConfig(),
		Branding: defaultBrandingConfig(),
	}

	// Extract per-field x-auth annotations.
	for name, def := range raw.Properties {
		xAuth, ok := def["x-auth"]
		if !ok {
			continue
		}
		// x-auth can be a map[string]any from JSON unmarshalling.
		b, err := json.Marshal(xAuth)
		if err != nil {
			continue
		}
		var fc AuthFieldConfig
		if json.Unmarshal(b, &fc) == nil {
			config.Fields[name] = fc
			if fc.Identifier {
				config.Identifiers = append(config.Identifiers, name)
			}
		}
	}

	// Extract schema-level x-login.
	if len(raw.XLogin) > 0 {
		_ = json.Unmarshal(raw.XLogin, &config.Login)
	}

	// Extract schema-level x-branding.
	if len(raw.XBranding) > 0 {
		_ = json.Unmarshal(raw.XBranding, &config.Branding)
	}

	// Apply defaults for missing fields.
	config.Login = mergeLoginDefaults(config.Login)
	config.Branding = mergeBrandingDefaults(config.Branding)

	return config
}

func defaultConfig() *SchemaAuthConfig {
	return &SchemaAuthConfig{
		Identifiers: []string{"email"},
		Fields:      map[string]AuthFieldConfig{"email": {Identifier: true}},
		Login:       defaultLoginConfig(),
		Branding:    defaultBrandingConfig(),
	}
}

func defaultLoginConfig() LoginConfig {
	return LoginConfig{
		Preset: "identifier_first",
		AuthMethods: map[string]*AuthMethodConfig{
			"password":   {Enabled: true, Position: 1},
			"passkey":    {Enabled: false, Position: 0},
			"magic_link": {Enabled: true, Position: 2},
			"sso":        {Enabled: true, Position: 3},
		},
		MFARequired:         false,
		RegistrationAllowed: true,
	}
}

func defaultBrandingConfig() BrandingConfig {
	return BrandingConfig{
		Heading:     "Welcome back",
		Description: "Sign in to your account",
		OrgName:     "ZITADEL",
		Colors: map[string]string{
			"primary":    "#6366f1",
			"background": "#f0f2ff",
			"surface":    "#ffffff",
			"text":       "#1a1a2e",
			"error":      "#ef4444",
		},
		FontFamily: "Inter, system-ui, sans-serif",
	}
}

func mergeLoginDefaults(lc LoginConfig) LoginConfig {
	if lc.Preset == "" {
		lc.Preset = "identifier_first"
	}
	if lc.AuthMethods == nil {
		lc.AuthMethods = defaultLoginConfig().AuthMethods
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

// ─── Flow State Machine ────────────────────────────────────

// StepType identifies a step in the login flow.
type StepType string

const (
	StepIdentifier StepType = "identifier"
	StepAuthSelect StepType = "auth_select"
	StepPassword   StepType = "password"
	StepPasskey    StepType = "passkey"
	StepMagicLink  StepType = "magic_link_sent"
	StepMFA        StepType = "mfa"
	StepComplete   StepType = "complete"
)

// UINode represents a single renderable element in the login UI.
type UINode struct {
	Type         string `json:"type"`                 // "heading", "input", "submit", "button", "divider", "sso_button", etc.
	Name         string `json:"name,omitempty"`       // form field name
	InputType    string `json:"input_type,omitempty"` // "text", "password", "email"
	Label        string `json:"label,omitempty"`      // display label
	Text         string `json:"text,omitempty"`       // heading/description text
	Placeholder  string `json:"placeholder,omitempty"`
	Autocomplete string `json:"autocomplete,omitempty"`
	Required     bool   `json:"required,omitempty"`
	Action       string `json:"action,omitempty"` // "identifier", "password", "magic_link", "sso", "back"
	ProviderID   string `json:"provider_id,omitempty"`
	ProviderName string `json:"provider_name,omitempty"`
	Template     string `json:"template,omitempty"` // SSO template (google, entraid, etc.)
	Initial      string `json:"initial,omitempty"`  // avatar initial
}

// FlowStep is the current step response sent to the UI.
type FlowStep struct {
	FlowID   string         `json:"flow_id"`
	Step     StepType       `json:"step"`
	Nodes    []UINode       `json:"nodes"`
	Branding BrandingConfig `json:"branding"`
	Identity *FlowIdentity  `json:"identity,omitempty"`
}

// FlowIdentity is the resolved identity info shown during auth steps.
type FlowIdentity struct {
	DisplayName   string `json:"display_name"`
	AvatarInitial string `json:"avatar_initial"`
}

// Flow holds the server-side state for an in-progress login.
type Flow struct {
	ID           string
	SchemaConfig *SchemaAuthConfig
	CurrentStep  StepType
	IdentityID   int64
	Identifier   string
	DisplayName  string
	Verified     bool
	SSOProviders []map[string]any
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

	switch flow.CurrentStep {
	case StepIdentifier:
		return buildIdentifierNodes(cfg, texts)
	case StepAuthSelect:
		return buildAuthSelectNodes(flow, cfg, texts)
	case StepPassword:
		return buildPasswordNodes(flow, texts)
	case StepMagicLink:
		return buildMagicLinkSentNodes(flow, texts)
	case StepComplete:
		return []UINode{
			{Type: "heading", Text: "Welcome!"},
			{Type: "description", Text: "Redirecting you now..."},
			{Type: "spinner"},
		}
	default:
		return []UINode{{Type: "heading", Text: "Unknown step"}}
	}
}

func buildIdentifierNodes(cfg *SchemaAuthConfig, texts map[string]string) []UINode {
	label := textOr(texts, "identifier_label", "Email or username")
	placeholder := textOr(texts, "identifier_placeholder", "you@example.com")

	nodes := []UINode{
		{Type: "heading", Text: cfg.Branding.Heading},
		{Type: "description", Text: cfg.Branding.Description},
		{Type: "input", Name: "identifier", InputType: "text",
			Label: label, Placeholder: placeholder,
			Autocomplete: "username", Required: true},
		{Type: "submit", Label: textOr(texts, "continue_button", "Continue"), Action: "identifier"},
	}

	// If passkey_first, add passkey button before identifier.
	if cfg.Login.Preset == "passkey_first" {
		if m := cfg.Login.AuthMethods["passkey"]; m != nil && m.Enabled {
			nodes = append([]UINode{
				{Type: "heading", Text: cfg.Branding.Heading},
				{Type: "description", Text: cfg.Branding.Description},
				{Type: "button", Label: "🔑 Sign in with a passkey", Action: "passkey"},
				{Type: "divider"},
			}, nodes[2:]...) // keep input + submit, drop the duplicate heading
		}
	}

	return nodes
}

func buildAuthSelectNodes(flow *Flow, cfg *SchemaAuthConfig, texts map[string]string) []UINode {
	initial := ""
	if flow.DisplayName != "" {
		initial = string([]rune(flow.DisplayName)[0])
	} else if flow.Identifier != "" {
		initial = string([]rune(flow.Identifier)[0])
	}

	nodes := []UINode{
		{Type: "avatar", Initial: initial, Text: flow.DisplayName},
		{Type: "heading", Text: flow.DisplayName},
		{Type: "description", Text: textOr(texts, "auth_select_description", "Choose how to sign in")},
	}

	// Password input (if enabled).
	if m := cfg.Login.AuthMethods["password"]; m != nil && m.Enabled {
		nodes = append(nodes,
			UINode{Type: "input", Name: "password", InputType: "password",
				Label:       textOr(texts, "password_label", "Password"),
				Placeholder: "••••••••", Autocomplete: "current-password", Required: true},
			UINode{Type: "submit", Label: textOr(texts, "signin_button", "Sign in with password"), Action: "password"},
		)
	}

	// Divider before alternative methods.
	hasAlternatives := false

	// Magic link.
	if m := cfg.Login.AuthMethods["magic_link"]; m != nil && m.Enabled {
		if !hasAlternatives {
			nodes = append(nodes, UINode{Type: "divider"})
			hasAlternatives = true
		}
		nodes = append(nodes, UINode{
			Type: "button", Label: textOr(texts, "magic_link_button", "✉ Send me a sign-in link"), Action: "magic_link",
		})
	}

	// Passkey.
	if m := cfg.Login.AuthMethods["passkey"]; m != nil && m.Enabled {
		if !hasAlternatives {
			nodes = append(nodes, UINode{Type: "divider"})
			hasAlternatives = true
		}
		nodes = append(nodes, UINode{
			Type: "button", Label: "🔑 Use a passkey", Action: "passkey",
		})
	}

	// SSO providers.
	if m := cfg.Login.AuthMethods["sso"]; m != nil && m.Enabled {
		for _, p := range flow.SSOProviders {
			if !hasAlternatives {
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

func textOr(texts map[string]string, key, fallback string) string {
	if v, ok := texts[key]; ok && v != "" {
		return v
	}
	return fallback
}

// ToFlowStep converts flow state to the response sent to the UI.
func (f *Flow) ToFlowStep() *FlowStep {
	step := &FlowStep{
		FlowID:   f.ID,
		Step:     f.CurrentStep,
		Nodes:    BuildNodes(f),
		Branding: f.SchemaConfig.Branding,
	}
	if f.DisplayName != "" {
		initial := string([]rune(f.DisplayName)[0])
		step.Identity = &FlowIdentity{
			DisplayName:   f.DisplayName,
			AvatarInitial: initial,
		}
	}
	return step
}
