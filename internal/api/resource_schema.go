package api

import (
	"context"

	"github.com/zitadel/zitadel/internal/resourcedata"
	"github.com/zitadel/zitadel/internal/schema"
)

func (a *API) resolveResourceSchema(ctx context.Context, schemaType, requestedSchemaID string) (*schema.SchemaRecord, error) {
	return schema.ResolveSchemaForType(ctx, a.db.SQL(), schemaType, requestedSchemaID, a.db.Dialect())
}

func objectMapOrEmpty(value any) (map[string]any, error) {
	return resourcedata.ObjectMapOrEmpty(value)
}

func decodeObjectString(raw string) map[string]any {
	return resourcedata.DecodeObjectString(raw)
}

func encodeObjectString(value map[string]any) string {
	return resourcedata.EncodeObjectString(value)
}

func cloneObjectMap(src map[string]any) map[string]any {
	return resourcedata.CloneObjectMap(src)
}

func stripKeys(input map[string]any, keys ...string) map[string]any {
	return resourcedata.StripKeys(input, keys...)
}

func stringFromAny(value any) string {
	return resourcedata.StringFromAny(value)
}

func stringSliceFromAny(value any) []string {
	return resourcedata.StringSliceFromAny(value)
}

func firstNonEmptyString(values ...string) string {
	return resourcedata.FirstNonEmptyString(values...)
}

func appCanonicalData(
	name, description, appType string,
	redirectURIs, postLogoutRedirectURIs, grantTypes, responseTypes []string,
	logoURI string,
	metadata map[string]any,
) map[string]any {
	return resourcedata.AppCanonicalData(name, description, appType, redirectURIs, postLogoutRedirectURIs, grantTypes, responseTypes, logoURI, metadata)
}

func normalizeAppType(value string) string {
	return resourcedata.NormalizeAppType(value)
}

func orgCanonicalData(name string, metadata map[string]any) map[string]any {
	return resourcedata.OrgCanonicalData(name, metadata)
}

func groupCanonicalData(name, description string, metadata map[string]any) map[string]any {
	return resourcedata.GroupCanonicalData(name, description, metadata)
}

func projectCanonicalData(name, description string, metadata map[string]any) map[string]any {
	return resourcedata.ProjectCanonicalData(name, description, metadata)
}
