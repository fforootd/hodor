// Package mgmt provides management secret authentication for the cloud
// control plane. When ZITADEL_MANAGEMENT_SECRET is configured, requests
// bearing that token bypass normal auth and receive full admin access.
package mgmt

import (
	"crypto/subtle"
	"net/http"
	"strings"
)

// ManagementUserID is the synthetic user identity for management requests.
const ManagementUserID = "_mgmt"

// Config holds the management secret for the cloud control plane.
type Config struct {
	Secret string // from ZITADEL_MANAGEMENT_SECRET
}

// IsEnabled returns true if a management secret is configured.
func (c *Config) IsEnabled() bool {
	return c != nil && c.Secret != ""
}

// IsManagementRequest checks if the request carries the management bearer token.
func (c *Config) IsManagementRequest(r *http.Request) bool {
	if !c.IsEnabled() {
		return false
	}
	auth := r.Header.Get("Authorization")
	if !strings.HasPrefix(auth, "Bearer ") {
		return false
	}
	token := strings.TrimPrefix(auth, "Bearer ")
	return subtle.ConstantTimeCompare([]byte(token), []byte(c.Secret)) == 1
}

// InjectManagementIdentity sets the standard identity headers for management requests.
func InjectManagementIdentity(r *http.Request) {
	r.Header.Set("X-Identity-Id", ManagementUserID)
	r.Header.Set("X-Token-Type", "management")
}
