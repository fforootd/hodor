package oidcop

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/google/uuid"
	"github.com/zitadel/oidc/v3/pkg/oidc"
	"github.com/zitadel/oidc/v3/pkg/op"

	"github.com/zitadel/zitadel/internal/auth"
)

func (s *Storage) GetClientByClientID(ctx context.Context, clientID string) (op.Client, error) {
	scoped := s.scoped(ctx)
	var appType, redirectURIsJSON, grantTypesJSON, responseTypesJSON, metadataJSON, schemaJSON sql.NullString
	err := scoped.QueryRowContext(ctx, scoped.Rebind(
		`SELECT COALESCE(a.app_type, 'web'),
		        COALESCE(a.redirect_uris, '[]'),
		        COALESCE(a.grant_types, '[]'),
		        COALESCE(a.response_types, '[]'),
		        COALESCE(a.metadata, '{}'),
		        COALESCE(sc.schema, '{}')
		 FROM apps a
		 LEFT JOIN schemas sc ON a.schema_id = sc.id
		 WHERE a.client_id = ? AND a.state = 'active' AND a.instance_id = ?`),
		clientID, scoped.InstanceID(),
	).Scan(&appType, &redirectURIsJSON, &grantTypesJSON, &responseTypesJSON, &metadataJSON, &schemaJSON)
	if err != nil {
		return nil, fmt.Errorf("client not found: %w", err)
	}

	dataJSON := buildClientDataJSON(
		metadataJSON.String,
		appType.String,
		redirectURIsJSON.String,
		grantTypesJSON.String,
		responseTypesJSON.String,
	)

	return ClientFromIdentity(clientID, dataJSON, schemaJSON.String)
}

func buildClientDataJSON(metadataJSON, appType, redirectURIsJSON, grantTypesJSON, responseTypesJSON string) string {
	data := map[string]any{}
	if metadataJSON != "" && metadataJSON != "{}" {
		_ = json.Unmarshal([]byte(metadataJSON), &data)
	}
	if data == nil {
		data = map[string]any{}
	}

	if strings.TrimSpace(appType) != "" {
		data["app_type"] = strings.TrimSpace(appType)
	}
	if json.Valid([]byte(redirectURIsJSON)) {
		var redirectURIs []string
		if err := json.Unmarshal([]byte(redirectURIsJSON), &redirectURIs); err == nil {
			data["redirect_uris"] = redirectURIs
		}
	}
	if json.Valid([]byte(grantTypesJSON)) {
		var grantTypes []string
		if err := json.Unmarshal([]byte(grantTypesJSON), &grantTypes); err == nil {
			data["grant_types"] = grantTypes
		}
	}
	if json.Valid([]byte(responseTypesJSON)) {
		var responseTypes []string
		if err := json.Unmarshal([]byte(responseTypesJSON), &responseTypes); err == nil {
			data["response_types"] = responseTypes
		}
	}

	encoded, err := json.Marshal(data)
	if err != nil {
		return "{}"
	}
	return string(encoded)
}

func (s *Storage) AuthorizeClientIDSecret(ctx context.Context, clientID, clientSecret string) error {
	scoped := s.scoped(ctx)
	var storedHash string
	err := scoped.QueryRowContext(ctx,
		scoped.Rebind(`SELECT client_secret FROM apps WHERE client_id = ? AND state = 'active' AND instance_id = ?`),
		clientID, scoped.InstanceID(),
	).Scan(&storedHash)
	if err != nil {
		return fmt.Errorf("client not found or no secret configured")
	}

	if storedHash == "" {
		return fmt.Errorf("no client secret configured")
	}

	passwords := auth.NewPasswords(s.db)
	ok, _, err := passwords.Verify(storedHash, clientSecret)
	if err != nil || !ok {
		return fmt.Errorf("invalid client secret")
	}
	return nil
}

// ClientCredentials implements op.ClientCredentialsStorage.
func (s *Storage) ClientCredentials(ctx context.Context, clientID, clientSecret string) (op.Client, error) {
	if err := s.AuthorizeClientIDSecret(ctx, clientID, clientSecret); err != nil {
		return nil, err
	}
	return s.GetClientByClientID(ctx, clientID)
}

// ClientCredentialsTokenRequest implements op.ClientCredentialsStorage.
func (s *Storage) ClientCredentialsTokenRequest(ctx context.Context, clientID string, scopes []string) (op.TokenRequest, error) {
	return &AuthRequest{
		ID:           uuid.NewString(),
		ClientID:     clientID,
		UserID:       clientID,
		Scopes:       scopes,
		AuthTime:     time.Now(),
		IsDone:       true,
		ResponseType: oidc.ResponseTypeCode,
	}, nil
}
