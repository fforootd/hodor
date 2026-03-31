package login

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"net/url"
	"strings"

	"github.com/zitadel/zitadel/internal/id"
	providers "github.com/zitadel/zitadel/internal/provider"
	"github.com/zitadel/zitadel/internal/schema"
	"github.com/zitadel/zitadel/internal/uniqueness"
)

func (h *Handler) findOrCreateLinkedIdentity(ctx context.Context, prov providers.Provider, externalSub, externalEmail string, claims map[string]any) (string, error) {
	var userID string
	err := h.db.SQL().QueryRowContext(ctx,
		`SELECT user_id FROM linked_identities WHERE provider_id = ? AND external_sub = ?`,
		prov.ID, externalSub,
	).Scan(&userID)
	if err == nil {
		claimsJSON, _ := json.Marshal(claims)
		_, _ = h.db.SQL().ExecContext(ctx,
			`UPDATE linked_identities SET last_used_at = datetime('now'), raw_claims = ?, external_email = ? WHERE provider_id = ? AND external_sub = ?`,
			string(claimsJSON), externalEmail, prov.ID, externalSub,
		)
		return userID, nil
	}

	if linkedUserID, ok := h.findLinkableIdentity(ctx, prov.Linking, externalEmail, externalSub, claims); ok {
		claimsJSON, _ := json.Marshal(claims)
		linkID := id.New()
		if _, linkErr := h.db.SQL().ExecContext(ctx,
			`INSERT INTO linked_identities (id, user_id, provider_id, external_sub, external_email, raw_claims, linked_at)
			 VALUES (?, ?, ?, ?, ?, ?, datetime('now'))`,
			linkID, linkedUserID, prov.ID, externalSub, externalEmail, string(claimsJSON),
		); linkErr == nil {
			return linkedUserID, nil
		}
	}

	if prov.Linking.Mode == providers.LinkModeLinkOnly {
		return "", fmt.Errorf("no linked account found and provider is configured for link_only")
	}

	targetSchemaID, _, err := providers.ResolveTargetSchema(ctx, h.db.SQL(), prov.Target)
	if err != nil {
		return "", err
	}
	schemaRec, err := schema.LoadSchemaRecord(ctx, h.db.SQL(), targetSchemaID)
	if err != nil {
		return "", fmt.Errorf("resolve target schema %s: %w", targetSchemaID, err)
	}

	profile, _ := MapClaims(schemaRec.Schema, prov.Mapping.Claims, claims)

	displayName := ""
	if dn, ok := profile["display_name"].(string); ok {
		displayName = dn
	}
	if displayName == "" {
		displayName = externalEmail
	}
	if displayName == "" {
		displayName = externalSub
	}

	identifier := externalEmail
	if prov.Linking.MatchBy == providers.LinkMatchIdentifier && identifier == "" {
		identifier = externalSub
	}
	if identifier == "" {
		identifier = externalSub
	}

	newID := id.New()
	payload := schema.MaterializeUserData(schemaRec.Schema, identifier, displayName, profile)
	if err := schema.ValidateData(schemaRec.Schema, payload); err != nil {
		return "", fmt.Errorf("validate identity against %s: %w", schemaRec.ID, err)
	}

	profileJSON, _ := json.Marshal(profile)

	tx, err := h.db.SQL().BeginTx(ctx, nil)
	if err != nil {
		return "", fmt.Errorf("begin identity create: %w", err)
	}
	defer tx.Rollback()

	_, err = tx.ExecContext(ctx,
		`INSERT INTO users (id, org_id, identifier, display_name, state, schema_id, metadata, created_at, updated_at)
		 VALUES (?, 1, ?, ?, 'active', ?, ?, datetime('now'), datetime('now'))`,
		newID, identifier, displayName, schemaRec.ID, string(profileJSON),
	)
	if err != nil {
		return "", fmt.Errorf("create identity: %w", err)
	}
	if err := uniqueness.EnforceFromIdentifier(ctx, tx, newID, "1", identifier); err != nil {
		return "", err
	}
	if err := uniqueness.Enforce(ctx, tx, newID, "1", uniqueness.ExtractConstraints(schemaRec.Schema), payload); err != nil {
		return "", err
	}

	linkID := id.New()
	claimsJSON, _ := json.Marshal(claims)
	_, err = tx.ExecContext(ctx,
		`INSERT INTO linked_identities (id, user_id, provider_id, external_sub, external_email, raw_claims, linked_at)
		 VALUES (?, ?, ?, ?, ?, ?, datetime('now'))`,
		linkID, newID, prov.ID, externalSub, externalEmail, string(claimsJSON),
	)
	if err != nil {
		return "", fmt.Errorf("create linked account: %w", err)
	}
	if err := tx.Commit(); err != nil {
		return "", fmt.Errorf("commit identity create: %w", err)
	}

	return newID, nil
}

func (h *Handler) findLinkableIdentity(ctx context.Context, linking providers.Linking, externalEmail, externalSub string, claims map[string]any) (string, bool) {
	switch linking.MatchBy {
	case providers.LinkMatchVerifiedEmail:
		if externalEmail == "" || !claimBool(claims["email_verified"]) {
			return "", false
		}
		var userID string
		err := h.db.SQL().QueryRowContext(ctx, `SELECT id FROM users WHERE identifier = ?`, externalEmail).Scan(&userID)
		return userID, err == nil
	case providers.LinkMatchIdentifier:
		candidate := externalEmail
		if candidate == "" {
			candidate = externalSub
		}
		if candidate == "" {
			return "", false
		}
		var userID string
		err := h.db.SQL().QueryRowContext(ctx, `SELECT id FROM users WHERE identifier = ?`, candidate).Scan(&userID)
		return userID, err == nil
	default:
		return "", false
	}
}

func (h *Handler) ssoSuccessRedirect(ctx context.Context, r *http.Request, flowID, userID string) (string, error) {
	if strings.TrimSpace(flowID) == "" {
		return loginExitURL("sso_success", ""), nil
	}

	flow, ok := h.flows.Get(flowID)
	if !ok {
		return loginExitURL("sso_success", ""), nil
	}
	defer h.flows.Delete(flowID)

	if flow.AuthRequestID != "" {
		if err := h.completeOIDCAuthRequest(ctx, flow.AuthRequestID, userID); err != nil {
			return "", err
		}
		return h.oidcAuthorizeCallbackURL(flow.AuthRequestID), nil
	}

	return loginExitURL("sso_success", sanitizeContinueTo(r, flow.RedirectURI)), nil
}

func loginExitURL(exit, continueTo string) string {
	values := url.Values{}
	values.Set("exit", exit)
	if continueTo != "" {
		values.Set("continue_to", continueTo)
	}
	return "/login?" + values.Encode()
}
