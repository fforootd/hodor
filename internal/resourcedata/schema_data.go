package resourcedata

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/zitadel/zitadel/internal/schema"
)

func ObjectMapOrEmpty(value any) (map[string]any, error) {
	obj, err := schema.ObjectMap(value)
	if err != nil {
		return nil, err
	}
	if obj == nil {
		return map[string]any{}, nil
	}
	return obj, nil
}

func DecodeObjectString(raw string) map[string]any {
	if strings.TrimSpace(raw) == "" {
		return map[string]any{}
	}
	var out map[string]any
	if err := json.Unmarshal([]byte(raw), &out); err != nil || out == nil {
		return map[string]any{}
	}
	return out
}

func EncodeObjectString(value map[string]any) string {
	if len(value) == 0 {
		return "{}"
	}
	raw, err := json.Marshal(value)
	if err != nil {
		return "{}"
	}
	return string(raw)
}

func CloneObjectMap(src map[string]any) map[string]any {
	if src == nil {
		return map[string]any{}
	}
	dst := make(map[string]any, len(src))
	for key, value := range src {
		dst[key] = value
	}
	return dst
}

func StripKeys(input map[string]any, keys ...string) map[string]any {
	out := CloneObjectMap(input)
	for _, key := range keys {
		delete(out, key)
	}
	return out
}

func StringFromAny(value any) string {
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

func StringSliceFromAny(value any) []string {
	switch typed := value.(type) {
	case nil:
		return nil
	case []string:
		return append([]string(nil), typed...)
	case []any:
		items := make([]string, 0, len(typed))
		for _, item := range typed {
			if text := StringFromAny(item); text != "" {
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

func FirstNonEmptyString(values ...string) string {
	for _, value := range values {
		if strings.TrimSpace(value) != "" {
			return strings.TrimSpace(value)
		}
	}
	return ""
}

func NormalizeAppType(value string) string {
	switch strings.TrimSpace(value) {
	case "oidc":
		return "web"
	case "api":
		return "m2m"
	default:
		return strings.TrimSpace(value)
	}
}

func AppCanonicalData(
	name, description, appType string,
	redirectURIs, postLogoutRedirectURIs, grantTypes, responseTypes []string,
	logoURI string,
	metadata map[string]any,
) map[string]any {
	data := CloneObjectMap(metadata)
	if name != "" {
		data["client_name"] = name
	}
	if description != "" {
		data["description"] = description
	}
	if appType != "" {
		data["app_type"] = NormalizeAppType(appType)
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

func OrgCanonicalData(name string, metadata map[string]any) map[string]any {
	data := CloneObjectMap(metadata)
	if name != "" {
		data["display_name"] = name
	}
	return data
}

func GroupCanonicalData(name, description string, metadata map[string]any) map[string]any {
	data := CloneObjectMap(metadata)
	if name != "" {
		data["name"] = name
	}
	if description != "" {
		data["description"] = description
	}
	return data
}

func ProjectCanonicalData(name, description string, metadata map[string]any) map[string]any {
	data := CloneObjectMap(metadata)
	if name != "" {
		data["name"] = name
	}
	if description != "" {
		data["description"] = description
	}
	return data
}

func BuildLoginFlowSchemaData(name, strategy string, isDefault bool, state string, priority int, audience, authMethods, configValue any, metadata map[string]any) (map[string]any, map[string]any, error) {
	configMap, err := ObjectMapOrEmpty(configValue)
	if err != nil {
		return nil, nil, err
	}

	data := CloneObjectMap(metadata)
	if name != "" {
		data["display_name"] = name
	}
	if strategy != "" {
		data["strategy"] = strategy
	}
	data["is_default"] = isDefault
	if state != "" {
		data["state"] = state
	}
	data["priority"] = priority
	if audience != nil {
		data["audience"] = audience
	}
	if authMethods != nil {
		data["auth_methods"] = authMethods
	}
	if len(configMap) > 0 {
		data["config"] = configMap
		for key, value := range configMap {
			data[key] = value
		}
	}
	return data, configMap, nil
}

func BuildActionSchemaData(name, hook, actionType, trigger string, priority int, enabled bool, configValue any, metadata map[string]any) (map[string]any, map[string]any, error) {
	configMap, err := ObjectMapOrEmpty(configValue)
	if err != nil {
		return nil, nil, err
	}

	data := CloneObjectMap(metadata)
	if name != "" {
		data["display_name"] = name
	}
	if hook != "" {
		data["hook"] = hook
	}
	if actionType != "" {
		data["action_type"] = actionType
	}
	if trigger != "" {
		data["trigger"] = trigger
	}
	data["priority"] = priority
	data["enabled"] = enabled
	data["config"] = configMap
	return data, configMap, nil
}
