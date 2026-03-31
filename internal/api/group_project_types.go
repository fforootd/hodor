package api

import "context"

type GroupRequest struct {
	SchemaID    string `json:"schema_id,omitempty"`
	Name        string `json:"name"`
	Description string `json:"description,omitempty"`
	State       string `json:"state,omitempty"`
	Metadata    any    `json:"metadata,omitempty"`
	Data        any    `json:"data,omitempty"`
}

type GroupResponse struct {
	ID          string `json:"id"`
	OrgID       string `json:"org_id"`
	Name        string `json:"name"`
	Description string `json:"description"`
	State       string `json:"state"`
	SchemaID    string `json:"schema_id,omitempty"`
	SchemaType  string `json:"schema_type,omitempty"`
	Metadata    any    `json:"metadata,omitempty"`
	Data        any    `json:"data,omitempty"`
	MemberCount int    `json:"member_count"`
	CreatedAt   string `json:"created_at"`
	UpdatedAt   string `json:"updated_at"`
}

type MemberRequest struct {
	UserID string `json:"user_id"`
	Role   string `json:"role,omitempty"`
}

type MemberResponse struct {
	UserID      string `json:"user_id"`
	DisplayName string `json:"display_name,omitempty"`
	Role        string `json:"role"`
	AddedAt     string `json:"added_at"`
}

type ProjectRequest struct {
	SchemaID    string `json:"schema_id,omitempty"`
	Name        string `json:"name"`
	Description string `json:"description,omitempty"`
	State       string `json:"state,omitempty"`
	Metadata    any    `json:"metadata,omitempty"`
	Data        any    `json:"data,omitempty"`
}

type ProjectResponse struct {
	ID          string `json:"id"`
	OrgID       string `json:"org_id"`
	Name        string `json:"name"`
	Description string `json:"description"`
	State       string `json:"state"`
	SchemaID    string `json:"schema_id,omitempty"`
	SchemaType  string `json:"schema_type,omitempty"`
	Metadata    any    `json:"metadata,omitempty"`
	Data        any    `json:"data,omitempty"`
	MemberCount int    `json:"member_count"`
	CreatedAt   string `json:"created_at"`
	UpdatedAt   string `json:"updated_at"`
}

func (a *API) buildGroupResponse(ctx context.Context, row GroupResponse, metadataStr string) GroupResponse {
	metadata := decodeObjectString(metadataStr)
	if rec, err := a.resolveResourceSchema(ctx, "group", row.SchemaID); err == nil {
		row.SchemaID = rec.ID
		row.SchemaType = rec.Type
	}
	row.Data = groupCanonicalData(row.Name, row.Description, metadata)
	if dataMap, ok := row.Data.(map[string]any); ok {
		row.Metadata = dataMap["metadata"]
	}
	return row
}

func (a *API) buildProjectResponse(ctx context.Context, row ProjectResponse, metadataStr string) ProjectResponse {
	metadata := decodeObjectString(metadataStr)
	if rec, err := a.resolveResourceSchema(ctx, "project", row.SchemaID); err == nil {
		row.SchemaID = rec.ID
		row.SchemaType = rec.Type
	}
	row.Data = projectCanonicalData(row.Name, row.Description, metadata)
	if dataMap, ok := row.Data.(map[string]any); ok {
		row.Metadata = dataMap["metadata"]
	}
	return row
}
