// Package config handles runtime configuration for the Zitadel server.
// Config is loaded from TOML files, environment variables, and CLI flags.
// Domain-specific configuration (policies, whitelabeling, etc.) lives in
// bootstrap.yaml, NOT here.
package config

import (
	"encoding/json"
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
	TLS           TLSConfig           `toml:"tls"`
	Encryption    EncryptionConfig    `toml:"encryption"`
	Database      DatabaseConfig      `toml:"database"`
	Observability ObservabilityConfig `toml:"observability"`
	Workers       WorkersConfig       `toml:"workers"`
	RateLimit     RateLimitConfig     `toml:"rate_limit"`
	Catalog       CatalogConfig       `toml:"catalog"`
	Dev           DevConfig           `toml:"dev"`
}

// ServerConfig controls HTTP server behavior.
type ServerConfig struct {
	Port           int      `toml:"port"`
	ExternalDomain string   `toml:"external_domain"`
	TLSCert        string   `toml:"tls_cert"`
	TLSKey         string   `toml:"tls_key"`
	CookieSecrets  []string `toml:"cookie_secrets"` // HMAC keys for session cookies; first signs, all verify

	// Sub-path deployment: host all routes under a prefix (e.g., "/auth").
	BasePath      string             `toml:"base_path"`
	PathOverrides PathOverrideConfig `toml:"path_overrides"` // per-app path overrides
	AppAccess     AppAccessConfig    `toml:"app_access"`     // per-app access control

	// Proxy trust: correctly resolve real client IP behind CDN/WAF/reverse proxies.
	TrustedProxies  []string `toml:"trusted_proxies"`   // CIDR ranges (e.g., ["10.0.0.0/8"])
	ProxyHeaderMode string   `toml:"proxy_header_mode"` // "standard" | "cloudflare" | "custom"
	RealIPHeader    string   `toml:"real_ip_header"`    // custom header (e.g., "CF-Connecting-IP")

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
	HSTSEnabled    bool `toml:"hsts_enabled"`    // default: true when TLS or external domain
	HSTSMaxAge     int  `toml:"hsts_max_age"`    // default: 63072000 (2 years)
	HSTSSubdomains bool `toml:"hsts_subdomains"` // default: true
	HSTSPreload    bool `toml:"hsts_preload"`    // default: false

	CSPEnabled   bool   `toml:"csp_enabled"`    // default: true
	CSPPolicy    string `toml:"csp_policy"`     // override entire policy string
	CSPReportURI string `toml:"csp_report_uri"` // URI for violation reports

	XFrameOptions       string `toml:"x_frame_options"`        // default: "DENY"
	XContentTypeOptions bool   `toml:"x_content_type_options"` // default: true (nosniff)
	ReferrerPolicy      string `toml:"referrer_policy"`        // default: "strict-origin-when-cross-origin"
	PermissionsPolicy   string `toml:"permissions_policy"`     // default: restrict camera, mic, etc.
	CrossOriginOpener   string `toml:"cross_origin_opener"`    // default: "same-origin"
}

// TLSConfig controls automatic TLS certificate provisioning via CertMagic.
type TLSConfig struct {
	// Mode controls how TLS is handled:
	//   "auto"     — CertMagic provisions certs via Let's Encrypt (default when external_domain is public)
	//   "manual"   — use tls_cert/tls_key from ServerConfig
	//   "external" — TLS terminated upstream (reverse proxy, load balancer)
	//   "off"      — plain HTTP only (default in dev)
	Mode      string `toml:"mode"`
	Email     string `toml:"email"`      // ACME account email (for auto mode)
	CADir     string `toml:"ca_dir"`     // Custom ACME CA directory URL (optional, default: Let's Encrypt production)
	HTTPPort  int    `toml:"http_port"`  // Port for HTTP redirect + ACME challenges (default: 80)
	HTTPSPort int    `toml:"https_port"` // Port for HTTPS (default: 443)
}

// EncryptionConfig controls application-level envelope encryption (AES-256-GCM)
// for secrets stored in the database (signing keys, TLS certs, IdP secrets).
//
// When no keys are configured, the system runs in plaintext mode (dev default).
// On first boot in dev mode, a random key is auto-generated.
type EncryptionConfig struct {
	// ActiveKeyID selects which key from the ring encrypts new writes.
	ActiveKeyID string `toml:"active_key_id"`

	// Keys is the full key ring. Old keys remain for decryption during rotation.
	Keys []EncryptionKey `toml:"keys"`
}

// EncryptionKey is a named 32-byte symmetric key for AES-256-GCM.
type EncryptionKey struct {
	ID     string `json:"id"     toml:"id"`     // e.g. "key_v1_2025"
	Secret string `json:"secret" toml:"secret"` // 64-char hex string (32 bytes)
}

// KeyMap returns the key ring as a map[id]hexSecret suitable for crypto.NewSecretBox.
func (e *EncryptionConfig) KeyMap() map[string]string {
	m := make(map[string]string, len(e.Keys))
	for _, k := range e.Keys {
		m[k.ID] = k.Secret
	}
	return m
}

// ResolveMode determines the effective TLS mode from config + server state.
// Zero-config: when external_domain is set and public, default to "auto".
// Dev mode or localhost: default to "off".
func (t *TLSConfig) ResolveMode(serverCfg *ServerConfig, isDev bool) string {
	// Explicit mode always wins.
	switch t.Mode {
	case "auto", "manual", "external", "off":
		return t.Mode
	}

	// Manual certs provided → manual mode.
	if serverCfg.TLSCert != "" && serverCfg.TLSKey != "" {
		return "manual"
	}

	// Dev mode → off.
	if isDev {
		return "off"
	}

	// Public domain set → auto.
	d := serverCfg.ExternalDomain
	if d != "" && d != "localhost" && d != "127.0.0.1" && d != "0.0.0.0" {
		return "auto"
	}

	return "off"
}

// ResolveHTTPPort returns the effective HTTP port.
func (t *TLSConfig) ResolveHTTPPort() int {
	if t.HTTPPort > 0 {
		return t.HTTPPort
	}
	return 80
}

// ResolveHTTPSPort returns the effective HTTPS port.
func (t *TLSConfig) ResolveHTTPSPort() int {
	if t.HTTPSPort > 0 {
		return t.HTTPSPort
	}
	return 443
}

// DatabaseConfig controls the primary database connection and startup lifecycle.
type DatabaseConfig struct {
	URL string `toml:"url"`

	// Migrate controls schema migration behavior on 'zitadel start'.
	//   "auto"  — run migrations before serving (default for all dialects)
	//   "check" — verify version only, fail if behind (opt-in for production PG)
	//   "skip"  — no version check, no migration (fastest cold-start)
	//   ""      — same as "auto" (consistent default)
	Migrate string `toml:"migrate"`

	// Bootstrap controls admin/schema bootstrapping on 'zitadel start'.
	//   "auto"  — run bootstrap if no entities exist (default)
	//   "skip"  — never bootstrap (production: admin created via 'zitadel migrate --bootstrap')
	//   ""      — same as "auto"
	Bootstrap string `toml:"bootstrap"`

	// Connection pool settings (Postgres only; ignored for SQLite).
	MaxOpenConns    int    `toml:"max_open_conns"`    // default: 25
	MaxIdleConns    int    `toml:"max_idle_conns"`    // default: 5
	ConnMaxLifetime string `toml:"conn_max_lifetime"` // default: "1h" (duration string)
}

// ResolveMigrateMode returns the effective migration mode.
// Empty string defaults to "auto" (consistent for all dialects).
func (c *DatabaseConfig) ResolveMigrateMode() string {
	switch c.Migrate {
	case "auto", "check", "skip":
		return c.Migrate
	default:
		return "auto"
	}
}

// ResolveBootstrapMode returns the effective bootstrap mode.
// Empty string defaults to "auto".
func (c *DatabaseConfig) ResolveBootstrapMode() string {
	switch c.Bootstrap {
	case "auto", "skip":
		return c.Bootstrap
	default:
		return "auto"
	}
}

// ObservabilityConfig controls logging, telemetry, and log routing.
type ObservabilityConfig struct {
	LogLevel  string              `toml:"log_level"`
	LogFormat string              `toml:"log_format"`
	CachePath string              `toml:"cache_path"` // local SQLite cache file (default: "zitadel-cache.db")
	CacheMax  int                 `toml:"cache_max"`  // ring buffer max rows (default: 50000, 0 = unlimited)
	Streams   StreamRoutingConfig `toml:"streams"`
	Sinks     SinksConfig         `toml:"sinks"`
	Redaction RedactionConfig     `toml:"redaction"`
}

// StreamConfig configures a single log stream.
type StreamConfig struct {
	Sinks      []string `toml:"sinks"`       // ["stdout", "otel", "analytics"]
	Mode       string   `toml:"mode"`        // "buffered" | "sampled" | "off"
	SampleRate float64  `toml:"sample_rate"` // for "sampled" mode (e.g., 0.01 = 1%)
}

// StreamRoutingConfig maps each log stream to its configuration.
// Omitted streams inherit sensible defaults.
type StreamRoutingConfig struct {
	Runtime     StreamConfig `toml:"runtime"`
	Request     StreamConfig `toml:"request"`
	Jobs        StreamConfig `toml:"jobs"`
	EventPusher StreamConfig `toml:"event_pusher"`
}

// SinksConfig holds per-sink configuration.
type SinksConfig struct {
	OTEL      OTELSinkConfig      `toml:"otel"`
	Analytics AnalyticsSinkConfig `toml:"analytics"`
}

// OTELSinkConfig configures the OTEL log exporter.
type OTELSinkConfig struct {
	Endpoint string `toml:"endpoint"` // OTLP endpoint (empty = disabled)
	Protocol string `toml:"protocol"` // "grpc" | "http"
}

// AnalyticsSinkConfig configures the analytics log sink.
type AnalyticsSinkConfig struct {
	Enabled       bool   `toml:"enabled"`        // writes log events to cache → events table
	DrainInterval string `toml:"drain_interval"` // how often to flush cache (default: "5s")
	DrainBatch    int    `toml:"drain_batch"`    // rows per flush (default: 500)
}

// RedactionConfig controls masking of sensitive fields in log output.
type RedactionConfig struct {
	Keys   []string `toml:"keys"`    // field key fragments to mask (case-insensitive)
	Mask   string   `toml:"mask"`    // replacement string (default: "***REDACTED***")
	IPMode string   `toml:"ip_mode"` // IP address handling: "keep" | "redact" | "hash" | "mask"
}

// WorkersConfig controls async background worker counts.
type WorkersConfig struct {
	NotificationWorkers int `toml:"notification_workers"`
	EventWorkers        int `toml:"event_workers"`
	LakeBatchSize       int `toml:"lake_batch_size"`
	LakeBatchWindowSecs int `toml:"lake_batch_window_secs"`
}

// RateLimitConfig controls the rate limiter backend and behavior.
// Backend options mirror the analytics pattern: "memory" (default), "sql", "redis".
type RateLimitConfig struct {
	Backend    string `toml:"backend"`     // "memory" (default) | "sql" | "redis"
	RedisURL   string `toml:"redis_url"`   // Redis connection URL (only when backend="redis")
	GCInterval int    `toml:"gc_interval"` // Bucket cleanup interval in seconds (memory backend, default: 60)
	BatchWrite bool   `toml:"batch_write"` // Batch counter updates to DB instead of per-request (sql backend)
}

// CatalogConfig controls the template catalog source (ADR-015).
// Templates (actions, providers, FGA models, schemas) are loaded from a git
// repository or local directory.
type CatalogConfig struct {
	URL             string `toml:"url"`              // Git repo URL for catalog (default: official zitadel catalog)
	LocalPath       string `toml:"local_path"`       // Local directory override (dev/air-gapped)
	RefreshInterval string `toml:"refresh_interval"` // How often to refresh (default: "1h", "0" = manual)
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
			URL:             "sqlite://./zitadel.db",
			Migrate:         "", // auto-detect: "auto" for all dialects
			Bootstrap:       "", // auto-detect: "auto" for all dialects
			MaxOpenConns:    25,
			MaxIdleConns:    5,
			ConnMaxLifetime: "1h",
		},
		Observability: ObservabilityConfig{
			LogLevel:  "info",
			LogFormat: "text",
			CachePath: "zitadel-cache.db",
			CacheMax:  50000,
			Streams: StreamRoutingConfig{
				Runtime: StreamConfig{
					Sinks: []string{"stdout", "analytics"},
					Mode:  "buffered",
				},
				Request: StreamConfig{
					Sinks: []string{"stdout", "otel", "analytics"},
					Mode:  "buffered",
				},
				Jobs: StreamConfig{
					Sinks: []string{"stdout", "analytics"},
					Mode:  "buffered",
				},
				EventPusher: StreamConfig{
					Mode: "off",
				},
			},
			Sinks: SinksConfig{
				Analytics: AnalyticsSinkConfig{
					Enabled:       true,
					DrainInterval: "5s",
					DrainBatch:    500,
				},
			},
			Redaction: RedactionConfig{
				Keys:   []string{"password", "secret", "token", "client_secret", "private_key"},
				Mask:   "***REDACTED***",
				IPMode: "keep",
			},
		},
		Workers: WorkersConfig{
			NotificationWorkers: 1,
			EventWorkers:        1,
			LakeBatchSize:       1000,
			LakeBatchWindowSecs: 5,
		},
		RateLimit: RateLimitConfig{
			Backend:    "memory",
			GCInterval: 60,
		},
		TLS: TLSConfig{
			// Mode defaults to "" — resolved dynamically by ResolveMode().
			HTTPPort:  80,
			HTTPSPort: 443,
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
	applyServerEnv(cfg)
	applyTLSEnv(cfg)
	applyEncryptionEnv(cfg)
	applyDatabaseEnv(cfg)
	applyObservabilityEnv(cfg)
	applyDevEnv(cfg)
	applyRateLimitEnv(cfg)
}

func applyServerEnv(cfg *Config) {
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
	if v := os.Getenv("ZITADEL_BASE_PATH"); v != "" {
		cfg.Server.BasePath = v
	}
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

func applyTLSEnv(cfg *Config) {
	if v := os.Getenv("ZITADEL_TLS_MODE"); v != "" {
		cfg.TLS.Mode = v
	}
	if v := os.Getenv("ZITADEL_TLS_EMAIL"); v != "" {
		cfg.TLS.Email = v
	}
	if v := os.Getenv("ZITADEL_TLS_CA_DIR"); v != "" {
		cfg.TLS.CADir = v
	}
	if v := os.Getenv("ZITADEL_TLS_HTTP_PORT"); v != "" {
		if port, err := strconv.Atoi(v); err == nil {
			cfg.TLS.HTTPPort = port
		}
	}
	if v := os.Getenv("ZITADEL_TLS_HTTPS_PORT"); v != "" {
		if port, err := strconv.Atoi(v); err == nil {
			cfg.TLS.HTTPSPort = port
		}
	}
}

func applyDatabaseEnv(cfg *Config) {
	if v := os.Getenv("ZITADEL_DATABASE_URL"); v != "" {
		cfg.Database.URL = v
	}
	if v := os.Getenv("ZITADEL_DATABASE_MIGRATE"); v != "" {
		cfg.Database.Migrate = v
	}
	if v := os.Getenv("ZITADEL_DATABASE_BOOTSTRAP"); v != "" {
		cfg.Database.Bootstrap = v
	}
	if v := os.Getenv("ZITADEL_DATABASE_MAX_OPEN_CONNS"); v != "" {
		if n, err := strconv.Atoi(v); err == nil {
			cfg.Database.MaxOpenConns = n
		}
	}
	if v := os.Getenv("ZITADEL_DATABASE_MAX_IDLE_CONNS"); v != "" {
		if n, err := strconv.Atoi(v); err == nil {
			cfg.Database.MaxIdleConns = n
		}
	}
	if v := os.Getenv("ZITADEL_DATABASE_CONN_MAX_LIFETIME"); v != "" {
		cfg.Database.ConnMaxLifetime = v
	}
}

func applyObservabilityEnv(cfg *Config) {
	if v := os.Getenv("ZITADEL_OTLP_ENDPOINT"); v != "" {
		cfg.Observability.Sinks.OTEL.Endpoint = v
	}
	if v := os.Getenv("ZITADEL_LOG_LEVEL"); v != "" {
		cfg.Observability.LogLevel = v
	}
	if v := os.Getenv("ZITADEL_LOG_FORMAT"); v != "" {
		cfg.Observability.LogFormat = v
	}
	if v := os.Getenv("ZITADEL_LOG_STREAMS_RUNTIME"); v != "" {
		cfg.Observability.Streams.Runtime.Sinks = splitCSV(v)
	}
	if v := os.Getenv("ZITADEL_LOG_STREAMS_REQUEST"); v != "" {
		cfg.Observability.Streams.Request.Sinks = splitCSV(v)
	}
	if v := os.Getenv("ZITADEL_LOG_STREAMS_JOBS"); v != "" {
		cfg.Observability.Streams.Jobs.Sinks = splitCSV(v)
	}
	if v := os.Getenv("ZITADEL_LOG_STREAMS_EVENT_PUSHER"); v != "" {
		cfg.Observability.Streams.EventPusher.Sinks = splitCSV(v)
	}
	if v := os.Getenv("ZITADEL_LOG_REDACT_KEYS"); v != "" {
		cfg.Observability.Redaction.Keys = splitCSV(v)
	}
}

func applyDevEnv(cfg *Config) {
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
}

func applyRateLimitEnv(cfg *Config) {
	if v := os.Getenv("ZITADEL_RATE_LIMIT_BACKEND"); v != "" {
		cfg.RateLimit.Backend = v
	}
	if v := os.Getenv("ZITADEL_RATE_LIMIT_REDIS_URL"); v != "" {
		cfg.RateLimit.RedisURL = v
	}
	if v := os.Getenv("ZITADEL_RATE_LIMIT_GC_INTERVAL"); v != "" {
		if n, err := strconv.Atoi(v); err == nil {
			cfg.RateLimit.GCInterval = n
		}
	}
	if v := os.Getenv("ZITADEL_RATE_LIMIT_BATCH_WRITE"); v == "true" || v == "1" {
		cfg.RateLimit.BatchWrite = true
	}
}

func applyEncryptionEnv(cfg *Config) {
	if v := os.Getenv("ZITADEL_ENCRYPTION_ACTIVE_KEY_ID"); v != "" {
		cfg.Encryption.ActiveKeyID = v
	}
	// ZITADEL_ENCRYPTION_KEYS expects JSON: [{"id":"k1","secret":"hex..."},...]
	if v := os.Getenv("ZITADEL_ENCRYPTION_KEYS"); v != "" {
		var keys []EncryptionKey
		if err := json.Unmarshal([]byte(v), &keys); err == nil {
			cfg.Encryption.Keys = keys
		}
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
