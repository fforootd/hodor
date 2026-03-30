// Package ztls provides automatic TLS certificate management via CertMagic.
// It integrates with the endpoints table for on-demand certificate provisioning
// and uses the existing keys table for certificate storage.
package ztls

import (
	"context"
	"crypto/tls"
	"database/sql"
	"fmt"
	"net/http"

	"github.com/caddyserver/certmagic"

	"github.com/zitadel/zitadel/internal/config"
	"github.com/zitadel/zitadel/internal/logging"
)

// Manager wraps CertMagic and integrates with the endpoints table.
type Manager struct {
	magic  *certmagic.Config
	issuer *certmagic.ACMEIssuer
	db     *sql.DB
	mode   string // resolved TLS mode: auto, manual, external, off
	cfg    config.TLSConfig
}

// NewManager creates a TLS manager based on the resolved config mode.
func NewManager(tlsCfg config.TLSConfig, serverCfg *config.ServerConfig, db *sql.DB, isDev bool) (*Manager, error) {
	mode := tlsCfg.ResolveMode(serverCfg, isDev)

	m := &Manager{
		db:   db,
		mode: mode,
		cfg:  tlsCfg,
	}

	if mode != "auto" {
		logging.Printf("[tls] mode=%s (CertMagic disabled)", mode)
		return m, nil
	}

	// Configure CertMagic for auto mode.
	storage := &keysStorage{db: db}

	// Set the default storage globally before creating the config.
	certmagic.Default.Storage = storage

	cmCfg := certmagic.NewDefault()

	// On-demand TLS: only provision certs for DNS-verified endpoints.
	cmCfg.OnDemand = &certmagic.OnDemandConfig{
		DecisionFunc: func(ctx context.Context, name string) error {
			return m.allowDomain(ctx, name)
		},
	}

	// ACME issuer setup.
	issuer := certmagic.NewACMEIssuer(cmCfg, certmagic.ACMEIssuer{
		Email:  tlsCfg.Email,
		Agreed: true,
		CA:     resolveCA(tlsCfg.CADir),
	})
	cmCfg.Issuers = []certmagic.Issuer{issuer}

	m.magic = cmCfg
	m.issuer = issuer

	logging.Printf("[tls] mode=auto (CertMagic enabled, storage=keys table)")
	return m, nil
}

// Mode returns the resolved TLS mode.
func (m *Manager) Mode() string {
	return m.mode
}

// TLSConfig returns a *tls.Config managed by CertMagic.
// Returns nil if mode is not "auto".
func (m *Manager) TLSConfig() *tls.Config {
	if m.magic == nil {
		return nil
	}
	return m.magic.TLSConfig()
}

// HTTPChallengeHandler returns an HTTP handler that responds to ACME HTTP-01
// challenges and redirects all other traffic to HTTPS.
func (m *Manager) HTTPChallengeHandler(httpsPort int) http.Handler {
	if m.issuer == nil {
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			http.Error(w, "not available", http.StatusNotFound)
		})
	}

	// ACME issuer's HTTP challenge handler wraps a redirect-to-HTTPS handler.
	redirect := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		host := r.Host
		target := fmt.Sprintf("https://%s:%d%s", host, httpsPort, r.URL.RequestURI())
		if httpsPort == 443 {
			target = fmt.Sprintf("https://%s%s", host, r.URL.RequestURI())
		}
		http.Redirect(w, r, target, http.StatusMovedPermanently)
	})

	return m.issuer.HTTPChallengeHandler(redirect)
}

// allowDomain checks the endpoints table for a DNS-verified, auto-TLS endpoint.
func (m *Manager) allowDomain(ctx context.Context, name string) error {
	var count int
	err := m.db.QueryRowContext(ctx,
		`SELECT COUNT(*) FROM endpoints
		 WHERE domain = ? AND dns_verified = 1 AND tls_mode = 'auto' AND enabled = 1`,
		name,
	).Scan(&count)
	if err != nil || count == 0 {
		return fmt.Errorf("domain %q not allowed for auto-TLS", name)
	}
	return nil
}

// resolveCA returns the ACME CA directory URL.
func resolveCA(caDir string) string {
	if caDir != "" {
		return caDir
	}
	return certmagic.LetsEncryptProductionCA
}
