package api

// types.go defines additional canonical request/response types for the
// Zitadel API that are NOT already declared in their handler files.
//
// Types already declared elsewhere (kept in place to avoid churn):
//   - UserRequest, UserResponse, ListResponse, ErrorResponse  → api.go
//   - SchemaRequest, SchemaResponse                                    → api.go
//   - SearchResult                                                     → api.go
//   - EventResponse, AggregateRow                                      → event.go
//   - SessionResponse, CreateSessionRequest, CreateSessionResponse     → session.go
//   - CreatePATRequest, CreatePATResponse, PATResponse                 → pat.go
//   - ProviderTemplate                                                 → provider.go
//   - ImportRequest, ImportEntity, ImportProvider, ImportResult, etc.   → bulk.go
//   - TokenInfo                                                        → token.go
//
// This file adds types needed for OpenAPI spec completeness and SDK generation.

// --- Status ---

// StatusResponse is a simple status acknowledgement.
type StatusResponse struct {
	Status string `json:"status"`
}

// --- Schemas (extensions) ---

// UpdateSchemaRequest is the body for PATCH /v1/schemas/{id}.
type UpdateSchemaRequest struct {
	Schema  any    `json:"schema,omitempty"`
	Message string `json:"message,omitempty"`
}

// PromoteSchemaResponse is returned from POST /v1/schemas/{id}/promote.
type PromoteSchemaResponse struct {
	Status           string `json:"status"`
	Version          int    `json:"version"`
	AffectedEntities int    `json:"affected_entities"`
}

// DiffSchemaResponse is returned from GET /v1/schemas/{id}/diff.
type DiffSchemaResponse struct {
	Left    any   `json:"left"`
	Right   any   `json:"right"`
	Changes []any `json:"changes"`
}

// PreviewSchemaRequest is the body for POST /v1/schemas/{id}/preview.
type PreviewSchemaRequest struct {
	UserID string `json:"user_id"`
}

// PreviewSchemaResponse is returned from POST /v1/schemas/{id}/preview.
type PreviewSchemaResponse struct {
	Entity        string         `json:"entity"`
	CurrentClaims map[string]any `json:"current_claims"`
	DraftClaims   map[string]any `json:"draft_claims"`
	Changes       []any          `json:"changes"`
}

// SchemaIdentityCountResponse is returned from GET /v1/schemas/{id}/identity-count.
type SchemaIdentityCountResponse struct {
	Count int `json:"count"`
}

// --- Users family (extensions) ---

// UpdateUserRequest is the body for PATCH /v1/users/{id}.
type UpdateUserRequest struct {
	Identifier   string         `json:"identifier,omitempty"`
	DisplayName  string         `json:"display_name,omitempty"`
	State        string         `json:"state,omitempty"`
	Data         map[string]any `json:"data,omitempty"`
	Profile      map[string]any `json:"profile,omitempty"`
	SchemaID     string         `json:"schema_id,omitempty"`
	Capabilities []string       `json:"capabilities,omitempty"`
}

// --- Account ---

// ProfileResponse is returned from GET /v1/account/profile.
type ProfileResponse struct {
	Identity         map[string]any            `json:"identity"`
	Schema           map[string]any            `json:"schema"`
	FieldPermissions map[string]map[string]any `json:"field_permissions"`
}

// UpdateProfileRequest is the body for PATCH /v1/account/profile.
type UpdateProfileRequest struct {
	DisplayName *string        `json:"display_name,omitempty"`
	Profile     map[string]any `json:"profile,omitempty"`
}

// UpdateProfileResponse is returned from PATCH /v1/account/profile.
type UpdateProfileResponse struct {
	Status        string   `json:"status"`
	FieldsChanged []string `json:"fields_changed"`
}

// OwnSessionsResponse is returned from GET /v1/account/sessions.
type OwnSessionsResponse struct {
	Sessions []map[string]any `json:"sessions"`
	Count    int              `json:"count"`
}

// ActivityResponse is returned from GET /v1/account/activity.
type ActivityResponse struct {
	Events []map[string]any `json:"events"`
	Count  int              `json:"count"`
}

// --- Providers (extensions) ---

// ProviderResponse is a single provider returned from the API.
type ProviderResponse struct {
	ID        string         `json:"id"`
	OrgID     string         `json:"org_id,omitempty"`
	Name      string         `json:"name"`
	Protocol  string         `json:"protocol"`
	Template  string         `json:"template,omitempty"`
	Config    map[string]any `json:"config"`
	Enabled   bool           `json:"enabled"`
	CreatedAt string         `json:"created_at"`
}

// CreateProviderRequest is the body for POST /v1/providers.
type CreateProviderRequest struct {
	Name     string         `json:"name"`
	Type     string         `json:"type"`
	Template string         `json:"template,omitempty"`
	Config   map[string]any `json:"config"`
	Enabled  bool           `json:"enabled"`
}

// UpdateProviderRequest is the body for PATCH /v1/providers/{id}.
type UpdateProviderRequest struct {
	Name    string         `json:"name,omitempty"`
	Config  map[string]any `json:"config,omitempty"`
	Enabled *bool          `json:"enabled,omitempty"`
}

// --- FGA ---

// FGACheckRequest is the body for POST /v1/fga/check.
type FGACheckRequest struct {
	User     string `json:"user"`
	Relation string `json:"relation"`
	Object   string `json:"object"`
}

// FGACheckResponse is returned from POST /v1/fga/check.
type FGACheckResponse struct {
	Allowed  bool   `json:"allowed"`
	User     string `json:"user"`
	Relation string `json:"relation"`
	Object   string `json:"object"`
}

// FGATuple represents a single relationship tuple.
type FGATuple struct {
	User     string `json:"user"`
	Relation string `json:"relation"`
	Object   string `json:"object"`
}

// FGAWriteTuplesRequest is the body for POST /v1/fga/tuples.
type FGAWriteTuplesRequest struct {
	Tuples []FGATuple `json:"tuples"`
}

// FGAWriteTuplesResponse is returned from POST /v1/fga/tuples.
type FGAWriteTuplesResponse struct {
	Status  string `json:"status"`
	Written int    `json:"written"`
}

// FGADeleteTuplesResponse is returned from DELETE /v1/fga/tuples.
type FGADeleteTuplesResponse struct {
	Status  string `json:"status"`
	Deleted int    `json:"deleted"`
}

// FGAReadTuplesResponse is returned from GET /v1/fga/tuples.
type FGAReadTuplesResponse struct {
	Tuples []FGATuple `json:"tuples"`
}

// FGAListObjectsRequest is the body for POST /v1/fga/list-objects.
type FGAListObjectsRequest struct {
	User     string `json:"user"`
	Relation string `json:"relation"`
	Type     string `json:"type"`
}

// FGAListObjectsResponse is returned from POST /v1/fga/list-objects.
type FGAListObjectsResponse struct {
	Objects []string `json:"objects"`
}

// FGAModelType is a single type in the FGA model.
type FGAModelType struct {
	Type      string   `json:"type"`
	Relations []string `json:"relations"`
}

// FGAModelResponse is returned from GET /v1/fga/model.
type FGAModelResponse struct {
	SchemaVersion string         `json:"schema_version"`
	Types         []FGAModelType `json:"types"`
}

// FGAModelNode is a node in the FGA model graph.
type FGAModelNode struct {
	ID          string   `json:"id"`
	Relations   []string `json:"relations"`
	Permissions []string `json:"permissions"`
}

// FGAModelEdge is an edge in the FGA model graph.
type FGAModelEdge struct {
	From     string `json:"from"`
	To       string `json:"to"`
	Relation string `json:"relation"`
	Kind     string `json:"kind"`
}

// FGAModelGraphResponse is returned from GET /v1/fga/model/graph.
type FGAModelGraphResponse struct {
	Nodes []FGAModelNode `json:"nodes"`
	Edges []FGAModelEdge `json:"edges"`
}

// FGAExpandRequest is the body for POST /v1/fga/expand.
type FGAExpandRequest struct {
	Relation string `json:"relation"`
	Object   string `json:"object"`
}

// FGAExpandResponse is returned from POST /v1/fga/expand.
type FGAExpandResponse struct {
	Tree any `json:"tree"`
}

// FGATestAssertion is a single assertion for batch testing.
type FGATestAssertion struct {
	User     string `json:"user"`
	Relation string `json:"relation"`
	Object   string `json:"object"`
	Expected bool   `json:"expected"`
}

// FGABatchTestRequest is the body for POST /v1/fga/test.
type FGABatchTestRequest struct {
	Assertions []FGATestAssertion `json:"assertions"`
}

// FGATestResult is a single test result.
type FGATestResult struct {
	User     string `json:"user"`
	Relation string `json:"relation"`
	Object   string `json:"object"`
	Expected bool   `json:"expected"`
	Actual   bool   `json:"actual"`
	Pass     bool   `json:"pass"`
	Error    string `json:"error,omitempty"`
}

// FGABatchTestResponse is returned from POST /v1/fga/test.
type FGABatchTestResponse struct {
	Results []FGATestResult `json:"results"`
	Total   int             `json:"total"`
	Passed  int             `json:"passed"`
	Failed  int             `json:"failed"`
}

// --- Settings ---

// SettingsResponse is returned from GET /v1/settings/{type}.
type SettingsResponse struct {
	Type    string         `json:"type"`
	Scope   string         `json:"scope"`
	ScopeID string         `json:"scope_id"`
	Data    map[string]any `json:"data"`
}

// PutSettingsRequest is the body for PUT /v1/settings/{type}.
type PutSettingsRequest struct {
	Scope   string         `json:"scope"`
	ScopeID string         `json:"scope_id"`
	Data    map[string]any `json:"data"`
}

// NotificationRenderResponse is returned from notification preview/test endpoints.
type NotificationRenderResponse struct {
	Medium      string            `json:"medium"`
	ChannelID   string            `json:"channel_id,omitempty"`
	TemplateKey string            `json:"template_key"`
	Locale      string            `json:"locale"`
	Subject     string            `json:"subject,omitempty"`
	TextBody    string            `json:"text_body"`
	HTMLBody    string            `json:"html_body,omitempty"`
	Metadata    map[string]string `json:"metadata,omitempty"`
}

// NotificationPreviewRequest renders a notification without delivering it.
type NotificationPreviewRequest struct {
	OrgID       string         `json:"org_id,omitempty"`
	Medium      string         `json:"medium"`
	TemplateKey string         `json:"template_key"`
	Locale      string         `json:"locale,omitempty"`
	Payload     map[string]any `json:"payload,omitempty"`
}

// NotificationTestRequest renders and delivers a notification immediately.
type NotificationTestRequest struct {
	OrgID       string         `json:"org_id,omitempty"`
	Medium      string         `json:"medium"`
	ChannelID   string         `json:"channel_id,omitempty"`
	Recipient   string         `json:"recipient"`
	TemplateKey string         `json:"template_key"`
	Locale      string         `json:"locale,omitempty"`
	Payload     map[string]any `json:"payload,omitempty"`
}

// NotificationPreset describes a built-in preset channel pack.
type NotificationPreset struct {
	ID          string         `json:"id"`
	Label       string         `json:"label"`
	Medium      string         `json:"medium"`
	Driver      string         `json:"driver"`
	Description string         `json:"description"`
	Config      map[string]any `json:"config"`
}

// NotificationPresetsResponse is returned from GET /v1/notifications/presets.
type NotificationPresetsResponse struct {
	Presets []NotificationPreset `json:"presets"`
}

// --- Catalog ---

// CatalogTemplateResponse is a single catalog template.
type CatalogTemplateResponse struct {
	ID          string   `json:"id"`
	Name        string   `json:"name"`
	Type        string   `json:"type"`
	Version     string   `json:"version"`
	Description string   `json:"description"`
	Tags        []string `json:"tags"`
	Source      string   `json:"source"`
}

// CatalogTemplateDetailResponse is the detail view of a catalog template.
type CatalogTemplateDetailResponse struct {
	Template  CatalogTemplateResponse `json:"template"`
	Variables map[string]any          `json:"variables"`
	Payload   map[string]any          `json:"payload"`
}

// CatalogInstallRequest is the body for POST /v1/catalog/{id}/install.
type CatalogInstallRequest struct {
	Variables map[string]any `json:"variables"`
}

// CatalogInstallResponse is returned from POST /v1/catalog/{id}/install.
type CatalogInstallResponse struct {
	ID         string `json:"id"`
	TemplateID string `json:"template_id"`
	Status     string `json:"status"`
}

// CatalogRefreshResponse is returned from POST /v1/catalog/refresh.
type CatalogRefreshResponse struct {
	Status string `json:"status"`
	New    int    `json:"new"`
}

// --- Search ---

// SearchResponse is returned from GET /v1/search.
type SearchResponse struct {
	Results []SearchResult `json:"results"`
	Query   string         `json:"query"`
	Count   int            `json:"count"`
}

// --- Counts ---

// CountsResponse is returned from GET /v1/counts.
type CountsResponse map[string]int

// --- Login / Auth ---

// LoginStartRequest is the body for POST /v1/login/start.
type LoginStartRequest struct {
	Identifier string `json:"identifier"`
}

// LoginStartResponse is returned from POST /v1/login/start.
type LoginStartResponse struct {
	FlowID string `json:"flow_id,omitempty"`
	Step   string `json:"step"`
	Status string `json:"status"`
}

// LoginPasswordRequest is the body for POST /v1/login/password.
type LoginPasswordRequest struct {
	FlowID   string `json:"flow_id"`
	Password string `json:"password"`
}

// MagicLinkRequest is the body for POST /v1/auth/magic-link.
type MagicLinkRequest struct {
	Email   string `json:"email"`
	Purpose string `json:"purpose,omitempty"`
}

// MagicLinkResponse is returned from POST /v1/auth/magic-link.
type MagicLinkResponse struct {
	Status  string `json:"status"`
	Purpose string `json:"purpose"`
	Message string `json:"message"`
}

// BrandingResponse is returned from GET /v1/branding.
type BrandingResponse struct {
	Name           string `json:"name"`
	LogoURL        string `json:"logo_url,omitempty"`
	PrimaryColor   string `json:"primary_color,omitempty"`
	BackgroundURL  string `json:"background_url,omitempty"`
	FavIconURL     string `json:"fav_icon_url,omitempty"`
	WelcomeMessage string `json:"welcome_message,omitempty"`
}

// AuthSettingsResponse is returned from GET /v1/auth/settings.
type AuthSettingsResponse struct {
	PasswordEnabled   bool                   `json:"password_enabled"`
	MagicLinkEnabled  bool                   `json:"magic_link_enabled"`
	RegisterEnabled   bool                   `json:"register_enabled"`
	ExternalProviders []AuthExternalProvider `json:"external_providers"`
}

// AuthExternalProvider is a provider entry in auth settings.
type AuthExternalProvider struct {
	ID   string `json:"id"`
	Name string `json:"name"`
	Type string `json:"type"`
}

// --- Analytics ---

// AnalyticsQueryRequest is the body for POST /v1/analytics/query.
type AnalyticsQueryRequest struct {
	Query  string         `json:"query,omitempty"`
	Table  string         `json:"table,omitempty"`
	Params map[string]any `json:"params,omitempty"`
}

// AnalyticsQueryResponse is returned from POST /v1/analytics/query.
type AnalyticsQueryResponse struct {
	Columns []string         `json:"columns"`
	Rows    []map[string]any `json:"rows"`
	Count   int              `json:"count"`
}

// --- Orgs ---
// OrgResponse and OrgRequest are declared in api.go (org handlers section).

// SetUserPasswordRequest is the payload for setting an entity's password.
type SetUserPasswordRequest struct {
	Password string `json:"password"`
}
