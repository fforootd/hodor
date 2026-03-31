package api

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"sort"
	"strings"

	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

type validatedUserWrite struct {
	Schema       *schema.SchemaRecord
	Metadata     map[string]any
	MetadataJSON string
	Payload      map[string]any
	Constraints  []uniqueness.FieldConstraint
}

func (a *API) prepareUserWrite(ctx context.Context, requestedSchemaID, identifier, displayName string, metadataValue, profileValue any) (*validatedUserWrite, error) {
	schemaRec, err := schema.ResolveUserSchemaForWrite(ctx, a.db.SQL(), requestedSchemaID, a.db.Dialect())
	if err != nil {
		return nil, err
	}

	metadata, err := schema.MergeObjectMaps(metadataValue, profileValue)
	if err != nil {
		return nil, err
	}

	payload := schema.MaterializeUserData(schemaRec.Schema, identifier, displayName, metadata)
	if err := schema.ValidateData(schemaRec.Schema, payload); err != nil {
		return nil, err
	}

	metadataJSON := "{}"
	if len(metadata) > 0 {
		bytes, err := json.Marshal(metadata)
		if err != nil {
			return nil, fmt.Errorf("marshal metadata: %w", err)
		}
		metadataJSON = string(bytes)
	}

	return &validatedUserWrite{
		Schema:       schemaRec,
		Metadata:     metadata,
		MetadataJSON: metadataJSON,
		Payload:      payload,
		Constraints:  uniqueness.ExtractConstraints(schemaRec.Schema),
	}, nil
}

func (a *API) prepareExistingUserWrite(ctx context.Context, schemaID, identifier, displayName string, metadata map[string]any) (*validatedUserWrite, error) {
	schemaRec, err := schema.ResolveUserSchemaForWrite(ctx, a.db.SQL(), schemaID, a.db.Dialect())
	if err != nil {
		return nil, err
	}

	payload := schema.MaterializeUserData(schemaRec.Schema, identifier, displayName, metadata)
	if err := schema.ValidateData(schemaRec.Schema, payload); err != nil {
		return nil, err
	}

	metadataJSON := "{}"
	if len(metadata) > 0 {
		bytes, err := json.Marshal(metadata)
		if err != nil {
			return nil, fmt.Errorf("marshal metadata: %w", err)
		}
		metadataJSON = string(bytes)
	}

	return &validatedUserWrite{
		Schema:       schemaRec,
		Metadata:     metadata,
		MetadataJSON: metadataJSON,
		Payload:      payload,
		Constraints:  uniqueness.ExtractConstraints(schemaRec.Schema),
	}, nil
}

func validatedUserWriteFromData(schemaRec *schema.SchemaRecord, identifier, displayName string, data map[string]any) (*validatedUserWrite, error) {
	metadata := userMetadataFromData(schemaRec.Schema, identifier, data)
	payload := schema.MaterializeUserData(schemaRec.Schema, identifier, displayName, metadata)
	if err := schema.ValidateData(schemaRec.Schema, payload); err != nil {
		return nil, err
	}

	metadataJSON := "{}"
	if len(metadata) > 0 {
		bytes, err := json.Marshal(metadata)
		if err != nil {
			return nil, fmt.Errorf("marshal metadata: %w", err)
		}
		metadataJSON = string(bytes)
	}

	return &validatedUserWrite{
		Schema:       schemaRec,
		Metadata:     metadata,
		MetadataJSON: metadataJSON,
		Payload:      payload,
		Constraints:  uniqueness.ExtractConstraints(schemaRec.Schema),
	}, nil
}

func identifierFromUserData(schemaJSON string, data map[string]any, currentIdentifier string) string {
	if len(data) == 0 {
		return strings.TrimSpace(currentIdentifier)
	}

	if field := schema.ResolveIdentifierField(schemaJSON, currentIdentifier); field != "" {
		if value := stringFromAny(data[field]); value != "" {
			return value
		}
	}

	for _, key := range []string{"email", "phone", "username", "identifier"} {
		if value := stringFromAny(data[key]); value != "" {
			if field := schema.ResolveIdentifierField(schemaJSON, value); field != "" {
				return value
			}
		}
	}

	type propertySpec struct {
		Format     string `json:"format"`
		Identifier bool   `json:"x-identifier"`
	}
	var raw struct {
		Properties map[string]propertySpec `json:"properties"`
	}
	if err := json.Unmarshal([]byte(schemaJSON), &raw); err != nil {
		return ""
	}

	var fields []string
	for name, spec := range raw.Properties {
		if spec.Identifier {
			fields = append(fields, name)
		}
	}
	sort.Strings(fields)
	for _, name := range fields {
		if value := stringFromAny(data[name]); value != "" {
			return value
		}
	}

	return ""
}

func userMetadataFromData(schemaJSON, identifier string, data map[string]any) map[string]any {
	metadata := cloneUserObjectMap(data)
	delete(metadata, "display_name")
	if field := schema.ResolveIdentifierField(schemaJSON, identifier); field != "" {
		delete(metadata, field)
	}
	return metadata
}

func cloneUserObjectMap(src map[string]any) map[string]any {
	if src == nil {
		return map[string]any{}
	}
	dst := make(map[string]any, len(src))
	for key, value := range src {
		dst[key] = value
	}
	return dst
}

func enforceUserUniqueness(ctx context.Context, tx *sql.Tx, userID, orgID, identifier string, write *validatedUserWrite) error {
	if err := uniqueness.EnforceFromIdentifier(ctx, tx, userID, orgID, identifier); err != nil {
		return err
	}
	if write == nil || len(write.Constraints) == 0 {
		return nil
	}
	return uniqueness.Enforce(ctx, tx, userID, orgID, write.Constraints, write.Payload)
}

func reindexUserUniqueness(ctx context.Context, tx *sql.Tx, userID, orgID, identifier string, write *validatedUserWrite) error {
	if err := uniqueness.Release(ctx, tx, userID); err != nil {
		return err
	}
	return enforceUserUniqueness(ctx, tx, userID, orgID, identifier, write)
}

func userWriteBadRequest(err error) string {
	if err == nil {
		return "invalid user payload"
	}
	msg := strings.TrimSpace(err.Error())
	if msg == "" {
		return "invalid user payload"
	}
	return msg
}
