package oidcop

import (
	"encoding/json"
	"strings"
	"time"

	"github.com/zitadel/oidc/v3/pkg/oidc"
	"github.com/zitadel/oidc/v3/pkg/op"
)

// Client implements the op.Client interface, built from an identity + its schema's x-oidc config.
type Client struct {
	id                  string
	redirectURIs        []string
	postLogoutURIs      []string
	applicationType     op.ApplicationType
	authMethod          oidc.AuthMethod
	responseTypes       []oidc.ResponseType
	grantTypes          []oidc.GrantType
	accessTokenType     op.AccessTokenType
	idTokenLifetime     time.Duration
	accessTokenLifetime time.Duration
	clockSkew           time.Duration
	devMode             bool
}

// ClientFromIdentity builds a Client from an identity's data JSON and its schema's x-oidc annotation.
func ClientFromIdentity(clientID string, dataJSON string, schemaJSON string) (*Client, error) {
	c := &Client{
		id:                  clientID,
		applicationType:     op.ApplicationTypeWeb,
		authMethod:          oidc.AuthMethodNone,
		responseTypes:       []oidc.ResponseType{oidc.ResponseTypeCode},
		grantTypes:          []oidc.GrantType{oidc.GrantTypeCode},
		accessTokenType:     op.AccessTokenTypeBearer,
		idTokenLifetime:     1 * time.Hour,
		accessTokenLifetime: 5 * time.Minute,
		clockSkew:           0,
		devMode:             true,
	}

	// Parse schema JSON first for defaults, then let per-app data override them.
	if schemaJSON != "" {
		var schema map[string]any
		if err := json.Unmarshal([]byte(schemaJSON), &schema); err == nil {
			if xoidc, ok := schema["x-oidc"].(map[string]any); ok {
				if gts, ok := xoidc["grant_types"].([]any); ok {
					c.grantTypes = nil
					for _, gt := range gts {
						if s, ok := gt.(string); ok {
							c.grantTypes = append(c.grantTypes, oidc.GrantType(s))
						}
					}
				}
				if rts, ok := xoidc["response_types"].([]any); ok {
					c.responseTypes = nil
					for _, rt := range rts {
						if s, ok := rt.(string); ok {
							c.responseTypes = append(c.responseTypes, oidc.ResponseType(s))
						}
					}
				}
				if method, ok := xoidc["token_endpoint_auth_method"].(string); ok {
					switch method {
					case "client_secret_post":
						c.authMethod = oidc.AuthMethodPost
					case "client_secret_basic":
						c.authMethod = oidc.AuthMethodBasic
					case "private_key_jwt":
						c.authMethod = oidc.AuthMethodPrivateKeyJWT
					default:
						c.authMethod = oidc.AuthMethodNone
					}
				}
				if at, ok := xoidc["access_token_type"].(string); ok && at == "jwt" {
					c.accessTokenType = op.AccessTokenTypeJWT
				}
			}
		}
	}

	if dataJSON != "" && dataJSON != "{}" {
		var data map[string]any
		if err := json.Unmarshal([]byte(dataJSON), &data); err == nil {
			c.redirectURIs = stringSliceFromField(data["redirect_uris"])
			c.postLogoutURIs = stringSliceFromField(data["post_logout_redirect_uris"])

			switch strings.TrimSpace(stringValueFromField(data["app_type"])) {
			case "native":
				c.applicationType = op.ApplicationTypeNative
			case "spa":
				c.applicationType = op.ApplicationTypeUserAgent
			default:
				c.applicationType = op.ApplicationTypeWeb
			}

			if grantTypes := grantTypesFromField(data["grant_types"]); len(grantTypes) > 0 {
				c.grantTypes = grantTypes
			}
			if responseTypes := responseTypesFromField(data["response_types"]); len(responseTypes) > 0 {
				c.responseTypes = responseTypes
			}
		}
	}

	return c, nil
}

func stringValueFromField(value any) string {
	if s, ok := value.(string); ok {
		return s
	}
	return ""
}

func stringSliceFromField(value any) []string {
	values, ok := value.([]any)
	if !ok {
		return nil
	}

	result := make([]string, 0, len(values))
	for _, value := range values {
		if s, ok := value.(string); ok {
			result = append(result, s)
		}
	}
	return result
}

func grantTypesFromField(value any) []oidc.GrantType {
	values := stringSliceFromField(value)
	if len(values) == 0 {
		return nil
	}

	result := make([]oidc.GrantType, 0, len(values))
	for _, value := range values {
		result = append(result, oidc.GrantType(value))
	}
	return result
}

func responseTypesFromField(value any) []oidc.ResponseType {
	values := stringSliceFromField(value)
	if len(values) == 0 {
		return nil
	}

	result := make([]oidc.ResponseType, 0, len(values))
	for _, value := range values {
		result = append(result, oidc.ResponseType(value))
	}
	return result
}

// op.Client interface implementation

func (c *Client) GetID() string                       { return c.id }
func (c *Client) RedirectURIs() []string              { return c.redirectURIs }
func (c *Client) PostLogoutRedirectURIs() []string    { return c.postLogoutURIs }
func (c *Client) ApplicationType() op.ApplicationType { return c.applicationType }
func (c *Client) AuthMethod() oidc.AuthMethod         { return c.authMethod }
func (c *Client) ResponseTypes() []oidc.ResponseType  { return c.responseTypes }
func (c *Client) GrantTypes() []oidc.GrantType        { return c.grantTypes }
func (c *Client) LoginURL(authRequestID string) string {
	return "/login?auth_request_id=" + authRequestID
}
func (c *Client) AccessTokenType() op.AccessTokenType { return c.accessTokenType }
func (c *Client) IDTokenLifetime() time.Duration      { return c.idTokenLifetime }
func (c *Client) DevMode() bool                       { return c.devMode }

//nolint:staticcheck // method name defined by upstream oidc library
func (c *Client) RestrictAdditionalIdTokenScopes() func(scopes []string) []string {
	return func(s []string) []string { return s }
}
func (c *Client) RestrictAdditionalAccessTokenScopes() func(scopes []string) []string {
	return func(s []string) []string { return s }
}
func (c *Client) IsScopeAllowed(scope string) bool {
	return strings.HasPrefix(scope, "urn:") || scope == oidc.ScopeOpenID || scope == oidc.ScopeProfile || scope == oidc.ScopeEmail || scope == oidc.ScopePhone || scope == oidc.ScopeOfflineAccess
}
func (c *Client) IDTokenUserinfoClaimsAssertion() bool { return false }
func (c *Client) ClockSkew() time.Duration             { return c.clockSkew }
