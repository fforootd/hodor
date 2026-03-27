package oidcop

import (
	"time"

	"github.com/zitadel/oidc/v3/pkg/oidc"
)

// AuthRequest implements the op.AuthRequest interface.
type AuthRequest struct {
	ID            string
	ClientID      string
	RedirectURI   string
	Scopes        []string
	State         string
	Nonce         string
	ResponseType  oidc.ResponseType
	CodeChallenge *oidc.CodeChallenge
	UserID        string
	AuthTime      time.Time
	IsDone        bool
	CreatedAt     time.Time
}

// op.AuthRequest interface implementation

func (a *AuthRequest) GetID() string            { return a.ID }
func (a *AuthRequest) GetACR() string           { return "" }
func (a *AuthRequest) GetAMR() []string         { return nil }
func (a *AuthRequest) GetAudience() []string     { return []string{a.ClientID} }
func (a *AuthRequest) GetAuthTime() time.Time   { return a.AuthTime }
func (a *AuthRequest) GetClientID() string      { return a.ClientID }
func (a *AuthRequest) GetCodeChallenge() *oidc.CodeChallenge { return a.CodeChallenge }
func (a *AuthRequest) GetNonce() string         { return a.Nonce }
func (a *AuthRequest) GetRedirectURI() string   { return a.RedirectURI }
func (a *AuthRequest) GetResponseType() oidc.ResponseType { return a.ResponseType }
func (a *AuthRequest) GetResponseMode() oidc.ResponseMode { return "" }
func (a *AuthRequest) GetScopes() []string      { return a.Scopes }
func (a *AuthRequest) GetState() string         { return a.State }
func (a *AuthRequest) GetSubject() string       { return a.UserID }
func (a *AuthRequest) Done() bool               { return a.IsDone }
