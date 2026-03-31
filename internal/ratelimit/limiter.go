// Package ratelimit implements the on_request pipeline stage rate limiter.
// It uses a token bucket algorithm with hierarchical settings resolution
// and expr-lang rule evaluation per ADR-009.
package ratelimit

import (
	"context"
	"database/sql"
	"fmt"
	"net"
	"strings"
	"time"

	"github.com/zitadel/zitadel/internal/settings"
)

// Store abstracts the rate limit state backend.
// The default implementation is in-memory (MemoryStore).
// Future implementations can use Redis, PostgreSQL, etc.
type Store interface {
	// Allow checks whether a request for the given key should be allowed.
	Allow(ctx context.Context, key string, limit int, burst int, window time.Duration) (Decision, error)
}

// Decision is the result of a rate limit check.
type Decision struct {
	Allowed    bool
	Remaining  int
	Limit      int
	ResetAt    time.Time
	RetryAfter time.Duration
}

// Config is the resolved rate limit configuration from settings.
type Config struct {
	RequestsPerMinute int
	Burst             int
	ByIP              bool
	ByUser            bool
	ByAPIKey          bool
	WhitelistIPs      []*net.IPNet
	CustomHeaders     bool
}

// DefaultConfig returns the defaults matching rate_limit.json schema defaults.
func DefaultConfig() *Config {
	return &Config{
		RequestsPerMinute: 1000,
		Burst:             50,
		ByIP:              true,
		ByUser:            false,
		ByAPIKey:          true,
		CustomHeaders:     true,
	}
}

// Limiter orchestrates rate limiting with settings resolution and rule evaluation.
type Limiter struct {
	store   Store
	db      *sql.DB
	actions *ActionEngine
}

// New creates a new Limiter with the given store and database.
func New(store Store, db *sql.DB) *Limiter {
	return &Limiter{
		store:   store,
		db:      db,
		actions: NewActionEngine(db),
	}
}

// Check performs a rate limit check for the given request context.
// It resolves settings, checks whitelists, and consults the token bucket store.
func (l *Limiter) Check(ctx context.Context, clientIP, orgID, appID string) (Decision, error) {
	cfg, err := l.resolveConfig(ctx, orgID, appID)
	if err != nil {
		// Fail open: if settings can't be resolved, allow the request.
		return Decision{Allowed: true, Limit: 0}, nil //nolint:nilerr // fail-open by design
	}

	// Disabled: rpm=0 means unlimited.
	if cfg.RequestsPerMinute == 0 {
		return Decision{Allowed: true, Limit: 0}, nil
	}

	// Whitelist check.
	if isWhitelisted(clientIP, cfg.WhitelistIPs) {
		return Decision{Allowed: true, Limit: cfg.RequestsPerMinute}, nil
	}

	// Build the rate limit key based on configuration.
	key := l.buildKey(cfg, clientIP, orgID)

	return l.store.Allow(ctx, key, cfg.RequestsPerMinute, cfg.Burst, time.Minute)
}

// resolveConfig loads the effective rate_limit settings from the hierarchy.
func (l *Limiter) resolveConfig(ctx context.Context, orgID, appID string) (*Config, error) {
	data, err := settings.Resolve(ctx, l.db, "rate_limit", orgID, appID)
	if err != nil {
		return nil, err
	}

	cfg := DefaultConfig()
	if len(data) == 0 {
		return cfg, nil
	}

	if v, ok := data["requests_per_minute"].(float64); ok {
		cfg.RequestsPerMinute = int(v)
	}
	if v, ok := data["burst"].(float64); ok {
		cfg.Burst = int(v)
	}
	if v, ok := data["by_ip"].(bool); ok {
		cfg.ByIP = v
	}
	if v, ok := data["by_user"].(bool); ok {
		cfg.ByUser = v
	}
	if v, ok := data["by_api_key"].(bool); ok {
		cfg.ByAPIKey = v
	}
	if v, ok := data["custom_headers"].(bool); ok {
		cfg.CustomHeaders = v
	}
	if v, ok := data["whitelist_ips"].([]any); ok {
		for _, item := range v {
			if cidr, ok := item.(string); ok {
				_, ipNet, err := net.ParseCIDR(cidr)
				if err != nil {
					// Try as plain IP.
					ip := net.ParseIP(cidr)
					if ip != nil {
						var mask net.IPMask
						if ip.To4() != nil {
							mask = net.CIDRMask(32, 32)
						} else {
							mask = net.CIDRMask(128, 128)
						}
						ipNet = &net.IPNet{IP: ip, Mask: mask}
					} else {
						continue
					}
				}
				cfg.WhitelistIPs = append(cfg.WhitelistIPs, ipNet)
			}
		}
	}

	return cfg, nil
}

// buildKey constructs the rate limit bucket key from the config and request.
func (l *Limiter) buildKey(cfg *Config, clientIP, orgID string) string {
	key := "rl"
	if orgID != "" {
		key += ":" + orgID
	}
	if cfg.ByIP && clientIP != "" {
		key += ":ip:" + clientIP
	}
	return key
}

// isWhitelisted checks if the client IP is in the whitelist.
func isWhitelisted(clientIP string, whitelist []*net.IPNet) bool {
	if len(whitelist) == 0 {
		return false
	}
	ip := net.ParseIP(clientIP)
	if ip == nil {
		return false
	}
	for _, cidr := range whitelist {
		if cidr.Contains(ip) {
			return true
		}
	}
	return false
}

// exemptPaths are paths excluded from rate limiting (health checks, etc.).
var exemptPaths = map[string]bool{
	"/healthz": true,
	"/readyz":  true,
}

// IsExempt returns true if the given path should skip rate limiting.
func IsExempt(path string) bool {
	if strings.HasPrefix(path, "/assets/") {
		return true
	}
	return exemptPaths[path]
}

// FormatRetryAfter formats a duration as seconds for the Retry-After header.
func FormatRetryAfter(d time.Duration) string {
	secs := int(d.Seconds())
	if secs < 1 {
		secs = 1
	}
	return fmt.Sprintf("%d", secs)
}
