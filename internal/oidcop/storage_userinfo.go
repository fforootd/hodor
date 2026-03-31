package oidcop

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"strings"
	"time"

	"github.com/zitadel/oidc/v3/pkg/oidc"
	"github.com/zitadel/oidc/v3/pkg/op"
	"golang.org/x/text/language"

	"github.com/zitadel/zitadel/internal/login"
)

func (s *Storage) SetUserinfoFromScopes(ctx context.Context, userinfo *oidc.UserInfo, userID, clientID string, scopes []string) error {
	return s.setUserinfo(ctx, userinfo, userID, scopes)
}

func (s *Storage) SetUserinfoFromRequest(ctx context.Context, userinfo *oidc.UserInfo, request op.IDTokenRequest, scopes []string) error {
	return s.setUserinfo(ctx, userinfo, request.GetSubject(), scopes)
}

func (s *Storage) SetUserinfoFromToken(ctx context.Context, userinfo *oidc.UserInfo, tokenID, subject, origin string) error {
	var expirationStr string
	var scopesStr string
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT scopes, expires_at FROM tokens WHERE id = ? AND type = 'oidc_access'`, tokenID,
	).Scan(&scopesStr, &expirationStr)
	if err != nil {
		return fmt.Errorf("token invalid")
	}
	exp, _ := time.Parse(time.RFC3339, expirationStr)
	if exp.Before(time.Now()) {
		return fmt.Errorf("token expired")
	}
	return s.setUserinfo(ctx, userinfo, subject, strings.Split(scopesStr, " "))
}

func (s *Storage) setUserinfo(ctx context.Context, userinfo *oidc.UserInfo, userID string, scopes []string) error {
	var identifier string
	var dataJSON, schemaJSON sql.NullString

	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT u.identifier, COALESCE(u.metadata, '{}'), COALESCE(sc.schema, '{}')
		 FROM users u
		 LEFT JOIN schemas sc ON u.schema_id = sc.id
		 WHERE u.identifier = ? OR u.id = ?`,
		userID, userID,
	).Scan(&identifier, &dataJSON, &schemaJSON)
	if err != nil {
		return fmt.Errorf("user not found: %w", err)
	}

	var data map[string]any
	if dataJSON.Valid {
		_ = json.Unmarshal([]byte(dataJSON.String), &data)
	}
	if data == nil {
		data = make(map[string]any)
	}

	claims := login.UserinfoClaims(schemaJSON.String, data)
	if claims == nil {
		claims = make(map[string]any)
	}

	for _, scope := range scopes {
		switch scope {
		case oidc.ScopeOpenID:
			userinfo.Subject = userID
		case oidc.ScopeEmail:
			if email, ok := claims["email"].(string); ok && email != "" {
				userinfo.Email = email
				userinfo.EmailVerified = oidc.Bool(true)
			} else if strings.Contains(identifier, "@") {
				userinfo.Email = identifier
				userinfo.EmailVerified = oidc.Bool(true)
			}
		case oidc.ScopeProfile:
			userinfo.PreferredUsername = identifier
			if name, ok := claims["name"].(string); ok {
				userinfo.Name = name
			}
			if gn, ok := claims["given_name"].(string); ok {
				userinfo.GivenName = gn
			}
			if fn, ok := claims["family_name"].(string); ok {
				userinfo.FamilyName = fn
			}
			if nick, ok := claims["nickname"].(string); ok {
				userinfo.Nickname = nick
			}
			if pic, ok := claims["picture"].(string); ok {
				userinfo.Picture = pic
			}
			if locale, ok := claims["locale"].(string); ok {
				userinfo.Locale = oidc.NewLocale(parseLocale(locale))
			}
			if zi, ok := claims["zoneinfo"].(string); ok {
				userinfo.Zoneinfo = zi
			}
		case oidc.ScopePhone:
			if phone, ok := claims["phone_number"].(string); ok && phone != "" {
				userinfo.PhoneNumber = phone
				userinfo.PhoneNumberVerified = oidc.Bool(true)
			}
		}
	}
	return nil
}

// parseLocale parses a BCP-47 language tag into a language.Tag.
// Returns language.Und on failure.
func parseLocale(s string) language.Tag {
	tag, err := language.Parse(s)
	if err != nil {
		return language.Und
	}
	return tag
}

func (s *Storage) SetIntrospectionFromToken(ctx context.Context, introspection *oidc.IntrospectionResponse, tokenID, subject, clientID string) error {
	var expirationStr, scopesStr, applicationID string
	err := s.db.SQL().QueryRowContext(ctx,
		`SELECT scopes, expires_at, application_id FROM tokens WHERE id = ? AND type = 'oidc_access'`, tokenID,
	).Scan(&scopesStr, &expirationStr, &applicationID)
	if err != nil {
		return fmt.Errorf("token invalid")
	}
	exp, _ := time.Parse(time.RFC3339, expirationStr)
	if exp.Before(time.Now()) {
		return fmt.Errorf("token expired")
	}

	introspection.Expiration = oidc.FromTime(exp)
	introspection.Scope = strings.Split(scopesStr, " ")
	introspection.ClientID = applicationID

	userinfo := new(oidc.UserInfo)
	if err := s.setUserinfo(ctx, userinfo, subject, introspection.Scope); err == nil {
		introspection.SetUserInfo(userinfo)
	}
	return nil
}

func (s *Storage) GetPrivateClaimsFromScopes(ctx context.Context, userID, clientID string, scopes []string) (map[string]any, error) {
	return map[string]any{}, nil
}
