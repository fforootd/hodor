package api

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"

	"github.com/zitadel/zitadel/internal/schema"
)

func (a *API) resolveResourceSchema(ctx context.Context, schemaType, requestedSchemaID string) (*schema.SchemaRecord, error) {
	if strings.TrimSpace(requestedSchemaID) != "" {
		rec, err := schema.LoadSchemaRecord(ctx, a.db.SQL(), requestedSchemaID)
		if err != nil {
			return nil, err
		}
		if rec.Type != schemaType {
			return nil, fmt.Errorf("schema %q is type %q, not %q", rec.ID, rec.Type, schemaType)
		}
		return rec, nil
	}

	var rec schema.SchemaRecord
	err := a.db.SQL().QueryRowContext(ctx,
		`SELECT id, type, schema
		 FROM schemas
		 WHERE type = ? AND is_default = true
		 ORDER BY created_at ASC
		 LIMIT 1`,
		schemaType,
	).Scan(&rec.ID, &rec.Type, &rec.Schema)
	if err == nil {
		return &rec, nil
	}
	if err != nil && !sqlErrNoRows(err) {
		return nil, fmt.Errorf("load default %s schema: %w", schemaType, err)
	}

	err = a.db.SQL().QueryRowContext(ctx,
		`SELECT id, type, schema
		 FROM schemas
		 WHERE type = ?
		 ORDER BY version DESC, created_at ASC
		 LIMIT 1`,
		schemaType,
	).Scan(&rec.ID, &rec.Type, &rec.Schema)
	if err != nil {
		if sqlErrNoRows(err) {
			return nil, fmt.Errorf("no %s schema configured", schemaType)
		}
		return nil, fmt.Errorf("load fallback %s schema: %w", schemaType, err)
	}
	return &rec, nil
}

func sqlErrNoRows(err error) bool {
	return err != nil && strings.Contains(err.Error(), sql.ErrNoRows.Error())
}

func objectMapOrEmpty(value any) (map[string]any, error) {
	obj, err := schema.ObjectMap(value)
	if err != nil {
		return nil, err
	}
	if obj == nil {
		return map[string]any{}, nil
	}
	return obj, nil
}

func decodeObjectString(raw string) map[string]any {
	if strings.TrimSpace(raw) == "" {
		return map[string]any{}
	}
	var out map[string]any
	if err := json.Unmarshal([]byte(raw), &out); err != nil || out == nil {
		return map[string]any{}
	}
	return out
}

func encodeObjectString(value map[string]any) string {
	if len(value) == 0 {
		return "{}"
	}
	raw, err := json.Marshal(value)
	if err != nil {
		return "{}"
	}
	return string(raw)
}

func cloneObjectMap(src map[string]any) map[string]any {
	if src == nil {
		return map[string]any{}
	}
	dst := make(map[string]any, len(src))
	for key, value := range src {
		dst[key] = value
	}
	return dst
}

func stripKeys(input map[string]any, keys ...string) map[string]any {
	out := cloneObjectMap(input)
	for _, key := range keys {
		delete(out, key)
	}
	return out
}

func stringFromAny(value any) string {
	switch typed := value.(type) {
	case string:
		return strings.TrimSpace(typed)
	default:
		if typed == nil {
			return ""
		}
		return strings.TrimSpace(fmt.Sprint(typed))
	}
}

func stringSliceFromAny(value any) []string {
	switch typed := value.(type) {
	case nil:
		return nil
	case []string:
		return append([]string(nil), typed...)
	case []any:
		items := make([]string, 0, len(typed))
		for _, item := range typed {
			if text := stringFromAny(item); text != "" {
				items = append(items, text)
			}
		}
		return items
	case string:
		if strings.TrimSpace(typed) == "" {
			return nil
		}
		var decoded []string
		if err := json.Unmarshal([]byte(typed), &decoded); err == nil {
			return decoded
		}
		return []string{typed}
	default:
		return nil
	}
}

func appCanonicalData(
	name, description, appType string,
	redirectURIs, postLogoutRedirectURIs, grantTypes, responseTypes []string,
	logoURI string,
	metadata map[string]any,
) map[string]any {
	data := cloneObjectMap(metadata)
	if data == nil {
		data = map[string]any{}
	}
	if name != "" {
		data["client_name"] = name
	}
	if description != "" {
		data["description"] = description
	}
	if appType != "" {
		data["app_type"] = normalizeAppType(appType)
	}
	if len(redirectURIs) > 0 {
		data["redirect_uris"] = redirectURIs
	}
	if len(postLogoutRedirectURIs) > 0 {
		data["post_logout_redirect_uris"] = postLogoutRedirectURIs
	}
	if len(grantTypes) > 0 {
		data["grant_types"] = grantTypes
	}
	if len(responseTypes) > 0 {
		data["response_types"] = responseTypes
	}
	if logoURI != "" {
		data["logo_uri"] = logoURI
	}
	return data
}

func normalizeAppType(value string) string {
	switch strings.TrimSpace(value) {
	case "oidc":
		return "web"
	case "api":
		return "m2m"
	default:
		return strings.TrimSpace(value)
	}
}

func orgCanonicalData(name string, metadata map[string]any) map[string]any {
	data := cloneObjectMap(metadata)
	if data == nil {
		data = map[string]any{}
	}
	if name != "" {
		data["display_name"] = name
	}
	return data
}

func groupCanonicalData(name, description string, metadata map[string]any) map[string]any {
	data := cloneObjectMap(metadata)
	if data == nil {
		data = map[string]any{}
	}
	if name != "" {
		data["name"] = name
	}
	if description != "" {
		data["description"] = description
	}
	return data
}

func projectCanonicalData(name, description string, metadata map[string]any) map[string]any {
	data := cloneObjectMap(metadata)
	if data == nil {
		data = map[string]any{}
	}
	if name != "" {
		data["name"] = name
	}
	if description != "" {
		data["description"] = description
	}
	return data
}
