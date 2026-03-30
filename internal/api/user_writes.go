package api

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
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
	schemaRec, err := schema.ResolveUserSchemaForWrite(ctx, a.db.SQL(), requestedSchemaID)
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
	schemaRec, err := schema.ResolveUserSchemaForWrite(ctx, a.db.SQL(), schemaID)
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
