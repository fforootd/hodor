// Package config handles runtime configuration for the Zitadel server.
// Config is loaded from TOML files, environment variables, and CLI flags.
// Domain-specific configuration (policies, whitelabeling, etc.) lives in
// bootstrap.yaml, NOT here.
package config

import (
	"fmt"
	"os"
	"strconv"
	"strings"

	"github.com/BurntSushi/toml"
)

// Config is the runtime configuration for the Zitadel server.
// This covers infrastructure-level settings only (~30 total).
type Config struct {
	Server        ServerConfig        `toml:"server"`
	Database      DatabaseConfig      `toml:"database"`
	Observability ObservabilityConfig `toml:"observability"`
	Workers       WorkersConfig       `toml:"workers"`
	Dev           DevConfig           `toml:"dev"`
}

// ServerConfig controls HTTP server behavior.
type ServerConfig struct {
	Port           int      `toml:"port"`
	ExternalDomain string   `toml:"external_domain"`
	TLSCert        string   `toml:"tls_cert"`
	TLSKey         string   `toml:"tls_key"`
	CookieSecrets  []string `toml:"cookie_secrets"` // HMAC keys for session cookies; first signs, all verify
	OIDCEncryptionKey string `toml:"oidc_encryption_key"` // 32-byte hex-encoded key for OIDC token encryption

	// Sub-path deployment: host all routes under a prefix (e.g., "/auth").
	BasePath      string             `toml:"base_path"`
	PathOverrides PathOverrideConfig `toml:"path_overrides"` // per-app path overrides
	AppAccess     AppAccessConfig    `toml:"app_access"`     // per-app access control

	// Proxy trust: correctly resolve real client IP behind CDN/WAF/reverse proxies.
	TrustedProxies  []string `toml:"trusted_proxies"`     // CIDR ranges (e.g., ["10.0.0.0/8"])
	ProxyHeaderMode string   `toml:"proxy_header_mode"`   // "standard" | "cloudflare" | "custom"
	RealIPHeader    string   `toml:"real_ip_header"`      // custom header (e.g., "CF-Connecting-IP")

	// Security response headers.
	SecurityHeaders SecurityHeadersConfig `toml:"security_headers"`
}

// PathOverrideConfig allows individual app path prefixes to diverge from the global BasePath.
// Set a value to "/" to keep that app at the domain root. Leave empty to inherit BasePath.
type PathOverrideConfig struct {
	OIDC    string `toml:"oidc"`    // default: "/" when base_path is set
	SAML    string `toml:"saml"`    // default: "/" when base_path is set
	API     string `toml:"api"`     // default: "" (inherit)
	Login   string `toml:"login"`   // default: "" (inherit)
	Console string `toml:"console"` // default: "" (inherit)
	Assets  string `toml:"assets"`  // default: "" (inherit)
}

// AppAccessEntry controls access to an individual app.
type AppAccessEntry struct {
	Enabled bool     `toml:"enabled"`  // false = 404 for all requests
	IPAllow []string `toml:"ip_allow"` // CIDR ranges; empty = allow all
}

// AppAccessConfig controls per-app access restrictions.
type AppAccessConfig struct {
	Console AppAccessEntry `toml:"console"`
	Admin   AppAccessEntry `toml:"admin"`
	API     AppAccessEntry `toml:"api"`
	Login   AppAccessEntry `toml:"login"`
}

// SecurityHeadersConfig controls HTTP security response headers.
type SecurityHeadersConfig struct {
	HSTSEnabled    bool   `toml:"hsts_enabled"`    // default: true when TLS or external domain
	HSTSMaxAge     int    `toml:"hsts_max_age"`    // default: 63072000 (2 years)
	HSTSSubdomains bool   `toml:"hsts_subdomains"` // default: true
	HSTSPreload    bool   `toml:"hsts_preload"`    // default: false

	CSPEnabled  bool   `toml:"csp_enabled"`   // default: true
	CSPPolicy   string `toml:"csp_policy"`    // override entire policy string
	CSPReportURI string `toml:"csp_report_uri"` // URI for violation reports

	XFrameOptions       string `toml:"x_frame_options"`        // default: "DENY"
	XContentTypeOptions bool   `toml:"x_content_type_options"` // default: true (nosniff)
	ReferrerPolicy      string `toml:"referrer_policy"`        // default: "strict-origin-when-cross-origin"
	PermissionsPolicy   string `toml:"permissions_policy"`     // default: restrict camera, mic, etc.
	CrossOriginOpener   string `toml:"cross_origin_opener"`    // default: "same-origin"
}

// DatabaseConfig controls the primary database connection.
type DatabaseConfig struct {
	URL string `toml:"url"`
}

// ObservabilityConfig controls logging and telemetry.
type ObservabilityConfig struct {
	OTLPEndpoint string `toml:"otlp_endpoint"`
	LogLevel     string `toml:"log_level"`
	LogFormat    string `toml:"log_format"`
}

// WorkersConfig controls async background worker counts.
type WorkersConfig struct {
	NotificationWorkers int `toml:"notification_workers"`
	EventWorkers        int `toml:"event_workers"`
	LakeBatchSize       int `toml:"lake_batch_size"`
	LakeBatchWindowSecs int `toml:"lake_batch_window_secs"`
}

// DevConfig controls development and testing features.
type DevConfig struct {
	MockOIDC     bool   `toml:"mock_oidc"`      // Enable embedded mock OIDC identity provider
	MockOIDCPort int    `toml:"mock_oidc_port"` // Port for mock OIDC server (default: 9998)
	SeedFile     string `toml:"seed_file"`      // Path to YAML seed file loaded on startup
}

// Defaults returns a Config with sensible defaults for zero-config startup.
func Defaults() *Config {
	return &Config{
		Server: ServerConfig{
			Port:           8080,
			ExternalDomain: "localhost",
			AppAccess: AppAccessConfig{
				Console: AppAccessEntry{Enabled: true},
				Admin:   AppAccessEntry{Enabled: true},
				API:     AppAccessEntry{Enabled: true},
				Login:   AppAccessEntry{Enabled: true},
			},
			SecurityHeaders: SecurityHeadersConfig{
				HSTSEnabled:         true,
				HSTSMaxAge:          63072000,
				HSTSSubdomains:      true,
				CSPEnabled:          true,
				XFrameOptions:       "DENY",
				XContentTypeOptions: true,
				ReferrerPolicy:      "strict-origin-when-cross-origin",
				PermissionsPolicy:   "camera=(), microphone=(), geolocation=(), payment=()",
				CrossOriginOpener:   "same-origin",
			},
		},
		Database: DatabaseConfig{
			URL: "sqlite://./zitadel.db",
		},
		Observability: ObservabilityConfig{
			LogLevel:  "info",
			LogFormat: "text",
		},
		Workers: WorkersConfig{
			NotificationWorkers: 1,
			EventWorkers:        1,
			LakeBatchSize:       1000,
			LakeBatchWindowSecs: 5,
		},
	}
}

// Load reads config from a TOML file (optional) and overlays environment variables.
// If path is empty, only defaults + env vars are used.
func Load(path string) (*Config, error) {
	cfg := Defaults()

	if path != "" {
		if _, err := toml.DecodeFile(path, cfg); err != nil {
			return nil, fmt.Errorf("decode config file %s: %w", path, err)
		}
	}

	// Environment variable overrides (ZITADEL_ prefix).
	applyEnv(cfg)

	return cfg, nil
}

func applyEnv(cfg *Config) {
	if v := os.Getenv("ZITADEL_PORT"); v != "" {
		if port, err := strconv.Atoi(v); err == nil {
			cfg.Server.Port = port
		}
	}
	if v := os.Getenv("ZITADEL_EXTERNAL_DOMAIN"); v != "" {
		cfg.Server.ExternalDomain = v
	}
	if v := os.Getenv("ZITADEL_TLS_CERT"); v != "" {
		cfg.Server.TLSCert = v
	}
	if v := os.Getenv("ZITADEL_TLS_KEY"); v != "" {
		cfg.Server.TLSKey = v
	}
	if v := os.Getenv("ZITADEL_DATABASE_URL"); v != "" {
		cfg.Database.URL = v
	}
	if v := os.Getenv("ZITADEL_OTLP_ENDPOINT"); v != "" {
		cfg.Observability.OTLPEndpoint = v
	}
	if v := os.Getenv("ZITADEL_LOG_LEVEL"); v != "" {
		cfg.Observability.LogLevel = v
	}
	if v := os.Getenv("ZITADEL_LOG_FORMAT"); v != "" {
		cfg.Observability.LogFormat = v
	}
	// Dev / feature flags
	if v := os.Getenv("ZITADEL_MOCK_OIDC"); v == "true" || v == "1" {
		cfg.Dev.MockOIDC = true
	}
	if v := os.Getenv("ZITADEL_MOCK_OIDC_PORT"); v != "" {
		if port, err := strconv.Atoi(v); err == nil {
			cfg.Dev.MockOIDCPort = port
		}
	}
	if v := os.Getenv("ZITADEL_SEED_FILE"); v != "" {
		cfg.Dev.SeedFile = v
	}

	// Path-based deployment
	if v := os.Getenv("ZITADEL_BASE_PATH"); v != "" {
		cfg.Server.BasePath = v
	}

	// Proxy trust
	if v := os.Getenv("ZITADEL_TRUSTED_PROXIES"); v != "" {
		cfg.Server.TrustedProxies = splitCSV(v)
	}
	if v := os.Getenv("ZITADEL_PROXY_HEADER_MODE"); v != "" {
		cfg.Server.ProxyHeaderMode = v
	}
	if v := os.Getenv("ZITADEL_REAL_IP_HEADER"); v != "" {
		cfg.Server.RealIPHeader = v
	}
}

// splitCSV splits a comma-separated string into a slice, trimming whitespace.
func splitCSV(s string) []string {
	parts := []string{}
	for _, p := range strings.Split(s, ",") {
		p = strings.TrimSpace(p)
		if p != "" {
			parts = append(parts, p)
		}
	}
	return parts
}
