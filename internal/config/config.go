// Package config handles runtime configuration for the ZITADEL server.
// Config is loaded from TOML files, environment variables, and CLI flags.
// Domain-specific configuration (policies, whitelabeling, etc.) lives in
// bootstrap.yaml, NOT here.
package config

import (
	"fmt"
	"os"
	"strconv"

	"github.com/BurntSushi/toml"
)

// Config is the runtime configuration for the ZITADEL server.
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
	Port           int    `toml:"port"`
	ExternalDomain string `toml:"external_domain"`
	TLSCert        string `toml:"tls_cert"`
	TLSKey         string `toml:"tls_key"`
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
}
