package config

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const (
	DefaultDatabaseURL = "sqlite://./data/zitadel.db"
	LegacyDatabaseURL  = "sqlite://./zitadel.db"
	DefaultCachePath   = "./data/zitadel-cache.db"
	LegacyCachePath    = "zitadel-cache.db"
)

// LocalStorageResolution describes how implicit local storage paths were resolved.
type LocalStorageResolution struct {
	BaseDir             string
	DatabasePath        string
	DefaultDatabasePath string
	LegacyDatabasePath  string
	CachePath           string
	DefaultCachePath    string
	LegacyCachePath     string
	LegacyDatabaseUsed  bool
	LegacyCacheUsed     bool
}

// DatabaseURLExplicit reports whether database.url was set explicitly in config or env.
func (c *Config) DatabaseURLExplicit() bool {
	return c.databaseURLExplicit
}

// CachePathExplicit reports whether observability.cache_path was set explicitly in config.
func (c *Config) CachePathExplicit() bool {
	return c.cachePathExplicit
}

// ResolveLocalStorage rewrites implicit local paths to concrete filesystem paths.
// Defaults are resolved relative to the config file directory when --config is used,
// otherwise relative to the current working directory. Existing flat-root legacy
// database files are adopted to avoid booting a fresh instance unexpectedly.
func (c *Config) ResolveLocalStorage(configPath string) (*LocalStorageResolution, error) {
	baseDir, err := resolveStorageBaseDir(configPath)
	if err != nil {
		return nil, err
	}

	resolution := &LocalStorageResolution{
		BaseDir:             baseDir,
		DefaultDatabasePath: resolveLocalPath(baseDir, strings.TrimPrefix(DefaultDatabaseURL, "sqlite://")),
		LegacyDatabasePath:  resolveLocalPath(baseDir, strings.TrimPrefix(LegacyDatabaseURL, "sqlite://")),
		DefaultCachePath:    resolveLocalPath(baseDir, DefaultCachePath),
		LegacyCachePath:     resolveLocalPath(baseDir, LegacyCachePath),
	}

	if c.Database.URL == DefaultDatabaseURL {
		if fileExists(resolution.LegacyDatabasePath) && !fileExists(resolution.DefaultDatabasePath) {
			c.Database.URL = "sqlite://" + resolution.LegacyDatabasePath
			resolution.DatabasePath = resolution.LegacyDatabasePath
			resolution.LegacyDatabaseUsed = true
		} else {
			c.Database.URL = "sqlite://" + resolution.DefaultDatabasePath
			resolution.DatabasePath = resolution.DefaultDatabasePath
		}
	} else if strings.HasPrefix(c.Database.URL, "sqlite://") {
		resolution.DatabasePath = strings.TrimPrefix(c.Database.URL, "sqlite://")
	}

	if c.Observability.CachePath == DefaultCachePath {
		if resolution.LegacyDatabaseUsed && fileExists(resolution.LegacyCachePath) {
			c.Observability.CachePath = resolution.LegacyCachePath
			resolution.CachePath = resolution.LegacyCachePath
			resolution.LegacyCacheUsed = true
		} else {
			c.Observability.CachePath = resolution.DefaultCachePath
			resolution.CachePath = resolution.DefaultCachePath
		}
	} else {
		resolution.CachePath = c.Observability.CachePath
	}

	return resolution, nil
}

func resolveStorageBaseDir(configPath string) (string, error) {
	if configPath == "" {
		wd, err := os.Getwd()
		if err != nil {
			return "", fmt.Errorf("get working directory: %w", err)
		}
		return wd, nil
	}

	absConfigPath, err := filepath.Abs(configPath)
	if err != nil {
		return "", fmt.Errorf("resolve config path %s: %w", configPath, err)
	}
	return filepath.Dir(absConfigPath), nil
}

func resolveLocalPath(baseDir, path string) string {
	if path == "" || filepath.IsAbs(path) {
		return path
	}
	return filepath.Join(baseDir, path)
}

func fileExists(path string) bool {
	if path == "" {
		return false
	}
	_, err := os.Stat(path)
	return err == nil
}
