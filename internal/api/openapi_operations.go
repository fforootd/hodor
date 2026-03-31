package api

// openapi_operations.go registers all API operations in the OpenAPI registry.
// This is called from RegisterRoutes to ensure the spec stays in sync with routes.

// registerOpenAPIOperations populates the spec registry with all API operations.
func (a *API) registerOpenAPIOperations() {
	idParam := OpenAPIParam{Name: "id", Type: "string", Required: true, Description: "Resource ID"}
	typeParam := OpenAPIParam{Name: "type", Type: "string", Required: true, Description: "Setting type"}

	// ─── Users ─────────────────────────────────────────────────
	listParams := []OpenAPIParam{
		{Name: "cursor", Type: "string", Description: "Pagination cursor"},
		{Name: "limit", Type: "integer", Description: "Max results (default 50)"},
		{Name: "org_id", Type: "string", Description: "Filter by org"},
		{Name: "state", Type: "string", Description: "Filter by state"},
		{Name: "schema_type", Type: "string", Description: "Filter the users family by schema type such as human_user, service_user, or ai_agent"},
	}
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/users", ID: "listUsers",
		Summary: "List users", Tags: []string{"Users"},
		Description: "Returns the typed users family. Use schema_type to filter human users, service accounts, or AI agents.",
		Response:    ListResponse{}, QueryParams: listParams, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/users", ID: "createUser",
		Summary: "Create a users-family resource", Tags: []string{"Users"},
		Description: "Creates a human user, service account, or AI agent. Provide schema_id in the request body to select the schema-backed subtype.",
		Request:     UserRequest{}, Response: UserResponse{},
		StatusCode: 201, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/users/{id}", ID: "getUser",
		Summary: "Get a users-family resource", Tags: []string{"Users"},
		Description: "Returns a single resource from the users family, including schema_id and schema_type.",
		Response:    UserResponse{}, PathParams: []OpenAPIParam{idParam},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "PATCH", Path: "/v1/users/{id}", ID: "updateUser",
		Summary: "Update a users-family resource", Tags: []string{"Users"},
		Description: "Updates a resource from the users family. Keep schema_id as the canonical subtype discriminator in write payloads.",
		Request:     UpdateUserRequest{}, Response: UserResponse{},
		PathParams: []OpenAPIParam{idParam}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "DELETE", Path: "/v1/users/{id}", ID: "deleteUser",
		Summary: "Delete a users-family resource", Tags: []string{"Users"},
		Description: "Deletes a resource from the canonical users family endpoint.",
		PathParams:  []OpenAPIParam{idParam}, StatusCode: 204, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/users/{id}/password", ID: "setUserPassword",
		Summary: "Set a password for a users-family resource", Tags: []string{"Users"},
		Description: "Sets a password when the selected users-family schema supports password authentication.",
		Request:     SetUserPasswordRequest{}, PathParams: []OpenAPIParam{idParam},
		StatusCode: 204, Security: true,
	})

	// ─── Applications ──────────────────────────────────────────
	appListParams := []OpenAPIParam{
		{Name: "cursor", Type: "string", Description: "Pagination cursor"},
		{Name: "limit", Type: "integer", Description: "Max results (default 50)"},
		{Name: "org_id", Type: "string", Description: "Filter by org"},
		{Name: "state", Type: "string", Description: "Filter by state"},
		{Name: "schema_type", Type: "string", Description: "Filter the applications family by schema type such as app"},
	}
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/apps", ID: "listApps",
		Summary: "List applications", Tags: []string{"Applications"},
		Description: "Returns the typed applications family. Use schema_type to filter a specific application schema.",
		Response:    ListResponse{}, QueryParams: appListParams, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/apps", ID: "createApp",
		Summary: "Create an application", Tags: []string{"Applications"},
		Description: "Creates an application-family resource. Provide schema_id in the request body to select the schema-backed subtype.",
		Request:     AppRequest{}, Response: AppResponse{},
		StatusCode: 201, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/apps/{id}", ID: "getApp",
		Summary: "Get an application", Tags: []string{"Applications"},
		Description: "Returns a single application-family resource, including schema_id and schema_type.",
		Response:    AppResponse{}, PathParams: []OpenAPIParam{idParam},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "PATCH", Path: "/v1/apps/{id}", ID: "updateApp",
		Summary: "Update an application", Tags: []string{"Applications"},
		Description: "Updates an application-family resource. Keep schema_id as the canonical subtype discriminator in write payloads.",
		Request:     AppRequest{}, Response: AppResponse{},
		PathParams: []OpenAPIParam{idParam}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "DELETE", Path: "/v1/apps/{id}", ID: "deleteApp",
		Summary: "Delete an application", Tags: []string{"Applications"},
		Description: "Deletes a resource from the canonical applications family endpoint.",
		PathParams:  []OpenAPIParam{idParam}, StatusCode: 204, Security: true,
	})

	// ─── Schemas ───────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/schemas", ID: "listSchemas",
		Summary: "List schemas", Tags: []string{"Schemas"},
		Response: ListResponse{},
		QueryParams: []OpenAPIParam{
			{Name: "type", Type: "string", Description: "Filter by schema type"},
		},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/schemas", ID: "createSchema",
		Summary: "Create a schema", Tags: []string{"Schemas"},
		Request: SchemaRequest{}, Response: SchemaResponse{},
		StatusCode: 201, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/schemas/$meta", ID: "getMetaSchema",
		Summary: "Get the meta schema", Tags: []string{"Schemas"},
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/schemas/{id}", ID: "getSchema",
		Summary: "Get a schema", Tags: []string{"Schemas"},
		Response: SchemaResponse{}, PathParams: []OpenAPIParam{idParam},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "PATCH", Path: "/v1/schemas/{id}", ID: "updateSchema",
		Summary: "Update a schema", Tags: []string{"Schemas"},
		Request: UpdateSchemaRequest{}, Response: SchemaResponse{},
		PathParams: []OpenAPIParam{idParam}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/schemas/{id}/promote", ID: "promoteSchema",
		Summary: "Promote a schema version", Tags: []string{"Schemas"},
		Response: PromoteSchemaResponse{}, PathParams: []OpenAPIParam{idParam},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/schemas/{id}/diff", ID: "diffSchema",
		Summary: "Diff two schema versions", Tags: []string{"Schemas"},
		Response: DiffSchemaResponse{}, PathParams: []OpenAPIParam{idParam},
		QueryParams: []OpenAPIParam{{Name: "compare", Type: "string", Required: true, Description: "ID of schema to compare against"}},
		Security:    true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/schemas/{id}/preview", ID: "previewSchema",
		Summary: "Preview schema changes on an entity", Tags: []string{"Schemas"},
		Request: PreviewSchemaRequest{}, Response: PreviewSchemaResponse{},
		PathParams: []OpenAPIParam{idParam}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/schemas/{id}/identity-count", ID: "schemaIdentityCount",
		Summary: "Count entities using a schema", Tags: []string{"Schemas"},
		Response: SchemaIdentityCountResponse{}, PathParams: []OpenAPIParam{idParam},
		Security: true,
	})

	// ─── Sessions ──────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/sessions", ID: "listSessions",
		Summary: "List sessions", Tags: []string{"Sessions"},
		Response: ListResponse{}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/sessions", ID: "createSession",
		Summary: "Create a session", Tags: []string{"Sessions"},
		Request: CreateSessionRequest{}, Response: CreateSessionResponse{},
		StatusCode: 201, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/sessions/{id}", ID: "getSession",
		Summary: "Get a session", Tags: []string{"Sessions"},
		Response: SessionResponse{}, PathParams: []OpenAPIParam{idParam},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/sessions/{id}/revoke", ID: "revokeSession",
		Summary: "Revoke a session", Tags: []string{"Sessions"},
		Response: StatusResponse{}, PathParams: []OpenAPIParam{idParam},
		Security: true,
	})

	// ─── Events ────────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/events", ID: "listEvents",
		Summary: "List events", Tags: []string{"Events"},
		Response: ListResponse{},
		QueryParams: []OpenAPIParam{
			{Name: "cursor", Type: "string"},
			{Name: "limit", Type: "integer"},
			{Name: "types", Type: "string", Description: "Comma-separated event types"},
			{Name: "aggregate_type", Type: "string"},
			{Name: "aggregate_id", Type: "string"},
			{Name: "session_id", Type: "string"},
			{Name: "since", Type: "string", Description: "ISO 8601 timestamp"},
		},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/events/aggregate", ID: "aggregateEvents",
		Summary: "Aggregate events", Tags: []string{"Events"},
		QueryParams: []OpenAPIParam{
			{Name: "query", Type: "string", Required: true, Description: "Aggregate query name"},
			{Name: "org_id", Type: "string"},
		},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/events/stream", ID: "streamEvents",
		Summary: "Stream events via SSE", Tags: []string{"Events"},
		Description: "Server-Sent Events stream for real-time event notifications.",
		QueryParams: []OpenAPIParam{
			{Name: "cursor", Type: "string"},
			{Name: "types", Type: "string"},
		},
		Security: true,
	})

	// ─── PATs ──────────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/pats", ID: "createPAT",
		Summary: "Create a personal access token", Tags: []string{"PATs"},
		Request: CreatePATRequest{}, Response: CreatePATResponse{},
		StatusCode: 201, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/pats", ID: "listPATs",
		Summary: "List personal access tokens", Tags: []string{"PATs"},
		Response: ListResponse{}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "DELETE", Path: "/v1/pats/{id}", ID: "revokePAT",
		Summary: "Revoke a personal access token", Tags: []string{"PATs"},
		PathParams: []OpenAPIParam{idParam}, StatusCode: 204, Security: true,
	})

	// ─── Account ───────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/account/profile", ID: "getProfile",
		Summary: "Get own profile", Tags: []string{"Account"},
		Response: ProfileResponse{}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "PATCH", Path: "/v1/account/profile", ID: "updateProfile",
		Summary: "Update own profile", Tags: []string{"Account"},
		Request: UpdateProfileRequest{}, Response: UpdateProfileResponse{},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/account/sessions", ID: "listOwnSessions",
		Summary: "List own sessions", Tags: []string{"Account"},
		Response: OwnSessionsResponse{}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/account/sessions/{id}/revoke", ID: "revokeOwnSession",
		Summary: "Revoke own session", Tags: []string{"Account"},
		PathParams: []OpenAPIParam{idParam}, Response: StatusResponse{},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/account/sessions/revoke-others", ID: "revokeOtherSessions",
		Summary: "Revoke all other sessions", Tags: []string{"Account"},
		Response: StatusResponse{}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/account/activity", ID: "listOwnActivity",
		Summary: "List own activity", Tags: []string{"Account"},
		Response:    ActivityResponse{},
		QueryParams: []OpenAPIParam{{Name: "limit", Type: "integer"}},
		Security:    true,
	})

	// ─── Providers ─────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/providers", ID: "listProviders",
		Summary: "List identity providers", Tags: []string{"Providers"},
		Response: ListResponse{}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/providers", ID: "createProvider",
		Summary: "Create a provider", Tags: []string{"Providers"},
		Request: CreateProviderRequest{}, Response: ProviderResponse{},
		StatusCode: 201, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/providers/{id}", ID: "getProvider",
		Summary: "Get a provider", Tags: []string{"Providers"},
		Response: ProviderResponse{}, PathParams: []OpenAPIParam{idParam},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "PATCH", Path: "/v1/providers/{id}", ID: "updateProvider",
		Summary: "Update a provider", Tags: []string{"Providers"},
		Request: UpdateProviderRequest{}, Response: ProviderResponse{},
		PathParams: []OpenAPIParam{idParam}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "DELETE", Path: "/v1/providers/{id}", ID: "deleteProvider",
		Summary: "Delete a provider", Tags: []string{"Providers"},
		PathParams: []OpenAPIParam{idParam}, StatusCode: 204, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/providers/templates", ID: "listProviderTemplates",
		Summary: "List provider templates", Tags: []string{"Providers"},
	})

	// ─── FGA ───────────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/fga/check", ID: "fgaCheck",
		Summary: "Check authorization", Tags: []string{"Authorization"},
		Request: FGACheckRequest{}, Response: FGACheckResponse{},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/fga/tuples", ID: "fgaWriteTuples",
		Summary: "Write relationship tuples", Tags: []string{"Authorization"},
		Request: FGAWriteTuplesRequest{}, Response: FGAWriteTuplesResponse{},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "DELETE", Path: "/v1/fga/tuples", ID: "fgaDeleteTuples",
		Summary: "Delete relationship tuples", Tags: []string{"Authorization"},
		Request: FGAWriteTuplesRequest{}, Response: FGADeleteTuplesResponse{},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/fga/tuples", ID: "fgaReadTuples",
		Summary: "Read relationship tuples", Tags: []string{"Authorization"},
		Response: FGAReadTuplesResponse{},
		QueryParams: []OpenAPIParam{
			{Name: "user", Type: "string"},
			{Name: "relation", Type: "string"},
			{Name: "object", Type: "string"},
		},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/fga/list-objects", ID: "fgaListObjects",
		Summary: "List authorized objects", Tags: []string{"Authorization"},
		Request: FGAListObjectsRequest{}, Response: FGAListObjectsResponse{},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/fga/model", ID: "fgaGetModel",
		Summary: "Get authorization model", Tags: []string{"Authorization"},
		Response: FGAModelResponse{}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/fga/model/graph", ID: "fgaModelGraph",
		Summary: "Get authorization model as graph", Tags: []string{"Authorization"},
		Response: FGAModelGraphResponse{}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/fga/expand", ID: "fgaExpand",
		Summary: "Expand relationship tree", Tags: []string{"Authorization"},
		Request: FGAExpandRequest{}, Response: FGAExpandResponse{},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/fga/test", ID: "fgaBatchTest",
		Summary: "Batch test authorization assertions", Tags: []string{"Authorization"},
		Request: FGABatchTestRequest{}, Response: FGABatchTestResponse{},
		Security: true,
	})

	// ─── Settings ──────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/settings/{type}", ID: "getSettings",
		Summary: "Get settings", Tags: []string{"Settings"},
		Response: SettingsResponse{}, PathParams: []OpenAPIParam{typeParam},
		QueryParams: []OpenAPIParam{
			{Name: "scope", Type: "string"},
			{Name: "scope_id", Type: "string"},
		},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "PUT", Path: "/v1/settings/{type}", ID: "putSettings",
		Summary: "Create or update settings", Tags: []string{"Settings"},
		Request: PutSettingsRequest{}, Response: StatusResponse{},
		PathParams: []OpenAPIParam{typeParam}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "DELETE", Path: "/v1/settings/{type}", ID: "deleteSettings",
		Summary: "Delete settings", Tags: []string{"Settings"},
		PathParams: []OpenAPIParam{typeParam}, StatusCode: 204,
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/notifications/presets", ID: "listNotificationPresets",
		Summary: "List notification presets", Tags: []string{"Notifications"},
		Response: NotificationPresetsResponse{}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/notifications/preview", ID: "previewNotification",
		Summary: "Preview a notification template", Tags: []string{"Notifications"},
		Request: NotificationPreviewRequest{}, Response: NotificationRenderResponse{},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/notifications/test", ID: "testNotification",
		Summary: "Send a test notification", Tags: []string{"Notifications"},
		Request: NotificationTestRequest{}, Response: NotificationRenderResponse{},
		Security: true,
	})

	// ─── Catalog ───────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/catalog", ID: "listCatalog",
		Summary: "List catalog templates", Tags: []string{"Catalog"},
		QueryParams: []OpenAPIParam{
			{Name: "type", Type: "string"},
			{Name: "tags", Type: "string"},
		},
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/catalog/{id}", ID: "getCatalogEntry",
		Summary: "Get a catalog template", Tags: []string{"Catalog"},
		Response: CatalogTemplateDetailResponse{}, PathParams: []OpenAPIParam{idParam},
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/catalog/{id}/install", ID: "installFromCatalog",
		Summary: "Install a catalog template", Tags: []string{"Catalog"},
		Request: CatalogInstallRequest{}, Response: CatalogInstallResponse{},
		PathParams: []OpenAPIParam{idParam}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/catalog/refresh", ID: "refreshCatalog",
		Summary: "Refresh catalog templates", Tags: []string{"Catalog"},
		Response: CatalogRefreshResponse{}, Security: true,
	})

	// ─── Import / Bulk ─────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/import", ID: "importData",
		Summary: "Import entities and providers", Tags: []string{"Import"},
		Request: ImportRequest{}, Response: ImportResult{},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/admin/bulk", ID: "adminBulk",
		Summary: "Bulk create entities (admin)", Tags: []string{"Admin"},
		Security: true,
	})

	// ─── Search ────────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/search", ID: "search",
		Summary: "Universal search across all resources", Tags: []string{"Search"},
		Response: SearchResponse{},
		QueryParams: []OpenAPIParam{
			{Name: "q", Type: "string", Required: true, Description: "Search query"},
			{Name: "limit", Type: "integer"},
		},
		Security: true,
	})

	// ─── Counts ────────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/counts", ID: "entityCounts",
		Summary: "Get entity counts by type", Tags: []string{"Entities"},
		Response: CountsResponse{}, Security: true,
	})

	// ─── Login / Auth ──────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/branding", ID: "getBranding",
		Summary: "Get branding settings", Tags: []string{"Auth"},
		Response: BrandingResponse{},
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/auth/settings", ID: "getAuthSettings",
		Summary: "Get authentication settings", Tags: []string{"Auth"},
		Response: AuthSettingsResponse{},
	})
	// Legacy login routes (loginStart, loginPassword, loginComplete) removed per ADR-019.
	// All login is now handled by the Flow API.
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/auth/magic-link", ID: "sendMagicLink",
		Summary: "Send a magic link", Tags: []string{"Auth"},
		Request: MagicLinkRequest{}, Response: MagicLinkResponse{},
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/auth/magic-link/verify", ID: "verifyMagicLink",
		Summary: "Verify a magic link token", Tags: []string{"Auth"},
		QueryParams: []OpenAPIParam{
			{Name: "token", Type: "string", Required: true},
		},
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/auth/sso/{provider_id}/start", ID: "ssoStart",
		Summary: "Start SSO flow", Tags: []string{"Auth"},
		PathParams: []OpenAPIParam{{Name: "provider_id", Type: "string", Required: true, Description: "Provider ID"}},
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/auth/sso/callback", ID: "ssoCallback",
		Summary: "SSO callback", Tags: []string{"Auth"},
	})

	// ─── Login Flows ───────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/login/flows", ID: "createFlow",
		Summary: "Create a login flow", Tags: []string{"Auth"},
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/login/flows/{id}", ID: "submitFlow",
		Summary: "Submit a login flow step", Tags: []string{"Auth"},
		PathParams: []OpenAPIParam{idParam},
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/login/flows/{id}", ID: "getFlow",
		Summary: "Get login flow state", Tags: []string{"Auth"},
		PathParams: []OpenAPIParam{idParam},
	})

	// ─── Orgs ──────────────────────────────────────────────────
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/orgs", ID: "listOrgs",
		Summary: "List organizations", Tags: []string{"Organizations"},
		Response: ListResponse{}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "POST", Path: "/v1/orgs", ID: "createOrg",
		Summary: "Create an organization", Tags: []string{"Organizations"},
		Request: OrgRequest{}, Response: OrgResponse{},
		StatusCode: 201, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "GET", Path: "/v1/orgs/{id}", ID: "getOrg",
		Summary: "Get an organization", Tags: []string{"Organizations"},
		Response: OrgResponse{}, PathParams: []OpenAPIParam{idParam},
		Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "PATCH", Path: "/v1/orgs/{id}", ID: "updateOrg",
		Summary: "Update an organization", Tags: []string{"Organizations"},
		Request: OrgRequest{}, Response: OrgResponse{},
		PathParams: []OpenAPIParam{idParam}, Security: true,
	})
	a.spec.Add(OpenAPIOperation{
		Method: "DELETE", Path: "/v1/orgs/{id}", ID: "deleteOrg",
		Summary: "Delete an organization", Tags: []string{"Organizations"},
		PathParams: []OpenAPIParam{idParam}, StatusCode: 204, Security: true,
	})
}
