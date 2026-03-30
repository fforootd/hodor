package login

import (
	"testing"
)

const testSchema = `{
  "type": "object",
  "x-auth-methods": {
    "password": {"enabled": true, "interactive": true, "position": 1},
    "passkey": {"enabled": true, "interactive": true, "position": 0, "preferred": true},
    "magic_link": {"enabled": true, "interactive": true, "position": 2},
    "sso": {"enabled": true, "interactive": true, "position": 3}
  },
  "x-login": {
    "preset": "identifier_first",
    "mfa_required": false,
    "registration_allowed": true
  },
  "x-branding": {
    "heading": "Welcome to Acme",
    "description": "Sign in to your workspace",
    "org_name": "Acme Corp",
    "colors": {"primary": "#ff6600"},
    "texts": {"continue_button": "Next"}
  },
  "properties": {
    "email": {
      "type": "string",
      "format": "email",
      "x-identifier": true,
      "x-verify": "email",
      "x-recover": "email"
    },
    "phone": {
      "type": "string",
      "x-identifier": true,
      "x-mfa": "sms"
    },
    "display_name": {
      "type": "string"
    }
  }
}`

func TestExtractAuthConfig_IdentifierFields(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)

	if len(cfg.Identifiers) != 2 {
		t.Fatalf("expected 2 identifiers, got %d", len(cfg.Identifiers))
	}

	// Check email field config.
	emailCfg, ok := cfg.Fields["email"]
	if !ok {
		t.Fatal("expected email field config")
	}
	if !emailCfg.Identifier {
		t.Error("email should be an identifier")
	}
	if emailCfg.Verification != "email" {
		t.Errorf("email verification = %q, want %q", emailCfg.Verification, "email")
	}
	if emailCfg.Recovery != "email" {
		t.Errorf("email recovery = %q, want %q", emailCfg.Recovery, "email")
	}

	// Check phone field config.
	phoneCfg, ok := cfg.Fields["phone"]
	if !ok {
		t.Fatal("expected phone field config")
	}
	if !phoneCfg.Identifier {
		t.Error("phone should be an identifier")
	}
	if phoneCfg.MFA != "sms" {
		t.Errorf("phone mfa = %q, want %q", phoneCfg.MFA, "sms")
	}

	// display_name should NOT be in Fields (no x-identifier).
	if _, ok := cfg.Fields["display_name"]; ok {
		t.Error("display_name should not have auth config")
	}
}

func TestExtractAuthConfig_LoginConfig(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)

	if cfg.Login.Preset != "identifier_first" {
		t.Errorf("preset = %q, want %q", cfg.Login.Preset, "identifier_first")
	}
	if cfg.Login.MFARequired {
		t.Error("mfa_required should be false")
	}
	if !cfg.Login.RegistrationAllowed {
		t.Error("registration_allowed should be true")
	}

	pw := cfg.AuthMethods["password"]
	if pw == nil || !pw.Enabled {
		t.Error("password should be enabled")
	}
	pk := cfg.AuthMethods["passkey"]
	if pk == nil || !pk.Enabled || !pk.Preferred {
		t.Error("passkey should be enabled and preferred")
	}
}

func TestExtractAuthConfig_Branding(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)

	if cfg.Branding.Heading != "Welcome to Acme" {
		t.Errorf("heading = %q, want %q", cfg.Branding.Heading, "Welcome to Acme")
	}
	if cfg.Branding.OrgName != "Acme Corp" {
		t.Errorf("org_name = %q, want %q", cfg.Branding.OrgName, "Acme Corp")
	}
	if cfg.Branding.Colors["primary"] != "#ff6600" {
		t.Errorf("primary color = %q, want %q", cfg.Branding.Colors["primary"], "#ff6600")
	}
	// Default colors should be merged in.
	if cfg.Branding.Colors["background"] != "#f0f2ff" {
		t.Errorf("background should use default, got %q", cfg.Branding.Colors["background"])
	}
	if cfg.Branding.FontFamily != "Inter, system-ui, sans-serif" {
		t.Errorf("font_family should use default, got %q", cfg.Branding.FontFamily)
	}
	if cfg.Branding.Texts["continue_button"] != "Next" {
		t.Errorf("continue_button text = %q, want %q", cfg.Branding.Texts["continue_button"], "Next")
	}
}

func TestExtractAuthConfig_Defaults(t *testing.T) {
	// Minimal schema with no annotations.
	cfg := ExtractAuthConfig(`{"type": "object", "properties": {"name": {"type": "string"}}}`)

	if cfg.Login.Preset != "identifier_first" {
		t.Errorf("default preset = %q, want %q", cfg.Login.Preset, "identifier_first")
	}
	if cfg.Branding.Heading != "Welcome back" {
		t.Errorf("default heading = %q, want %q", cfg.Branding.Heading, "Welcome back")
	}
}

func TestExtractAuthConfig_InvalidJSON(t *testing.T) {
	cfg := ExtractAuthConfig("not json")
	if cfg == nil {
		t.Fatal("should return default config for invalid JSON")
	}
	if cfg.Login.Preset != "identifier_first" {
		t.Errorf("fallback preset = %q, want %q", cfg.Login.Preset, "identifier_first")
	}
}

func TestBuildNodes_Identifier(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)
	flow := &Flow{
		ID:           "f_test",
		SchemaConfig: cfg,
		CurrentStep:  StepIdentifier,
	}

	nodes := BuildNodes(flow)
	if len(nodes) == 0 {
		t.Fatal("expected nodes for identifier step")
	}

	// Should have a heading, description, input, and submit.
	types := map[string]bool{}
	for _, n := range nodes {
		types[n.Type] = true
	}
	for _, expected := range []string{"heading", "description", "input", "submit"} {
		if !types[expected] {
			t.Errorf("missing node type %q", expected)
		}
	}

	// Check custom text from branding.
	for _, n := range nodes {
		if n.Type == "submit" && n.Label != "Next" {
			t.Errorf("submit label = %q, want %q (from x-branding texts)", n.Label, "Next")
		}
	}
}

func TestBuildNodes_AuthSelect(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)
	flow := &Flow{
		ID:           "f_test",
		SchemaConfig: cfg,
		CurrentStep:  StepAuthSelect,
		DisplayName:  "Jane Doe",
		Identifier:   "jane@acme.com",
		RevealMode:   IdentityRevealModeKnownUser,
		SSOProviders: []map[string]any{
			{"id": "p_1", "name": "Google", "template": "google"},
		},
	}

	nodes := BuildNodes(flow)
	types := map[string]int{}
	for _, n := range nodes {
		types[n.Type]++
	}

	if types["avatar"] < 1 {
		t.Error("missing avatar node")
	}
	if types["input"] < 1 {
		t.Error("missing password input")
	}
	if types["sso_button"] < 1 {
		t.Error("missing SSO button")
	}
	if types["button"] < 2 {
		t.Error("expected at least magic_link + passkey buttons")
	}
}

func TestBuildNodes_AuthSelectAnonymousDoesNotExposeDerivedIdentity(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)
	flow := &Flow{
		ID:           "f_test",
		SchemaConfig: cfg,
		CurrentStep:  StepAuthSelect,
		Identifier:   "jane@acme.com",
		DisplayName:  "Jane Doe",
		RevealMode:   IdentityRevealModeAnonymous,
	}

	nodes := BuildNodes(flow)
	for _, node := range nodes {
		if node.Type == "avatar" {
			t.Fatalf("did not expect avatar node in anonymous auth-select: %+v", node)
		}
		if node.Type == "heading" && node.Text != "jane@acme.com" {
			t.Fatalf("heading = %q, want typed identifier", node.Text)
		}
	}
}

func TestBuildNodes_Complete(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)
	flow := &Flow{
		ID:           "f_test",
		SchemaConfig: cfg,
		CurrentStep:  StepComplete,
	}

	nodes := BuildNodes(flow)
	if len(nodes) < 2 {
		t.Fatal("expected nodes for complete step")
	}
	if nodes[0].Type != "heading" || nodes[0].Text != "Welcome!" {
		t.Errorf("first node = %+v, want heading 'Welcome!'", nodes[0])
	}
}

func TestFlowStore(t *testing.T) {
	store := NewFlowStore()

	f := &Flow{ID: "f_1", CurrentStep: StepIdentifier}
	store.Put(f)

	got, ok := store.Get("f_1")
	if !ok || got.ID != "f_1" {
		t.Fatal("expected to find flow f_1")
	}

	store.Delete("f_1")
	_, ok = store.Get("f_1")
	if ok {
		t.Error("flow should be deleted")
	}
}

func TestToFlowStep(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)
	flow := &Flow{
		ID:           "f_test",
		SchemaConfig: cfg,
		CurrentStep:  StepIdentifier,
		DisplayName:  "Jane",
		RevealMode:   IdentityRevealModeKnownUser,
	}

	step := flow.ToFlowStep()
	if step.FlowID != "f_test" {
		t.Errorf("flow_id = %q, want %q", step.FlowID, "f_test")
	}
	if step.Step != StepIdentifier {
		t.Errorf("step = %q, want %q", step.Step, StepIdentifier)
	}
	if step.Branding.Heading != "Welcome to Acme" {
		t.Errorf("branding heading = %q, want %q", step.Branding.Heading, "Welcome to Acme")
	}
	if step.Identity == nil || step.Identity.AvatarInitial != "J" {
		t.Error("expected identity with initial 'J'")
	}
	if len(step.Nodes) == 0 {
		t.Error("expected nodes in step")
	}
}

func TestToFlowStep_OmitsIdentityForAnonymousFlows(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)
	flow := &Flow{
		ID:           "f_test",
		SchemaConfig: cfg,
		CurrentStep:  StepAuthSelect,
		Identifier:   "jane@acme.com",
		DisplayName:  "Jane",
		RevealMode:   IdentityRevealModeAnonymous,
	}

	step := flow.ToFlowStep()
	if step.Identity != nil {
		t.Fatalf("expected no identity payload for anonymous flow, got %+v", step.Identity)
	}
}

func TestBuildNodes_Register(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)
	flow := &Flow{
		ID:           "f_test",
		SchemaConfig: cfg,
		CurrentStep:  StepRegister,
		Identifier:   "jane@acme.com",
	}

	nodes := BuildNodes(flow)
	if len(nodes) == 0 {
		t.Fatal("expected nodes for register step")
	}

	types := map[string]int{}
	for _, n := range nodes {
		types[n.Type]++
	}

	if types["heading"] < 1 {
		t.Error("missing heading node")
	}
	if types["input"] < 1 {
		t.Error("expected at least one input for schema fields")
	}
	if types["submit"] < 1 {
		t.Error("missing submit button")
	}

	// Check that the identifier field is pre-filled.
	for _, n := range nodes {
		if n.Type == "input" && n.Name == "email" {
			if n.Value != "jane@acme.com" {
				t.Errorf("email input value = %q, want %q", n.Value, "jane@acme.com")
			}
			if n.InputType != "email" {
				t.Errorf("email input_type = %q, want %q", n.InputType, "email")
			}
		}
	}

	// Submit button should have register_submit action.
	for _, n := range nodes {
		if n.Type == "submit" && n.Action != "register_submit" {
			t.Errorf("submit action = %q, want %q", n.Action, "register_submit")
		}
	}
}

func TestBuildNodes_IdentifierWithRegistration(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema) // registration_allowed = true
	flow := &Flow{
		ID:           "f_test",
		SchemaConfig: cfg,
		CurrentStep:  StepIdentifier,
	}

	nodes := BuildNodes(flow)
	hasRegLink := false
	for _, n := range nodes {
		if n.Type == "registration_link" {
			hasRegLink = true
			if n.Action != "register" {
				t.Errorf("registration_link action = %q, want %q", n.Action, "register")
			}
		}
	}
	if !hasRegLink {
		t.Error("expected registration_link when registration_allowed=true")
	}
}

func TestFlowErrors_ClearedAfterRender(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)
	flow := &Flow{
		ID:           "f_test",
		SchemaConfig: cfg,
		CurrentStep:  StepIdentifier,
		Errors: []FlowError{
			{Code: "test", Message: "Test error"},
		},
		Messages: []FlowMessage{
			{Type: "info", Text: "Test info"},
		},
	}

	step := flow.ToFlowStep()
	if len(step.Errors) != 1 {
		t.Errorf("expected 1 error in step, got %d", len(step.Errors))
	}
	if len(step.Messages) != 1 {
		t.Errorf("expected 1 message in step, got %d", len(step.Messages))
	}

	// After rendering, errors/messages should be cleared from flow.
	if len(flow.Errors) != 0 {
		t.Error("flow errors should be cleared after ToFlowStep()")
	}
	if len(flow.Messages) != 0 {
		t.Error("flow messages should be cleared after ToFlowStep()")
	}
}

func TestExtractAuthConfig_SchemaProps(t *testing.T) {
	cfg := ExtractAuthConfig(testSchema)

	if len(cfg.SchemaProps) == 0 {
		t.Fatal("expected schema props to be populated")
	}

	found := map[string]bool{}
	for _, f := range cfg.SchemaProps {
		found[f.Name] = true
		if f.Name == "email" {
			if f.Format != "email" {
				t.Errorf("email format = %q, want %q", f.Format, "email")
			}
			if !f.Identifier {
				t.Error("email should be marked as identifier")
			}
		}
	}

	if !found["email"] {
		t.Error("expected email in schema props")
	}
	if !found["display_name"] {
		t.Error("expected display_name in schema props")
	}
}

func TestHumanize(t *testing.T) {
	tests := []struct {
		input string
		want  string
	}{
		{"display_name", "Display Name"},
		{"email", "Email"},
		{"first-name", "First Name"},
		{"id", "Id"},
	}
	for _, tt := range tests {
		got := humanize(tt.input)
		if got != tt.want {
			t.Errorf("humanize(%q) = %q, want %q", tt.input, got, tt.want)
		}
	}
}

func TestFieldInputType(t *testing.T) {
	tests := []struct {
		field SchemaFieldDef
		want  string
	}{
		{SchemaFieldDef{Format: "email"}, "email"},
		{SchemaFieldDef{Sensitive: true}, "password"},
		{SchemaFieldDef{Type: "integer"}, "number"},
		{SchemaFieldDef{Type: "boolean"}, "checkbox"},
		{SchemaFieldDef{Type: "string"}, "text"},
	}
	for _, tt := range tests {
		got := fieldInputType(tt.field)
		if got != tt.want {
			t.Errorf("fieldInputType(%+v) = %q, want %q", tt.field, got, tt.want)
		}
	}
}
